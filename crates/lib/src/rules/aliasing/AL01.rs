use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::ToErasedSegment;
use crate::utils::reflow::sequence::{Filter, ReflowSequence};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Aliasing {
    Explicit,
    Implicit,
}

#[derive(Debug, Clone)]
pub struct RuleAL01 {
    aliasing: Aliasing,
    target_parent_types: &'static [&'static str],
}

impl RuleAL01 {
    pub fn aliasing(mut self, aliasing: Aliasing) -> Self {
        self.aliasing = aliasing;
        self
    }

    pub fn target_parent_types(mut self, target_parent_types: &'static [&'static str]) -> Self {
        self.target_parent_types = target_parent_types;
        self
    }
}

impl Default for RuleAL01 {
    fn default() -> Self {
        Self {
            aliasing: Aliasing::Explicit,
            target_parent_types: &["from_expression_element", "merge_statement"],
        }
    }
}

impl Rule for RuleAL01 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> ErasedRule {
        let aliasing = match config.get("aliasing").unwrap().as_string().unwrap() {
            "explicit" => Aliasing::Explicit,
            "implicit" => Aliasing::Implicit,
            _ => unreachable!(),
        };

        RuleAL01 { aliasing, target_parent_types: &["from_expression_element", "merge_statement"] }
            .erased()
    }

    fn name(&self) -> &'static str {
        "aliasing.table"
    }

    fn description(&self) -> &'static str {
        "Implicit/explicit aliasing of table."
    }

    fn eval(&self, rule_cx: RuleContext) -> Vec<LintResult> {
        let last_seg = rule_cx.parent_stack.last().unwrap();
        let last_seg_ty = last_seg.get_type();

        if self.target_parent_types.iter().any(|&it| last_seg_ty == it) {
            let as_keyword = rule_cx
                .segment
                .segments()
                .iter()
                .find(|seg| seg.get_raw_upper() == Some("AS".into()))
                .cloned();

            if let Some(as_keyword) = as_keyword
                && self.aliasing == Aliasing::Implicit
            {
                return vec![LintResult::new(
                    as_keyword.clone().into(),
                    ReflowSequence::from_around_target(
                        &as_keyword,
                        rule_cx.parent_stack[0].clone(),
                        "both",
                        rule_cx.config.unwrap(),
                    )
                    .without(&as_keyword)
                    .respace(false, Filter::All)
                    .fixes(),
                    None,
                    None,
                    None,
                )];
            } else if self.aliasing != Aliasing::Implicit {
                let identifier = rule_cx
                    .segment
                    .get_raw_segments()
                    .iter()
                    .find(|seg| seg.is_code())
                    .expect("Failed to find identifier. Raise this as a bug on GitHub.")
                    .clone();

                return vec![LintResult::new(
                    rule_cx.segment.clone().into(),
                    ReflowSequence::from_around_target(
                        &identifier,
                        rule_cx.parent_stack[0].clone(),
                        "before",
                        rule_cx.config.unwrap(),
                    )
                    .insert(
                        KeywordSegment::new("AS".into(), None).to_erased_segment(),
                        identifier.clone(),
                        "before",
                    )
                    .respace(false, Filter::All)
                    .fixes(),
                    None,
                    None,
                    None,
                )];
            }
        }

        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["alias_expression"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;
    use crate::rules::aliasing::AL01::{Aliasing, RuleAL01};

    #[test]
    fn test_fail_default_explicit() {
        let sql = "select foo.bar from table1 foo";
        let result = fix(sql.to_string(), vec![RuleAL01::default().erased()]);

        assert_eq!(result, "select foo.bar from table1 AS foo");
    }

    #[test]
    fn test_fail_explicit() {
        let sql = "select foo.bar from table1 foo";
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Explicit).erased()]);

        assert_eq!(result, "select foo.bar from table1 AS foo");
    }

    #[test]
    fn test_fail_implicit() {
        let sql = "select foo.bar from table1 AS foo";
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Implicit).erased()]);

        assert_eq!(result, "select foo.bar from table1 foo");
    }

    #[test]
    fn test_fail_implicit_alias() {
        let sql = "select foo.bar from (select 1 as bar)foo";
        let result = fix(sql.to_string(), vec![RuleAL01::default().erased()]);

        assert_eq!(result, "select foo.bar from (select 1 as bar) AS foo");
    }

    #[test]
    fn test_fail_implicit_alias_space() {
        let sql = "select foo.bar from (select 1 as bar) foo";
        let result = fix(sql.to_string(), vec![RuleAL01::default().erased()]);

        assert_eq!(result, "select foo.bar from (select 1 as bar) AS foo");
    }

    #[test]
    fn test_fail_implicit_alias_explicit() {
        let sql = "select foo.bar from (select 1 as bar) foo";
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Explicit).erased()]);

        assert_eq!(result, "select foo.bar from (select 1 as bar) AS foo");
    }

    #[test]
    fn test_fail_implicit_alias_implicit() {
        let sql = "select foo.bar from (select 1 as bar) AS foo";
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Implicit).erased()]);

        assert_eq!(result, "select foo.bar from (select 1 as bar) foo");
    }

    #[test]
    fn test_fail_implicit_alias_implicit_multiple() {
        let sql = "select foo.bar from (select 1 as bar) AS bar, (select 1 as foo) AS foo";
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Implicit).erased()]);

        assert_eq!(result, "select foo.bar from (select 1 as bar) bar, (select 1 as foo) foo");
    }

    #[test]
    fn test_fail_implicit_alias_implicit_newline() {
        let sql = "select foo.bar from (select 1 as bar)\nAS foo";
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Implicit).erased()]);

        assert_eq!(result, "select foo.bar from (select 1 as bar)\nfoo");
    }

    #[test]
    #[ignore = "parser bug"]
    fn test_fail_default_explicit_alias_merge() {
        let sql = "MERGE dataset.inventory t\nUSING dataset.newarrivals s\n    ON t.product = \
                   s.product\nWHEN MATCHED THEN\n    UPDATE SET quantity = t.quantity + \
                   s.quantity;";
        let result = fix(sql.to_string(), vec![RuleAL01::default().erased()]);
        assert_eq!(
            result,
            "MERGE dataset.inventory AS t\nUSING dataset.newarrivals AS s\n    ON t.product = \
             s.product\nWHEN MATCHED THEN\n    UPDATE SET quantity = t.quantity + s.quantity;"
        );
    }

    #[test]
    #[ignore = "parser bug"]
    fn test_fail_explicit_alias_merge() {
        let sql = "MERGE dataset.inventory t\nUSING dataset.newarrivals s\n    ON t.product = \
                   s.product\nWHEN MATCHED THEN\n    UPDATE SET quantity = t.quantity + \
                   s.quantity;";
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Explicit).erased()]);
        assert_eq!(
            result,
            "MERGE dataset.inventory AS t\nUSING dataset.newarrivals AS s\n    ON t.product = \
             s.product\nWHEN MATCHED THEN\n    UPDATE SET quantity = t.quantity + s.quantity;"
        );
    }

    #[test]
    fn test_pass_implicit_alias_merge() {
        let sql = "MERGE dataset.inventory t\nUSING dataset.newarrivals s\n    ON t.product = \
                   s.product\nWHEN MATCHED THEN\n    UPDATE SET quantity = t.quantity + \
                   s.quantity;";
        // This test seems to expect the same SQL to be valid under implicit aliasing
        // settings, hence no change in the result. Assuming `fix` returns the
        // original SQL if no changes are required.
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Implicit).erased()]);
        assert_eq!(result, sql);
    }

    #[test]
    fn test_alias_expression_4492() {
        let sql = "SELECT\n    voo.a\nFROM foo voo";
        let result = fix(sql.to_string(), vec![RuleAL01::default().erased()]);
        assert_eq!(result, "SELECT\n    voo.a\nFROM foo AS voo");
    }
}
