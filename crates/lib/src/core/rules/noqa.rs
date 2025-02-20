use ahash::HashSet;
use itertools::Itertools;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::errors::SQLBaseError;
use sqruff_lib_core::parser::segments::base::ErasedSegment;

/// The NoQA directive is a way to disable specific rules or all rules for a specific line or range of lines.
/// Similar to flake8’s ignore, individual lines can be ignored by adding `-- noqa` to the end of the line.
/// Additionally, specific rules can be ignored by quoting their code or the category.
///
/// ## Ignoring single line errors
///
/// The following example will ignore all errors on line 1.
///
/// ```sql
/// -- Ignore all errors
/// SeLeCt  1 from tBl ;    -- noqa
///
/// -- Ignore rule CP02 & rule CP03
/// SeLeCt  1 from tBl ;    -- noqa: CP02,CP03
/// ```
///
/// ## Ignoring multiple line errors
///
/// Similar to pylint’s “pylint directive”, ranges of lines can be ignored by adding `-- noqa:disable=<rule>[,...] | all` to the line.
/// Following this directive, specified rules (or all rules, if “all” was specified)
/// will be ignored until a corresponding `-– noqa:enable=<rule>[,…] | all`.
///
/// For example:
///
/// ```sql
/// -- Ignore rule AL02 from this line forward
/// SELECT col_a a FROM foo -- noqa: disable=AL02
///
/// -- Ignore all rules from this line forward
/// SELECT col_a a FROM foo -- noqa: disable=all
///
/// -- Enforce all rules from this line forward
/// SELECT col_a a FROM foo -- noqa: enable=all
/// ```
#[derive(Eq, PartialEq, Debug, Clone)]
enum NoQADirective {
    LineIgnoreAll(LineIgnoreAll),
    LineIgnoreRules(LineIgnoreRules),
    RangeIgnoreAll(RangeIgnoreAll),
    RangeIgnoreRules(RangeIgnoreRules),
}

impl NoQADirective {
    /// validate checks if the NoQADirective is valid by checking it against a rule set and returns
    /// error if it is valid against a set of errors rules
    #[allow(dead_code)]
    fn validate_against_rules(&self, available_rules: &HashSet<&str>) -> Result<(), SQLBaseError> {
        fn check_rules(
            rules: &HashSet<String>,
            available_rules: &HashSet<&str>,
        ) -> Result<(), SQLBaseError> {
            for rule in rules {
                if !available_rules.contains(rule.as_str()) {
                    return Err(SQLBaseError {
                        fatal: true,
                        ignore: false,
                        warning: false,
                        line_no: 0,
                        line_pos: 0,
                        description: format!("Rule {} not found in rule set", rule),
                        rule: None,
                        source_slice: Default::default(),
                        fixable: false,
                    });
                }
            }
            Ok(())
        }

        match self {
            NoQADirective::LineIgnoreAll(_) => Ok(()),
            NoQADirective::LineIgnoreRules(LineIgnoreRules { rules, .. }) => {
                check_rules(rules, available_rules)
            }
            NoQADirective::RangeIgnoreAll(_) => Ok(()),
            NoQADirective::RangeIgnoreRules(RangeIgnoreRules { rules, .. }) => {
                check_rules(rules, available_rules)
            }
        }
    }

    /// Extract ignore mask entries from a comment string, returning a NoQADirective if found. It
    /// does not validate the directive rules, only parses it.
    fn parse_from_comment(
        original_comment: &str,
        // TODO eventually could refactor the type
        line_no: usize,
        line_pos: usize,
    ) -> Result<Option<Self>, SQLBaseError> {
        // Comment lines can also have noqa e.g.
        //     --dafhsdkfwdiruweksdkjdaffldfsdlfjksd -- noqa: LT05
        // Therefore extract last possible inline ignore.
        let comment = original_comment.split("--").last();
        if let Some(comment) = comment {
            let comment = comment.trim();
            if let Some(comment) = comment.strip_prefix(NOQA_PREFIX) {
                let comment = comment.trim();
                if comment.is_empty() {
                    Ok(Some(NoQADirective::LineIgnoreAll(LineIgnoreAll {
                        line_no,
                        line_pos,
                        raw_string: original_comment.to_string(),
                    })))
                } else if let Some(comment) = comment.strip_prefix(":") {
                    let comment = comment.trim();
                    if let Some(comment) = comment.strip_prefix("disable=") {
                        let comment = comment.trim();
                        if comment == "all" {
                            Ok(Some(NoQADirective::RangeIgnoreAll(RangeIgnoreAll {
                                line_no,
                                line_pos,
                                raw_string: original_comment.to_string(),
                                action: IgnoreAction::Disable,
                            })))
                        } else {
                            let rules: HashSet<_> = comment
                                .split(",")
                                .map(|rule| rule.trim().to_string())
                                .filter(|rule| !rule.is_empty())
                                .collect();
                            if rules.is_empty() {
                                Err(SQLBaseError {
                                    fatal: true,
                                    ignore: false,
                                    warning: false,
                                    line_no,
                                    line_pos,
                                    description: "Malformed 'noqa' section. Expected 'noqa: <rule>[,...] | all'"
                                        .into(),
                                    rule: None,
                                    source_slice: Default::default(),
                                    fixable: false,
                                })
                            } else {
                                Ok(Some(NoQADirective::RangeIgnoreRules(RangeIgnoreRules {
                                    line_no,
                                    line_pos,
                                    raw_string: original_comment.into(),
                                    action: IgnoreAction::Disable,
                                    rules,
                                })))
                            }
                        }
                    } else if let Some(comment) = comment.strip_prefix("enable=") {
                        let comment = comment.trim();
                        if comment == "all" {
                            Ok(Some(NoQADirective::RangeIgnoreAll(RangeIgnoreAll {
                                line_no,
                                line_pos,
                                action: IgnoreAction::Enable,
                                raw_string: original_comment.to_string(),
                            })))
                        } else {
                            let rules: HashSet<_> = comment
                                .split(",")
                                .map(|rule| rule.trim().to_string())
                                .filter(|rule| !rule.is_empty())
                                .collect();
                            if rules.is_empty() {
                                Err(SQLBaseError {
                                    fatal: true,
                                    ignore: false,
                                    warning: false,
                                    line_no,
                                    line_pos,
                                    description:
                                        "Malformed 'noqa' section. Expected 'noqa: <rule>[,...]'"
                                            .to_string(),
                                    rule: None,
                                    source_slice: Default::default(),
                                    fixable: false,
                                })
                            } else {
                                Ok(Some(NoQADirective::RangeIgnoreRules(RangeIgnoreRules {
                                    line_no,
                                    line_pos,
                                    raw_string: original_comment.to_string(),
                                    action: IgnoreAction::Enable,
                                    rules,
                                })))
                            }
                        }
                    } else if !comment.is_empty() {
                        let rules = comment.split(",").map_into().collect::<HashSet<String>>();
                        if rules.is_empty() {
                            Err(SQLBaseError {
                                fatal: true,
                                ignore: false,
                                warning: false,
                                line_no,
                                line_pos,
                                description:
                                    "Malformed 'noqa' section. Expected 'noqa: <rule>[,...] | all'"
                                        .into(),
                                rule: None,
                                source_slice: Default::default(),
                                fixable: false,
                            })
                        } else {
                            return Ok(Some(NoQADirective::LineIgnoreRules(LineIgnoreRules {
                                line_no,
                                line_pos: 0,
                                raw_string: original_comment.into(),
                                rules,
                            })));
                        }
                    } else {
                        Err(SQLBaseError {
                            fatal: true,
                            ignore: false,
                            warning: false,
                            line_no,
                            line_pos,
                            description:
                                "Malformed 'noqa' section. Expected 'noqa: <rule>[,...] | all'"
                                    .into(),
                            rule: None,
                            source_slice: Default::default(),
                            fixable: false,
                        })
                    }
                } else {
                    Err(SQLBaseError {
                        fatal: true,
                        ignore: false,
                        warning: false,
                        line_no,
                        line_pos,
                        description:
                            "Malformed 'noqa' section. Expected 'noqa' or 'noqa: <rule>[,...]'"
                                .to_string(),
                        rule: None,
                        source_slice: Default::default(),
                        fixable: false,
                    })
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, strum_macros::EnumString)]
#[strum(serialize_all = "lowercase")]
enum IgnoreAction {
    Enable,
    Disable,
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct RangeIgnoreAll {
    line_no: usize,
    line_pos: usize,
    raw_string: String,
    action: IgnoreAction,
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct RangeIgnoreRules {
    line_no: usize,
    line_pos: usize,
    raw_string: String,
    action: IgnoreAction,
    rules: HashSet<String>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct LineIgnoreAll {
    line_no: usize,
    line_pos: usize,
    raw_string: String,
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct LineIgnoreRules {
    line_no: usize,
    line_pos: usize,
    raw_string: String,
    rules: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct IgnoreMask {
    ignore_list: Vec<NoQADirective>,
}

const NOQA_PREFIX: &str = "noqa";

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
                fixable: false,
            })?
            .source_position();
        NoQADirective::parse_from_comment(comment_content, line_no, line_pos)
    }

    /// Parse a `noqa` directive from an erased segment.
    ///
    /// TODO - The output IgnoreMask should be validated against the ruleset.
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
    /// TODO - The parsing should also return warnings for rules that aren't used
    pub fn is_masked(&self, violation: &SQLBaseError) -> bool {
        fn is_masked_by_line_rules(ignore_mask: &IgnoreMask, violation: &SQLBaseError) -> bool {
            for ignore in &ignore_mask.ignore_list {
                match ignore {
                    NoQADirective::LineIgnoreAll(LineIgnoreAll { line_no, .. }) => {
                        if violation.line_no == *line_no {
                            return true;
                        }
                    }
                    NoQADirective::LineIgnoreRules(LineIgnoreRules { line_no, rules, .. }) => {
                        if violation.line_no == *line_no {
                            if let Some(rule) = &violation.rule {
                                if rules.contains(rule.code) {
                                    return true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            false
        }

        /// is_masked_by_range returns true if the violation is masked by the RangeIgnoreRules and
        /// RangeIgnoreAll components in the ignore mask
        fn is_masked_by_range_rules(ignore_mask: &IgnoreMask, violation: &SQLBaseError) -> bool {
            // Collect RangeIgnore directives
            let mut directives = Vec::new();

            for ignore in &ignore_mask.ignore_list {
                match ignore {
                    NoQADirective::RangeIgnoreAll(RangeIgnoreAll {
                        line_no, line_pos, ..
                    }) => {
                        directives.push((line_no, line_pos, ignore));
                    }
                    NoQADirective::RangeIgnoreRules(RangeIgnoreRules {
                        line_no, line_pos, ..
                    }) => {
                        directives.push((line_no, line_pos, ignore));
                    }
                    _ => {}
                }
            }

            // Sort directives by line_no, line_pos
            directives.sort_by(|(line_no1, line_pos1, _), (line_no2, line_pos2, _)| {
                line_no1.cmp(line_no2).then(line_pos1.cmp(line_pos2))
            });

            // Initialize state
            let mut all_rules_disabled = false;
            let mut disabled_rules = <HashSet<String>>::default();

            // For each directive
            for (line_no, line_pos, ignore) in directives {
                // Check if the directive is before the violation
                if *line_no > violation.line_no {
                    break;
                }
                if *line_no == violation.line_no && *line_pos > violation.line_pos {
                    break;
                }

                // Process the directive
                match ignore {
                    NoQADirective::RangeIgnoreAll(RangeIgnoreAll { action, .. }) => match action {
                        IgnoreAction::Disable => {
                            all_rules_disabled = true;
                        }
                        IgnoreAction::Enable => {
                            all_rules_disabled = false;
                        }
                    },
                    NoQADirective::RangeIgnoreRules(RangeIgnoreRules { action, rules, .. }) => {
                        match action {
                            IgnoreAction::Disable => {
                                for rule in rules {
                                    disabled_rules.insert(rule.clone());
                                }
                            }
                            IgnoreAction::Enable => {
                                for rule in rules {
                                    disabled_rules.remove(rule);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Check whether the violation is masked
            if all_rules_disabled {
                return true;
            } else if let Some(rule) = &violation.rule {
                if disabled_rules.contains(rule.code) {
                    return true;
                }
            }

            false
        }

        is_masked_by_line_rules(self, violation) || is_masked_by_range_rules(self, violation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;
    use crate::core::rules::noqa::NoQADirective;
    use itertools::Itertools;
    use sqruff_lib_core::errors::ErrorStructRule;

    #[test]
    fn test_is_masked_single_line() {
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
            fixable: true,
        };
        let mask = IgnoreMask {
            ignore_list: vec![NoQADirective::LineIgnoreRules(LineIgnoreRules {
                line_no: 2,
                line_pos: 13,
                raw_string: "--noqa: AL02".to_string(),
                rules: ["AL02".to_string()].into_iter().collect(),
            })],
        };
        let not_mask_wrong_line = IgnoreMask {
            ignore_list: vec![NoQADirective::LineIgnoreRules(LineIgnoreRules {
                line_no: 3,
                line_pos: 13,
                raw_string: "--noqa: AL02".to_string(),
                rules: ["AL02".to_string()].into_iter().collect(),
            })],
        };
        let not_mask_wrong_rule = IgnoreMask {
            ignore_list: vec![NoQADirective::LineIgnoreRules(LineIgnoreRules {
                line_no: 3,
                line_pos: 13,
                raw_string: "--noqa: AL03".to_string(),
                rules: ["AL03".to_string()].into_iter().collect(),
            })],
        };

        assert!(!not_mask_wrong_line.is_masked(&error));
        assert!(!not_mask_wrong_rule.is_masked(&error));
        assert!(mask.is_masked(&error));
    }

    #[test]
    fn test_parse_noqa() {
        let test_cases = vec![
            ("", Ok::<Option<NoQADirective>, &'static str>(None)),
            (
                "noqa",
                Ok(Some(NoQADirective::LineIgnoreAll(LineIgnoreAll {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa".to_string(),
                }))),
            ),
            (
                "noqa?",
                Err("Malformed 'noqa' section. Expected 'noqa' or 'noqa: <rule>[,...]'"),
            ),
            (
                "noqa:",
                Err("Malformed 'noqa' section. Expected 'noqa: <rule>[,...] | all'"),
            ),
            (
                "noqa: ",
                Err("Malformed 'noqa' section. Expected 'noqa: <rule>[,...] | all'"),
            ),
            (
                "noqa: LT01,LT02",
                Ok(Some(NoQADirective::LineIgnoreRules(LineIgnoreRules {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa: LT01,LT02".into(),
                    rules: ["LT01", "LT02"]
                        .into_iter()
                        .map_into()
                        .collect::<HashSet<String>>(),
                }))),
            ),
            (
                "noqa: enable=LT01",
                Ok(Some(NoQADirective::RangeIgnoreRules(RangeIgnoreRules {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa: enable=LT01".to_string(),
                    action: IgnoreAction::Enable,
                    rules: ["LT01"].into_iter().map_into().collect::<HashSet<String>>(),
                }))),
            ),
            (
                "noqa: disable=CP01",
                Ok(Some(NoQADirective::RangeIgnoreRules(RangeIgnoreRules {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa: disable=CP01".to_string(),
                    action: IgnoreAction::Disable,
                    rules: ["CP01"].into_iter().map_into().collect::<HashSet<String>>(),
                }))),
            ),
            (
                "noqa: disable=all",
                Ok(Some(NoQADirective::RangeIgnoreAll(RangeIgnoreAll {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "noqa: disable=all".to_string(),
                    action: IgnoreAction::Disable,
                }))),
            ),
            // TODO Implement
            // ("noqa: disable", Err("")),
            (
                "Inline comment before inline ignore -- noqa: disable=LT01,LT02",
                Ok(Some(NoQADirective::RangeIgnoreRules(RangeIgnoreRules {
                    line_no: 0,
                    line_pos: 0,
                    raw_string: "Inline comment before inline ignore -- noqa: disable=LT01,LT02"
                        .to_string(),
                    action: IgnoreAction::Disable,
                    rules: ["LT01".to_string(), "LT02".to_string()]
                        .into_iter()
                        .collect(),
                }))),
            ),
        ];

        for (input, expected) in test_cases {
            let result = NoQADirective::parse_from_comment(input, 0, 0);
            match expected {
                Ok(_) => assert_eq!(result.unwrap(), expected.unwrap()),
                Err(err) => {
                    assert!(result.is_err());
                    let result_err = result.err().unwrap();
                    assert_eq!(result_err.description, err);
                    assert!(result_err.fatal);
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
[sqruff]
dialect = bigquery
rules = AL02
    "#,
                None,
            ),
            None,
            None,
            false,
        );

        let sql = r#"SELECT
    col_a a,
    col_b b --noqa: AL02
FROM foo
"#;

        let result = linter.lint_string(sql, None, false);
        let violations = result.get_violations(None);

        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations.iter().map(|v| v.line_no).collect::<Vec<_>>(),
            [2].to_vec()
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
                None,
            ),
            None,
            None,
            false,
        );
        let linter_with_disabled = Linter::new(
            FluffConfig::from_source(
                r#"
[sqruff]
dialect = bigquery
rules = AL02
disable_noqa = True
    "#,
                None,
            ),
            None,
            None,
            false,
        );

        let sql = r#"SELECT
    col_a a,
    col_b b --noqa
FROM foo
    "#;
        let result_with_disabled = linter_with_disabled.lint_string(sql, None, false);
        let result_without_disabled = linter_without_disabled.lint_string(sql, None, false);

        assert_eq!(result_without_disabled.get_violations(None).len(), 1);
        assert_eq!(result_with_disabled.get_violations(None).len(), 2);
    }

    #[test]
    fn test_range_code() {
        let linter_without_disabled = Linter::new(
            FluffConfig::from_source(
                r#"
[sqruff]
dialect = bigquery
rules = AL02
    "#,
                None,
            ),
            None,
            None,
            false,
        );
        let sql_disable_rule = r#"SELECT
    col_a a,
    col_c c, --noqa: disable=AL02
    col_d d,
    col_e e, --noqa: enable=AL02
    col_f f
FROM foo
"#;

        let sql_disable_all = r#"SELECT
    col_a a,
    col_c c, --noqa: disable=all
    col_d d,
    col_e e, --noqa: enable=all
    col_f f
FROM foo
"#;
        let result_rule = linter_without_disabled.lint_string(sql_disable_rule, None, false);
        let result_all = linter_without_disabled.lint_string(sql_disable_all, None, false);

        assert_eq!(result_rule.get_violations(None).len(), 3);
        assert_eq!(result_all.get_violations(None).len(), 3);
    }
}
