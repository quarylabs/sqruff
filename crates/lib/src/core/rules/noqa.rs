use std::cell::Cell;

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::errors::{ErrorStructRule, SQLBaseError};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::rules::{ErasedRule, LintResult};

pub trait HasViolation {
    fn source_position(&self) -> Option<(usize, usize)>;
}

impl HasViolation for SQLBaseError {
    fn source_position(&self) -> Option<(usize, usize)> {
        Some((self.line_no, self.line_pos))
    }
}

impl HasViolation for LintResult {
    fn source_position(&self) -> Option<(usize, usize)> {
        self.anchor
            .as_ref()?
            .get_position_marker()
            .map(|m| m.source_position())
    }
}

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
                        fixable: false,
                        line_no: 0,
                        line_pos: 0,
                        description: format!("Rule {rule} not found in rule set"),
                        rule: None,
                        source_slice: Default::default(),
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
                                    fixable: false,
                                    line_no,
                                    line_pos,
                                    description: "Malformed 'noqa' section. Expected 'noqa: <rule>[,...] | all'"
                                        .into(),
                                    rule: None,
                                    source_slice: Default::default(),
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
                                    fixable: false,
                                    line_no,
                                    line_pos,
                                    description:
                                        "Malformed 'noqa' section. Expected 'noqa: <rule>[,...]'"
                                            .to_string(),
                                    rule: None,
                                    source_slice: Default::default(),
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
                                fixable: false,
                                line_no,
                                line_pos,
                                description:
                                    "Malformed 'noqa' section. Expected 'noqa: <rule>[,...] | all'"
                                        .into(),
                                rule: None,
                                source_slice: Default::default(),
                            })
                        } else {
                            Ok(Some(NoQADirective::LineIgnoreRules(LineIgnoreRules {
                                line_no,
                                line_pos,
                                raw_string: original_comment.into(),
                                rules,
                            })))
                        }
                    } else {
                        Err(SQLBaseError {
                            fixable: false,
                            line_no,
                            line_pos,
                            description:
                                "Malformed 'noqa' section. Expected 'noqa: <rule>[,...] | all'"
                                    .into(),
                            rule: None,
                            source_slice: Default::default(),
                        })
                    }
                } else {
                    Err(SQLBaseError {
                        fixable: false,
                        line_no,
                        line_pos,
                        description:
                            "Malformed 'noqa' section. Expected 'noqa' or 'noqa: <rule>[,...]'"
                                .to_string(),
                        rule: None,
                        source_slice: Default::default(),
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

impl NoQADirective {
    fn line_no(&self) -> usize {
        match self {
            NoQADirective::LineIgnoreAll(d) => d.line_no,
            NoQADirective::LineIgnoreRules(d) => d.line_no,
            NoQADirective::RangeIgnoreAll(d) => d.line_no,
            NoQADirective::RangeIgnoreRules(d) => d.line_no,
        }
    }

    fn line_pos(&self) -> usize {
        match self {
            NoQADirective::LineIgnoreAll(d) => d.line_pos,
            NoQADirective::LineIgnoreRules(d) => d.line_pos,
            NoQADirective::RangeIgnoreAll(d) => d.line_pos,
            NoQADirective::RangeIgnoreRules(d) => d.line_pos,
        }
    }

    fn raw_string(&self) -> &str {
        match self {
            NoQADirective::LineIgnoreAll(d) => &d.raw_string,
            NoQADirective::LineIgnoreRules(d) => &d.raw_string,
            NoQADirective::RangeIgnoreAll(d) => &d.raw_string,
            NoQADirective::RangeIgnoreRules(d) => &d.raw_string,
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
    /// Tracks, per directive in `ignore_list`, whether the directive was ever
    /// used to mask a violation. Held separately (rather than as a field on the
    /// directive) so directive equality/parsing stay untouched, and behind
    /// `Cell` so it can be marked during the immutable `is_masked` pass.
    used: Vec<Cell<bool>>,
}

const NOQA_PREFIX: &str = "noqa";

impl IgnoreMask {
    fn new(ignore_list: Vec<NoQADirective>) -> Self {
        let used = ignore_list.iter().map(|_| Cell::new(false)).collect();
        IgnoreMask { ignore_list, used }
    }

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
                fixable: false,
                line_no: 0,
                line_pos: 0,
                description: "Could not get position marker".to_string(),
                rule: None,
                source_slice: Default::default(),
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
        (IgnoreMask::new(ignore_list), violations)
    }

    /// is_masked returns true if the IgnoreMask masks the violation.
    ///
    /// When `mark_used` is true, the directive(s) responsible for masking the
    /// violation are recorded as "used" so that any remaining unused directives
    /// can later be surfaced via [`IgnoreMask::generate_warnings_for_unused`].
    pub fn is_masked(
        &self,
        violation: &impl HasViolation,
        rule: Option<&ErasedRule>,
        mark_used: bool,
    ) -> bool {
        let Some((vline_no, vline_pos)) = violation.source_position() else {
            return true;
        };

        // Line-specific directives.
        for (idx, ignore) in self.ignore_list.iter().enumerate() {
            match ignore {
                NoQADirective::LineIgnoreAll(LineIgnoreAll { line_no, .. })
                    if vline_no == *line_no =>
                {
                    if mark_used {
                        self.used[idx].set(true);
                    }
                    return true;
                }
                NoQADirective::LineIgnoreRules(LineIgnoreRules { line_no, rules, .. }) => {
                    if vline_no == *line_no
                        && let Some(rule) = rule
                        && rules.contains(rule.code())
                    {
                        if mark_used {
                            self.used[idx].set(true);
                        }
                        return true;
                    }
                }
                _ => {}
            }
        }

        // Range directives (`disable`/`enable`). Collect their indices along
        // with their position so they can be evaluated in source order.
        let mut directives: Vec<(usize, usize, usize)> = Vec::new();
        for (idx, ignore) in self.ignore_list.iter().enumerate() {
            match ignore {
                NoQADirective::RangeIgnoreAll(RangeIgnoreAll {
                    line_no, line_pos, ..
                })
                | NoQADirective::RangeIgnoreRules(RangeIgnoreRules {
                    line_no, line_pos, ..
                }) => {
                    directives.push((*line_no, *line_pos, idx));
                }
                _ => {}
            }
        }

        // Sort directives by line_no, line_pos
        directives.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        // Initialize state
        let mut all_rules_disabled = false;
        let mut disabled_rules = <HashSet<String>>::default();
        // Track which directive is currently responsible for a disabled scope,
        // so it can be marked used if it ends up masking this violation.
        let mut responsible_all: Option<usize> = None;
        let mut responsible_rule: HashMap<String, usize> = HashMap::default();

        for (line_no, line_pos, idx) in directives {
            // Check if the directive is before the violation
            if line_no > vline_no {
                break;
            }
            if line_no == vline_no && line_pos > vline_pos {
                break;
            }

            // Process the directive
            match &self.ignore_list[idx] {
                NoQADirective::RangeIgnoreAll(RangeIgnoreAll { action, .. }) => match action {
                    IgnoreAction::Disable => {
                        all_rules_disabled = true;
                        responsible_all = Some(idx);
                    }
                    IgnoreAction::Enable => {
                        // An enable is "used" if it counteracts an active disable.
                        if all_rules_disabled && mark_used {
                            self.used[idx].set(true);
                        }
                        all_rules_disabled = false;
                        responsible_all = None;
                    }
                },
                NoQADirective::RangeIgnoreRules(RangeIgnoreRules { action, rules, .. }) => {
                    match action {
                        IgnoreAction::Disable => {
                            for rule in rules {
                                disabled_rules.insert(rule.clone());
                                responsible_rule.insert(rule.clone(), idx);
                            }
                        }
                        IgnoreAction::Enable => {
                            let mut counteracted = false;
                            for rule in rules {
                                if disabled_rules.remove(rule) {
                                    counteracted = true;
                                }
                                responsible_rule.remove(rule);
                            }
                            if counteracted && mark_used {
                                self.used[idx].set(true);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Check whether the violation is masked
        if all_rules_disabled {
            if mark_used && let Some(i) = responsible_all {
                self.used[i].set(true);
            }
            return true;
        } else if let Some(rule) = rule
            && disabled_rules.contains(rule.code())
        {
            if mark_used && let Some(&i) = responsible_rule.get(rule.code()) {
                self.used[i].set(true);
            }
            return true;
        }

        false
    }

    /// Generate a warning for every directive that never masked a violation.
    ///
    /// Mirrors SQLFluff's `IgnoreMask.generate_warnings_for_unused`, surfacing
    /// `-- noqa:` comments that had no effect so they can be cleaned up.
    pub fn generate_warnings_for_unused(&self) -> Vec<SQLBaseError> {
        self.ignore_list
            .iter()
            .zip(self.used.iter())
            .filter(|(_, used)| !used.get())
            .map(|(directive, _)| {
                // Strip any leading comment marker so the message reads
                // `Unused noqa: 'noqa: CP01'` rather than including `--`.
                let raw = directive.raw_string();
                let text = raw.split("--").last().unwrap_or(raw).trim();
                SQLBaseError {
                    fixable: false,
                    line_no: directive.line_no(),
                    line_pos: directive.line_pos(),
                    description: format!("Unused noqa: '{text}'"),
                    rule: Some(ErrorStructRule {
                        name: "noqa",
                        code: "NOQA",
                    }),
                    source_slice: Default::default(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;
    use crate::core::rules::Erased;
    use crate::core::rules::noqa::NoQADirective;
    use itertools::Itertools;

    #[test]
    fn test_is_masked_single_line() {
        let error = SQLBaseError {
            fixable: true,
            line_no: 2,
            line_pos: 11,
            description: "Implicit/explicit aliasing of columns.".to_string(),
            rule: None,
            source_slice: Default::default(),
        };
        let mask = IgnoreMask::new(vec![NoQADirective::LineIgnoreRules(LineIgnoreRules {
            line_no: 2,
            line_pos: 13,
            raw_string: "--noqa: AL02".to_string(),
            rules: ["AL02".to_string()].into_iter().collect(),
        })]);
        let not_mask_wrong_line =
            IgnoreMask::new(vec![NoQADirective::LineIgnoreRules(LineIgnoreRules {
                line_no: 3,
                line_pos: 13,
                raw_string: "--noqa: AL02".to_string(),
                rules: ["AL02".to_string()].into_iter().collect(),
            })]);
        let not_mask_wrong_rule =
            IgnoreMask::new(vec![NoQADirective::LineIgnoreRules(LineIgnoreRules {
                line_no: 3,
                line_pos: 13,
                raw_string: "--noqa: AL03".to_string(),
                rules: ["AL03".to_string()].into_iter().collect(),
            })]);

        assert!(!not_mask_wrong_line.is_masked(&error, None, false));
        assert!(!not_mask_wrong_rule.is_masked(&error, None, false));
        assert!(mask.is_masked(
            &error,
            Some(&crate::rules::aliasing::al02::RuleAL02::default().erased()),
            false
        ));
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
        )
        .unwrap();

        let sql = r#"SELECT
    col_a a,
    col_b b --noqa: AL02
FROM foo
"#;

        let result = linter.lint_string(sql, None, false).unwrap();
        let violations = result.violations();

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
        )
        .unwrap();
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
        )
        .unwrap();

        let sql = r#"SELECT
    col_a a,
    col_b b --noqa
FROM foo
    "#;
        let result_with_disabled = linter_with_disabled.lint_string(sql, None, false).unwrap();
        let result_without_disabled = linter_without_disabled
            .lint_string(sql, None, false)
            .unwrap();

        assert_eq!(result_without_disabled.violations().len(), 1);
        assert_eq!(result_with_disabled.violations().len(), 2);
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
        )
        .unwrap();
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
        let result_rule = linter_without_disabled
            .lint_string(sql_disable_rule, None, false)
            .unwrap();
        let result_all = linter_without_disabled
            .lint_string(sql_disable_all, None, false)
            .unwrap();

        assert_eq!(result_rule.violations().len(), 3);
        assert_eq!(result_all.violations().len(), 3);
    }

    #[test]
    /// A directive that masks a violation is "used"; one that masks nothing is
    /// reported by `generate_warnings_for_unused`.
    fn test_generate_warnings_for_unused() {
        let used = SQLBaseError {
            fixable: true,
            line_no: 2,
            line_pos: 11,
            description: "Implicit aliasing.".to_string(),
            rule: None,
            source_slice: Default::default(),
        };
        let mask = IgnoreMask::new(vec![
            NoQADirective::LineIgnoreRules(LineIgnoreRules {
                line_no: 2,
                line_pos: 13,
                raw_string: "--noqa: AL02".to_string(),
                rules: ["AL02".to_string()].into_iter().collect(),
            }),
            NoQADirective::LineIgnoreRules(LineIgnoreRules {
                line_no: 3,
                line_pos: 13,
                raw_string: "--noqa: AL02".to_string(),
                rules: ["AL02".to_string()].into_iter().collect(),
            }),
        ]);

        // Only the first directive masks a violation, so it becomes "used".
        assert!(mask.is_masked(
            &used,
            Some(&crate::rules::aliasing::al02::RuleAL02::default().erased()),
            true
        ));

        let warnings = mask.generate_warnings_for_unused();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].line_no, 3);
        assert_eq!(warnings[0].rule_code(), "NOQA");
        assert_eq!(warnings[0].description, "Unused noqa: 'noqa: AL02'");
    }

    #[test]
    /// End-to-end: `warn_unused_ignores` surfaces a warning for an inline noqa
    /// that never masked a violation, and stays silent when disabled.
    fn test_linter_warn_unused_noqa() {
        let sql = r#"SELECT
    col_a a, --noqa: AL02
    col_b --noqa: AL02
FROM foo
"#;

        let linter_off = Linter::new(
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
        )
        .unwrap();
        let linter_on = Linter::new(
            FluffConfig::from_source(
                r#"
[sqruff]
dialect = bigquery
rules = AL02
warn_unused_ignores = True
    "#,
                None,
            ),
            None,
            None,
            false,
        )
        .unwrap();

        // Without the option there are no NOQA warnings.
        let result_off = linter_off.lint_string(sql, None, false).unwrap();
        assert!(
            result_off
                .violations()
                .iter()
                .all(|v| v.rule_code() != "NOQA")
        );

        // With the option the unused directive on line 3 is reported, while the
        // directive on line 2 (which masks a real AL02 violation) is not.
        let result_on = linter_on.lint_string(sql, None, false).unwrap();
        let noqa_warnings: Vec<_> = result_on
            .violations()
            .iter()
            .filter(|v| v.rule_code() == "NOQA")
            .collect();
        assert_eq!(noqa_warnings.len(), 1);
        assert_eq!(noqa_warnings[0].line_no, 3);
        assert_eq!(noqa_warnings[0].description, "Unused noqa: 'noqa: AL02'");
    }
}
