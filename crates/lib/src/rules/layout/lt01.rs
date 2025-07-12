use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::RootOnlyCrawler;
use crate::core::rules::{Erased, LintResult, RuleGroups};
use crate::define_rule;
use crate::utils::reflow::sequence::{Filter, ReflowSequence};

define_rule!(
    /// **Anti-pattern**
    ///
    /// In this example, spacing is all over the place and is represented by `•`.
    ///
    /// ```sql
    /// SELECT
    ///     a,        b(c) as d••
    /// FROM foo••••
    /// JOIN bar USING(a)
    /// ```
    ///
    /// **Best practice**
    ///
    /// - Unless an indent or preceding a comment, whitespace should be a single space.
    /// - There should also be no trailing whitespace at the ends of lines.
    /// - There should be a space after USING so that it’s not confused for a function.
    ///
    /// ```sql
    /// SELECT
    ///     a, b(c) as d
    /// FROM foo
    /// JOIN bar USING (a)
    /// ```
    pub struct RuleLT01 {};

    name = "layout.spacing";
    description = "Inappropriate Spacing.";
    groups = [RuleGroups::All, RuleGroups::Core, RuleGroups::Layout];
    eval = eval;
    load_from_config = load_from_config;
    is_fix_compatible = true;
    crawl_behaviour = RootOnlyCrawler;
);

fn eval(context: &RuleContext) -> Vec<LintResult> {
    let sequence = ReflowSequence::from_root(context.segment.clone(), context.config);
    sequence
        .respace(context.tables, false, Filter::All)
        .results()
}

fn load_from_config(
    _config: &ahash::AHashMap<String, crate::core::config::Value>,
) -> Result<crate::core::rules::ErasedRule, String> {
    Ok(RuleLT01 {}.erased())
}
