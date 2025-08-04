use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::SegmentSeekerCrawler;
use crate::core::rules::{Erased, LintResult, RuleGroups};
use crate::define_rule;
use crate::utils::reflow::sequence::{ReflowSequence, TargetSide};

define_rule!(
    /// **Anti-pattern**
    ///
    /// In this example, `UNION ALL` is not on a line itself.
    ///
    /// ```sql
    /// SELECT 'a' AS col UNION ALL
    /// SELECT 'b' AS col
    /// ```
    ///
    /// **Best practice**
    ///
    /// Place `UNION ALL` on its own line.
    ///
    /// ```sql
    /// SELECT 'a' AS col
    /// UNION ALL
    /// SELECT 'b' AS col
    /// ```
    pub struct RuleLT11 {};

    name = "layout.set_operators";
    description = "Set operators should be surrounded by newlines.";
    groups = [RuleGroups::All, RuleGroups::Core, RuleGroups::Layout];
    eval = eval;
    load_from_config = load_from_config;
    is_fix_compatible = true;
    crawl_behaviour = SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SetOperator]) });
);

fn eval(context: &RuleContext) -> Vec<LintResult> {
    ReflowSequence::from_around_target(
        &context.segment,
        context.parent_stack.first().unwrap().clone(),
        TargetSide::Both,
        context.config,
    )
    .rebreak(context.tables)
    .results()
}

fn load_from_config(
    _config: &ahash::AHashMap<String, crate::core::config::Value>,
) -> Result<crate::core::rules::ErasedRule, String> {
    Ok(RuleLT11 {}.erased())
}