use std::collections::HashSet;

use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{BaseCrawler, SegmentSeekerCrawler};
use crate::helpers::Boxed;
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Debug, PartialEq, Eq)]
pub enum Aliasing {
    Explicit,
    Implicit,
}

#[derive(Debug)]
pub struct RuleAL01 {
    aliasing: Aliasing,
}

impl RuleAL01 {
    pub fn aliasing(mut self, aliasing: Aliasing) -> Self {
        self.aliasing = aliasing;
        self
    }
}

impl Default for RuleAL01 {
    fn default() -> Self {
        Self { aliasing: Aliasing::Explicit }
    }
}

impl Rule for RuleAL01 {
    fn eval(&self, rule_cx: RuleContext) -> Vec<LintResult> {
        let last_seg = rule_cx.parent_stack.last().unwrap();
        let last_seg_ty = last_seg.get_type();

        if matches!(last_seg_ty, "from_expression_element" | "merge_statement") {
            let as_keyword = rule_cx
                .segment
                .get_segments()
                .iter()
                .find(|seg| seg.get_raw_upper() == Some("AS".into()))
                .cloned();

            if let Some(as_keyword) = as_keyword
                && self.aliasing == Aliasing::Implicit
            {
                return vec![LintResult::new(
                    as_keyword.clone().into(),
                    ReflowSequence::from_around_target(
                        as_keyword.clone(),
                        rule_cx.parent_stack[0].clone(),
                        "both",
                    )
                    .without(&as_keyword)
                    .respace()
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
                        identifier.clone(),
                        rule_cx.parent_stack[0].clone(),
                        "before",
                    )
                    .insert(
                        KeywordSegment::new("AS".into(), None).boxed(),
                        identifier.clone(),
                        "before",
                    )
                    .respace()
                    .fixes(),
                    None,
                    None,
                    None,
                )];
            }
        }

        Vec::new()
    }

    fn crawl_behaviour(&self) -> Box<dyn BaseCrawler> {
        SegmentSeekerCrawler::new(HashSet::from(["alias_expression"])).boxed()
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
    fn test_fail_implicit() {
        let sql = "select foo.bar from table1 AS foo";
        let result =
            fix(sql.to_string(), vec![RuleAL01::default().aliasing(Aliasing::Implicit).erased()]);

        assert_eq!(result, "select foo.bar from table1 foo");
    }
}
