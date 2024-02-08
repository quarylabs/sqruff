use std::collections::HashSet;

use crate::core::parser::segments::base::Segment;
use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;
use crate::utils::functional::segments::Segments;

struct SelectTargetsInfo {
    select_idx: Option<usize>,
    first_new_line_idx: Option<usize>,
    first_select_target_idx: Option<usize>,
    first_whitespace_idx: Option<usize>,
    comment_after_select_idx: Option<usize>,
    select_targets: Segments,
    from_segment: Option<Box<dyn Segment>>,
    pre_from_whitespace: Segments,
}

#[derive(Debug)]
pub struct RuleLT09 {
    wildcard_policy: &'static str,
}

impl Rule for RuleLT09 {
    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(HashSet::from(["select_clause".into()])).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let select_targets_info = Self::get_indexes(context.clone());
        let select_clause = FunctionalContext::new(context.clone());

        // let wildcards = select_clause
        //     .children(sp.is_type("select_clause_element"))
        //     .children(sp.is_type("wildcard_expression"));
        let has_wildcard = false;

        if select_targets_info.select_targets.len() == 1 && !has_wildcard
            || self.wildcard_policy == "single"
        {
            return self.eval_single_select_target_element(select_targets_info, context);
        } else {
            unimplemented!()
        }

        unimplemented!()
    }
}

impl RuleLT09 {
    fn get_indexes(context: RuleContext) -> SelectTargetsInfo {
        let children = FunctionalContext::new(context.clone()).segment().children(None);

        let select_targets = children.select(
            Some(|segment| segment.is_type("select_clause_element")),
            None,
            None,
            None,
        );

        let first_select_target_idx = children.find(select_targets.get(0, None).unwrap().as_ref());

        let selects = children.select(
            Some(|segment| {
                segment.get_type() == "keyword"
                    && segment.get_raw().unwrap().to_lowercase() == "select"
            }),
            None,
            None,
            None,
        );

        let select_idx = (!selects.is_empty())
            .then(|| children.find(selects.get(0, None).unwrap().as_ref()).unwrap());

        let newlines = children.select(Some(|it| it.is_type("newline")), None, None, None);

        let first_new_line_idx = (!newlines.is_empty())
            .then(|| children.find(newlines.get(0, None).unwrap().as_ref()).unwrap());

        if !newlines.is_empty() {
            unimplemented!()
        }

        if let Some(first_new_line_idx) = first_new_line_idx {
            unimplemented!()
        }

        let siblings_post = FunctionalContext::new(context).siblings_post();
        let from_segment = siblings_post
            .find_first(Some(|seg: &dyn Segment| seg.is_type("from_clause")))
            .find_first::<fn(&dyn Segment) -> bool>(None)
            .get(0, None);
        let pre_from_whitespace = siblings_post.select(
            Some(|seg| seg.is_type("whitespace")),
            None,
            None,
            from_segment.as_deref(),
        );

        SelectTargetsInfo {
            select_idx,
            first_new_line_idx,
            first_select_target_idx,
            first_whitespace_idx: None,
            comment_after_select_idx: None,
            select_targets,
            from_segment,
            pre_from_whitespace,
        }
    }

    fn eval_single_select_target_element(
        &self,
        select_targets_info: SelectTargetsInfo,
        context: RuleContext,
    ) -> Vec<LintResult> {
        let select_clause = FunctionalContext::new(context.clone()).segment();
        let parent_stack = context.parent_stack;

        if !(select_targets_info.select_idx < select_targets_info.first_new_line_idx
            && select_targets_info.first_new_line_idx < select_targets_info.first_select_target_idx)
        {
            return Vec::new();
        }

        unimplemented!()
    }
}

impl Default for RuleLT09 {
    fn default() -> Self {
        Self { wildcard_policy: "single" }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT09::RuleLT09;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT09::default().erased()]
    }

    #[test]
    fn test_single_select_target_and_no_newline_between_select_and_select_target() {
        let violations =
            lint("select a from x".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }
}
