use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::lt03::check_trail_lead_shortcut;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::SegmentSeekerCrawler;
use crate::core::rules::{Erased, LintResult, RuleGroups};
use crate::define_rule;
use crate::utils::reflow::sequence::{ReflowSequence, TargetSide};

define_rule!(
    /// **Anti-pattern**
    ///
    /// There is a mixture of leading and trailing commas.
    ///
    /// ```sql
    /// SELECT
    ///     a
    ///     , b,
    ///     c
    /// FROM foo
    /// ```
    ///
    /// **Best practice**
    ///
    /// By default, sqruff prefers trailing commas. However it is configurable for leading commas. The chosen style must be used consistently throughout your SQL.
    ///
    /// ```sql
    /// SELECT
    ///     a,
    ///     b,
    ///     c
    /// FROM foo
    ///
    /// -- Alternatively, set the configuration file to 'leading'
    /// -- and then the following would be acceptable:
    ///
    /// SELECT
    ///     a
    ///     , b
    ///     , c
    /// FROM foo
    /// ```
    pub struct RuleLT04 {};

    name = "layout.commas";
    description = "Leading/Trailing comma enforcement.";
    groups = [RuleGroups::All, RuleGroups::Layout];
    eval = eval;
    load_from_config = load_from_config;
    is_fix_compatible = true;
    crawl_behaviour = SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Comma]) });
);

fn eval(context: &RuleContext) -> Vec<LintResult> {
    let comma_positioning = context.config.raw["layout"]["type"]["comma"]["line_position"]
        .as_string()
        .unwrap();

    if check_trail_lead_shortcut(
        &context.segment,
        context.parent_stack.last().unwrap(),
        comma_positioning,
    ) {
        return vec![LintResult::new(None, Vec::new(), None, None)];
    };

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
    Ok(RuleLT04 {}.erased())
}