use std::collections::hash_map::Entry;

use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Default, Clone)]
pub struct RuleAL08 {}

impl Rule for RuleAL08 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleAL08::default().erased()
    }

    fn name(&self) -> &'static str {
        "layout.cte_newline"
    }

    fn description(&self) -> &'static str {
        "Column aliases should be unique within each clause."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut used_aliases = AHashMap::new();
        let mut violations = Vec::new();

        for clause_element in context.segment.children(&["select_clause_element"]) {
            let mut column_alias = None;

            if let Some(alias_expression) = clause_element.child(&["alias_expression"]) {
                for it in alias_expression.segments() {
                    if !it.is_code() || it.get_raw_upper().unwrap() == "AS" {
                        continue;
                    }

                    column_alias = it.clone().into();
                    break;
                }
            } else if let Some(column_reference) = clause_element.child(&["column_reference"]) {
                column_alias = column_reference.segments().last().cloned();
            }

            let Some(column_alias) = column_alias else { continue };

            let key = column_alias.get_raw_upper().unwrap().replace(['\"', '\'', '`'], "");

            match used_aliases.entry(key) {
                Entry::Occupied(entry) => {
                    let previous: &ErasedSegment = entry.get();

                    let alias = column_alias.get_raw().unwrap();
                    let line_no = previous.get_position_marker().unwrap().source_position().0;

                    violations.push(LintResult::new(
                        column_alias.clone().into(),
                        vec![],
                        None,
                        format!("Reuse of column alias '{alias}' from line {line_no}.").into(),
                        None,
                    ))
                }
                Entry::Vacant(entry) => _ = entry.insert(clause_element),
            };
        }

        violations
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["select_clause"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::lint;
    use crate::core::rules::base::Erased;
    use crate::rules::aliasing::AL08::RuleAL08;

    #[test]
    fn test_fail_references() {
        let sql = "select foo, foo";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleAL08::default().erased()], None, None)
                .unwrap();

        assert_eq!(result[0].desc(), "Reuse of column alias 'foo' from line 1.");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_fail_aliases() {
        let sql = "select a as foo, b as foo";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleAL08::default().erased()], None, None)
                .unwrap();

        assert_eq!(result[0].desc(), "Reuse of column alias 'foo' from line 1.");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_fail_alias_refs() {
        let sql = "select foo, b as foo";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleAL08::default().erased()], None, None)
                .unwrap();

        assert_eq!(result[0].desc(), "Reuse of column alias 'foo' from line 1.");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_fail_locs() {
        let sql = "select foo, b as foo, c as bar, bar, d foo";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleAL08::default().erased()], None, None)
                .unwrap();

        assert_eq!(result[0].desc(), "Reuse of column alias 'foo' from line 1.");
        assert_eq!(result[1].desc(), "Reuse of column alias 'bar' from line 1.");
        assert_eq!(result[2].desc(), "Reuse of column alias 'foo' from line 1.");
        assert_eq!(result.len(), 3);
    }

    #[test]
    #[ignore = "snowflake"]
    fn test_fail_alias_quoted() {
        let sql = "select foo, b as \"foo\"";
        let result = lint(
            sql.to_string(),
            "the snowflake dialect is not implemented".into(),
            vec![RuleAL08::default().erased()],
            None,
            None,
        )
        .unwrap();

        dbg!(result);
    }

    #[test]
    fn test_fail_alias_case() {
        let sql = "select foo, b as FOO";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleAL08::default().erased()], None, None)
                .unwrap();

        assert_eq!(result[0].desc(), "Reuse of column alias 'FOO' from line 1.");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_fail_qualified() {
        let sql = "select a.foo, b as foo from a";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleAL08::default().erased()], None, None)
                .unwrap();

        assert_eq!(result[0].desc(), "Reuse of column alias 'foo' from line 1.");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_pass_table_names() {
        let sql = "select a.b, b.c, c.d from a, b, c";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleAL08::default().erased()], None, None)
                .unwrap();

        assert_eq!(result, []);
    }
}
