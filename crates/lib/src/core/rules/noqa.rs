use ahash::HashSet;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::errors::SQLBaseError;
use sqruff_lib_core::parser::segments::base::ErasedSegment;
use std::str::FromStr;

#[derive(Eq, PartialEq, Debug)]
struct NoQADirective {
    line_no: usize,
    line_pos: usize,
    raw_string: String,
    // Can be Enable, Disable or None.
    // TODO Make the None clearer to actually what it is
    action: Option<IgnoreAction>,
    // This could be able to be a perfect map because we should know all the rules upfront.
    // TODO Make it clearer what None means
    rules: Option<HashSet<String>>,
    // TODO Introduce a method for used because want to be able to return all unused noqas
    // used: bool
}

#[derive(Debug, Default)]
pub struct IgnoreMask {
    ignore_list: Vec<NoQADirective>,
}

const NOQA_PREFIX: &str = "noqa";

#[derive(Eq, PartialEq, Debug, strum_macros::EnumString)]
#[strum(serialize_all = "lowercase")]
enum IgnoreAction {
    Enable,
    Disable,
}

impl IgnoreMask {
    /// Extract ignore mask entries from a comment segment
    fn extract_ignore_from_comment(
        comment: ErasedSegment,
    ) -> Result<Option<NoQADirective>, SQLBaseError> {
        // Trim any whitespace
        let mut comment_content = comment.raw().trim();
        // If we have leading or trailing block comment markers, also strip them.
        // NOTE: We need to strip block comment markers from the start
        // to ensure that noqa directives in the following form are followed:
        // /* noqa: disable=all */
        if comment_content.ends_with("*/") {
            comment_content = comment_content[..comment_content.len() - 2].trim_end();
        }
        if comment_content.starts_with("/*") {
            comment_content = comment_content[2..].trim_start();
        }
        let (line_no, line_pos) = comment
            .get_position_marker()
            .ok_or(SQLBaseError {
                fatal: true,
                ignore: false,
                warning: false,
                line_no: 0,
                line_pos: 0,
                description: "Could not get position marker".to_string(),
                rule: None,
                source_slice: Default::default(),
            })?
            .source_position();
        IgnoreMask::parse_noqa(comment_content, line_no, line_pos)
    }

    /// Extract ignore mask entries from a comment string.
    fn parse_noqa(
        original_comment: &str,
        line_no: usize,
        line_pos: usize,
    ) -> Result<Option<NoQADirective>, SQLBaseError> {
        // Comment lines can also have noqa e.g.
        //     --dafhsdkfwdiruweksdkjdaffldfsdlfjksd -- noqa: LT05
        // Therefore extract last possible inline ignore.
        let comment = original_comment.split("--").last();
        if let Some(comment) = comment {
            let comment = comment.trim();
            if comment.starts_with(NOQA_PREFIX) {
                if comment.trim() == NOQA_PREFIX {
                    return Ok(Some(NoQADirective {
                        line_no,
                        line_pos,
                        raw_string: comment.to_string(),
                        action: None,
                        rules: None,
                    }));
                }
                // TODO you could make this more efficient by stripping and checking start in one go
                let comment = comment.strip_prefix(NOQA_PREFIX).unwrap_or(comment);
                if !comment.starts_with(":") {
                    return Err(SQLBaseError {
                        fatal: true,
                        ignore: false,
                        warning: false,
                        line_no,
                        line_pos,
                        description: "Malformed 'noqa' section. Expected 'noqa: <rule>[,...]"
                            .to_string(),
                        rule: None,
                        // TODO: Add source slice
                        source_slice: Default::default(),
                    });
                }
                let comment = comment[1..].trim();

                let mut action: Option<IgnoreAction> = None;
                let mut rule_part: Option<&str> = None;

                if let Some(position) = comment.find("=") {
                    let (action_part, rule_part_parsed) = comment.split_at(position);
                    action = Some(IgnoreAction::from_str(action_part.trim()).map_err(
                        |err| SQLBaseError {
                            fatal: true,
                            ignore: false,
                            warning: false,
                            line_no,
                            line_pos,
                            description: format!("Malformed 'noqa' section. Expected --noqa: disable=[rules]|all or --noqa: enable=[rules]|all  : {}", err),
                            rule: None,
                            // TODO Add source slice
                            source_slice: Default::default(),
                        }
                    )?);
                    rule_part = Some(rule_part_parsed[1..].trim());
                } else if ["disable", "enable"].contains(&comment) {
                    return Err(SQLBaseError {
                        fatal: true,
                        ignore: false,
                        warning: false,
                        line_no,
                        line_pos,
                        description: "Malformed 'noqa' section. Expected --noqa: disable=[rules]|all or --noqa: enable=[rules]|all".to_string(),
                        rule: None,
                        // TODO Add source slice
                        source_slice: Default::default(),
                    });
                }

                return if rule_part == Some("all") {
                    Ok(Some(NoQADirective {
                        line_no,
                        line_pos,
                        raw_string: original_comment.to_string(),
                        action,
                        rules: None,
                    }))
                } else {
                    // TODO HERE SHOULD MAKE SURE EVERY RULE MAKES SENSE
                    let rules: HashSet<_> = rule_part
                        .unwrap_or("")
                        .split(",")
                        .map(|rule| rule.trim().to_string())
                        .filter(|rule| !rule.is_empty())
                        .collect();
                    if rules.is_empty() {
                        Ok(Some(NoQADirective {
                            line_no,
                            line_pos,
                            raw_string: original_comment.to_string(),
                            action,
                            rules: None,
                        }))
                    } else {
                        Ok(Some(NoQADirective {
                            line_no,
                            line_pos,
                            raw_string: original_comment.to_string(),
                            action,
                            rules: Some(rules),
                        }))
                    }
                };
            }
        }
        Ok(None)
    }

    // TODO See if need to implement reference_map
    pub fn from_tree(tree: &ErasedSegment) -> (IgnoreMask, Vec<SQLBaseError>) {
        let mut ignore_list: Vec<NoQADirective> = vec![];
        let mut violations: Vec<SQLBaseError> = vec![];

        for comment in tree.recursive_crawl(
            const {
                &SyntaxSet::new(&[
                    SyntaxKind::Comment,
                    SyntaxKind::InlineComment,
                    SyntaxKind::BlockComment,
                ])
            },
            false,
            &SyntaxSet::new(&[]),
            false,
        ) {
            let ignore_entry = IgnoreMask::extract_ignore_from_comment(comment);
            if let Err(err) = ignore_entry {
                violations.push(err);
            } else if let Ok(Some(ignore_entry)) = ignore_entry {
                ignore_list.push(ignore_entry);
            }
        }

        (IgnoreMask { ignore_list }, violations)
    }

    /// is_masked returns true if the IgnoreMask masks the violation
    pub fn is_masked(&self, violation: &SQLBaseError) -> bool {
        let ignore_on_specific_line = self
            .ignore_list
            .iter()
            .filter(|ignore| {
                ignore.action.is_none() || matches!(ignore.action, Some(IgnoreAction::Disable))
            })
            .filter(|ignore| ignore.line_no == violation.line_no)
            .collect::<Vec<_>>();

        for ignore in ignore_on_specific_line {
            if let (Some(violation_rule), Some(ignore_rules)) = (&violation.rule, &ignore.rules) {
                if ignore_rules.contains(violation_rule.code) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;
    use crate::core::rules::noqa::NoQADirective;
    use sqruff_lib_core::errors::ErrorStructRule;

    #[test]
    fn test_is_masked() {
        let error = SQLBaseError {
            fatal: false,
            ignore: false,
            warning: false,
            line_no: 2,
            line_pos: 11,
            description: "Implicit/explicit aliasing of columns.".to_string(),
            rule: Some(ErrorStructRule {
                name: "aliasing.column",
                code: "AL02",
            }),
            source_slice: Default::default(),
        };
        let mask = IgnoreMask {
            ignore_list: vec![NoQADirective {
                line_no: 2,
                line_pos: 13,
                raw_string: "--noqa: disable=AL02".to_string(),
                action: Some(IgnoreAction::Disable),
                rules: Some(["AL02".to_string()].into_iter().collect()),
            }],
        };
        let not_mask_wrong_line = IgnoreMask {
            ignore_list: vec![NoQADirective {
                line_no: 3,
                line_pos: 13,
                raw_string: "--noqa: disable=AL02".to_string(),
                action: Some(IgnoreAction::Disable),
                rules: Some(["AL02".to_string()].into_iter().collect()),
            }],
        };
        let not_mask_wrong_rule = IgnoreMask {
            ignore_list: vec![NoQADirective {
                line_no: 3,
                line_pos: 13,
                raw_string: "--noqa: disable=AL03".to_string(),
                action: Some(IgnoreAction::Disable),
                rules: Some(["AL03".to_string()].into_iter().collect()),
            }],
        };

        assert!(!not_mask_wrong_line.is_masked(&error));
        assert!(!not_mask_wrong_rule.is_masked(&error));
        assert!(mask.is_masked(&error));
    }

    #[test]
    fn test_parse_noqa() {
        let test_cases = vec![
            ("", Ok(None)),
            (
                "noqa",
                Ok(Some(NoQADirective {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa".to_string(),
                    action: None,
                    rules: None,
                })),
            ),
            ("noqa?", Err("")),
            (
                "noqa:",
                Ok(Some(NoQADirective {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa:".to_string(),
                    action: None,
                    rules: None,
                })),
            ),
            // (
            //     "noqa:LT01,LT02",
            //     Ok(Some(NoQADirective {
            //         line_no: 0,
            //         line_pos: 0,
            //         raw_string: "noqa:LT01,LT02".to_string(),
            //         action: None,
            //         rules: Some(
            //             ["LT01".to_string(), "LT02".to_string()]
            //                 .into_iter()
            //                 .collect(),
            //         ),
            //     })),
            // ),
            (
                "noqa: enable=LT01",
                Ok(Some(NoQADirective {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa: enable=LT01".to_string(),
                    action: Some(IgnoreAction::Enable),
                    rules: Some(["LT01".to_string()].into_iter().collect()),
                })),
            ),
            (
                "noqa: disable=CP01",
                Ok(Some(NoQADirective {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa: disable=CP01".to_string(),
                    action: Some(IgnoreAction::Disable),
                    rules: Some(["CP01".to_string()].into_iter().collect()),
                })),
            ),
            // (
            //     "noqa: disable=all",
            //     Ok(Some(NoQADirective {
            //         line_no: 0,
            //         line_pos: 0,
            //         raw_string: "noqa: disable=all".to_string(),
            //         action: Some(IgnoreAction::Disable),
            //         rules: None,
            //     })),
            // ),
            // ("noqa: disable", Err("")),
            (
                "Inline comment before inline ignore -- noqa: disable=LT01,LT02",
                Ok(Some(NoQADirective {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "Inline comment before inline ignore -- noqa: disable=LT01,LT02"
                        .to_string(),
                    action: Some(IgnoreAction::Disable),
                    rules: Some(
                        ["LT01".to_string(), "LT02".to_string()]
                            .into_iter()
                            .collect(),
                    ),
                })),
            ),
            // TODO Eventually think about GLOB expansion of rules
            // ("noqa:L04*", Some(NoQADirective { line_no: 0, line_pos: 0, raw_string: "noqa:L04*".to_string(), action: None, rules: Some(HashSet::from(["AM04".to_string(), "CP04".to_string(), "CV04".to_string(), "CV05".to_string(), "JJ01".to_string(), "LT01".to_string(), "LT10".to_string(), "ST02".to_string(), "ST03".to_string(), "ST05".to_string()])) })),
            // ("noqa:L002", Some(NoQADirective { line_no: 0, line_pos: 0, raw_string: "noqa:L002".to_string(), action: None, rules: Some(HashSet::from(["LT02".to_string()])) })),
            // ("noqa:L00*", Some(NoQADirective { line_no: 0, line_pos: 0, raw_string: "noqa:L00*".to_string(), action: None, rules: Some(HashSet::from(["LT01".to_string(), "LT02".to_string(), "LT03".to_string(), "LT12".to_string()])) })),
            // TODO Implement these as well
            // ("noqa:capitalisation.keywords", Some(NoQADirective { line_no: 0, line_pos: 0, raw_string: "noqa:capitalisation.keywords".to_string(), action: None, rules: Some(HashSet::from(["CP01".to_string()])) })),
            // ("noqa:capitalisation", Some(NoQADirective { line_no: 0, line_pos: 0, raw_string: "noqa:capitalisation".to_string(), action: None, rules: Some(HashSet::from(["CP01".to_string(), "CP02".to_string(), "CP03".to_string(), "CP04".to_string(), "CP05".to_string()])) })),
        ];

        for (input, expected) in test_cases {
            let result = IgnoreMask::parse_noqa(input, 0, 0);
            match expected {
                Ok(ref ok) => assert_eq!(result.unwrap(), expected.unwrap()),
                Err(_) => {
                    assert!(result.is_err());
                    assert!(result.err().unwrap().fatal);
                }
            }
        }
    }

    #[test]
    /// Test "noqa" feature at the higher "Linter" level.
    fn test_linter_single_noqa() {
        let linter = Linter::new(
            FluffConfig::from_source(
                r#"
dialect: bigquery,
rules: AL02
"#,
            ),
            None,
            None,
        );

        let sql = r#"SELECT
    col_a a,
    col_b b --noqa: disable=AL02
FROM foo
"#;

        let result = linter.lint_string(sql, None, false);
        let violations = result.get_violations(None);

        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations.iter().map(|v| v.line_no).collect::<Vec<_>>(),
            [2].iter().cloned().collect::<Vec<_>>()
        );
    }

    #[test]
    /// Test "noqa" feature at the higher "Linter" level and turn off noqa
    fn test_linter_noqa_but_disabled() {
        let linter_without_disabled = Linter::new(
            FluffConfig::from_source(
                r#"
[sqruff]
dialect = bigquery
rules = AL02
"#,
            ),
            None,
            None,
        );
        let linter_with_disabled = Linter::new(
            FluffConfig::from_source(
                r#"
[sqruff]
dialect = bigquery
rules = AL02
disable_noqa = True
"#,
            ),
            None,
            None,
        );

        let sql = r#"SELECT
    col_a a,
    col_b b --noqa: disable=AL02
FROM foo
"#;
        let result_with_disabled = linter_with_disabled.lint_string(sql, None, false);
        let result_without_disabled = linter_without_disabled.lint_string(sql, None, false);

        assert_eq!(result_without_disabled.get_violations(None).len(), 1);
        assert_eq!(result_with_disabled.get_violations(None).len(), 2);
    }
}
