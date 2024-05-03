use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::{
    ErasedSegment, SymbolSegment, SymbolSegmentNewArgs, WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;
use crate::utils::functional::segments::Segments;
use crate::utils::reflow::sequence::{Filter, ReflowSequence};

#[derive(Debug, Default, Clone)]
pub struct RuleST08;

impl RuleST08 {
    pub fn remove_unneeded_brackets(
        &self,
        context: RuleContext,
        bracketed: Segments,
    ) -> (ErasedSegment, ReflowSequence) {
        let anchor = &bracketed.get(0, None).unwrap();
        let seq = ReflowSequence::from_around_target(
            anchor,
            context.parent_stack[0].clone(),
            "before",
            context.config.unwrap(),
        )
        .replace(
            anchor.clone(),
            &Self::filter_meta(&anchor.segments()[1..anchor.segments().len() - 1], false),
        );

        (anchor.clone(), seq)
    }

    pub fn filter_meta(segments: &[ErasedSegment], keep_meta: bool) -> Vec<ErasedSegment> {
        segments.iter().filter(|&elem| elem.is_meta() == keep_meta).cloned().collect()
    }
}

impl Rule for RuleST08 {
    fn name(&self) -> &'static str {
        "structure.distinct"
    }

    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleST08.erased()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["select_clause", "function"].into()).into()
    }

    fn description(&self) -> &'static str {
        "Looking for DISTINCT before a bracket"
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut seq: Option<ReflowSequence> = None;
        let mut anchor: Option<ErasedSegment> = None;
        let children = FunctionalContext::new(context.clone()).segment().children(None);

        if context.segment.is_type("select_clause") {
            let modifier =
                children.select(Some(|it| it.is_type("select_clause_modifier")), None, None, None);
            let selected_elements =
                children.select(Some(|it| it.is_type("select_clause_element")), None, None, None);
            let first_element = selected_elements.find_first::<fn(&_) -> _>(None);
            let expression = first_element
                .children(Some(|it| it.is_type("expression")))
                .find_first::<fn(&_) -> _>(None);
            let expression = if expression.is_empty() { first_element } else { expression };
            let bracketed = expression
                .children(Some(|it| it.get_type() == "bracketed"))
                .find_first::<fn(&_) -> _>(None);

            if !modifier.is_empty() && !bracketed.is_empty() {
                if expression[0].segments().len() == 1 {
                    let ret = self.remove_unneeded_brackets(context.clone(), bracketed);
                    anchor = ret.0.into();
                    seq = ret.1.into();
                } else {
                    anchor = Some(modifier[0].clone());
                    seq = Some(ReflowSequence::from_around_target(
                        &modifier[0],
                        context.parent_stack[0].clone(),
                        "after",
                        context.config.unwrap(),
                    ));
                }
            }
        } else if context.segment.is_type("function") {
            let anchor = context.parent_stack.last().unwrap();

            if !anchor.is_type("expression") || anchor.segments().len() != 1 {
                return Vec::new();
            }

            let selected_functions =
                children.select(Some(|it| it.is_type("function_name")), None, None, None);
            let function_name = selected_functions.first();
            let bracketed = children.find_first(Some(|it: &ErasedSegment| it.is_type("bracketed")));

            if function_name.is_none()
                || function_name.unwrap().get_raw_upper() != Some(String::from("DISTINCT"))
                || bracketed.is_empty()
            {
                return Vec::new();
            }

            let bracketed = &bracketed[0];
            let mut edits = vec![
                SymbolSegment::create(
                    "DISTINCT",
                    &<_>::default(),
                    SymbolSegmentNewArgs { r#type: "function_name_identifier" },
                ),
                WhitespaceSegment::create(" ", &<_>::default(), WhitespaceSegmentNewArgs),
            ];
            edits.extend(Self::filter_meta(
                &bracketed.segments()[1..bracketed.segments().len() - 1],
                false,
            ));

            return vec![LintResult::new(
                anchor.clone().into(),
                vec![LintFix::replace(anchor.clone(), edits, None)],
                None,
                None,
                None,
            )];
        }

        if let Some(seq) = seq {
            if let Some(anchor) = anchor {
                let fixes = seq.respace(false, Filter::All).fixes();

                if !fixes.is_empty() {
                    return vec![LintResult::new(Some(anchor), fixes, None, None, None)];
                }
            }
        }

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::api::simple::{fix, lint};

    fn rules() -> Vec<ErasedRule> {
        vec![RuleST08::default().erased()]
    }

    #[test]
    fn test_fail_distinct_with_parenthesis_1() {
        let fail_str = "SELECT DISTINCT(a)";
        let fix_str = "SELECT DISTINCT a";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_distinct_with_parenthesis_2() {
        let fail_str = "SELECT DISTINCT(a + b) * c";
        let fix_str = "SELECT DISTINCT (a + b) * c";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_distinct_with_parenthesis_3() {
        let fail_str = "SELECT DISTINCT (a)";
        let fix_str = "SELECT DISTINCT a";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_distinct_with_parenthesis_4() {
        let pass_str = "SELECT DISTINCT (a + b) * c";
        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_distinct_with_parenthesis_5() {
        let fail_str = r#"SELECT DISTINCT(field_1) FROM my_table"#;
        let fix_str = "SELECT DISTINCT field_1 FROM my_table";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_distinct_with_parenthesis_6() {
        let fail_str = "SELECT DISTINCT(a), b";
        let fix_str = "SELECT DISTINCT a, b";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_pass_no_distinct() {
        let fail_str = "SELECT a, b";
        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_distinct_column_inside_count() {
        let fail_str = "SELECT COUNT(DISTINCT(unique_key))";
        let fix_str = "SELECT COUNT(DISTINCT unique_key)";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_distinct_concat_inside_count() {
        let fail_str = "SELECT COUNT(DISTINCT(CONCAT(col1, '-', col2, '-', col3)))";
        let fix_str = "SELECT COUNT(DISTINCT CONCAT(col1, '-', col2, '-', col3))";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }
}
