use itertools::Itertools;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::SegmentSeekerCrawler;
use crate::core::rules::{Erased, LintResult, RuleGroups};
use crate::define_rule;
use crate::utils::functional::context::FunctionalContext;

define_rule!(
    /// **Anti-pattern**
    ///
    /// In this example, there is a space between the function and the parenthesis.
    ///
    /// ```sql
    /// SELECT
    ///     sum (a)
    /// FROM foo
    /// ```
    ///
    /// **Best practice**
    ///
    /// Remove the space between the function and the parenthesis.
    ///
    /// ```sql
    /// SELECT
    ///     sum(a)
    /// FROM foo
    /// ```
    pub struct RuleLT06 {};

    name = "layout.functions";
    description = "Function name not immediately followed by parenthesis.";
    groups = [RuleGroups::All, RuleGroups::Core, RuleGroups::Layout];
    eval = eval;
    load_from_config = load_from_config;
    is_fix_compatible = true;
    crawl_behaviour = SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Function]) });
);

fn eval(context: &RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(context).segment();
        let children = segment.children(None);

        let function_name = children
            .find_first(Some(|segment: &ErasedSegment| {
                segment.is_type(SyntaxKind::FunctionName)
            }))
            .pop();
        let function_contents = children
            .find_first(Some(|segment: &ErasedSegment| {
                segment.is_type(SyntaxKind::FunctionContents)
            }))
            .pop();

        let mut intermediate_segments = children.select::<fn(&ErasedSegment) -> bool>(
            None,
            None,
            Some(&function_name),
            Some(&function_contents),
        );

        if !intermediate_segments.is_empty() {
            return if intermediate_segments.all(Some(|seg| {
                matches!(seg.get_type(), SyntaxKind::Whitespace | SyntaxKind::Newline)
            })) {
                vec![LintResult::new(
                    intermediate_segments.first().cloned(),
                    intermediate_segments
                        .into_iter()
                        .map(LintFix::delete)
                        .collect_vec(),
                    None,
                    None,
                )]
            } else {
                vec![LintResult::new(
                    intermediate_segments.pop().into(),
                    vec![],
                    None,
                    None,
                )]
            };
        }

        vec![]
}

fn load_from_config(
    _config: &ahash::AHashMap<String, crate::core::config::Value>,
) -> Result<crate::core::rules::ErasedRule, String> {
    Ok(RuleLT06 {}.erased())
}
