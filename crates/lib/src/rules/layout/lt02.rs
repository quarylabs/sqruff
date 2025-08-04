use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::RootOnlyCrawler;
use crate::core::rules::{Erased, LintResult, RuleGroups};
use crate::define_rule;
use crate::utils::reflow::sequence::ReflowSequence;

define_rule!(
    /// **Anti-pattern**
    ///
    /// The `•` character represents a space and the `→` character represents a tab.
    /// In this example, the third line contains five spaces instead of four and
    /// the second line contains two spaces and one tab.
    ///
    /// ```sql
    /// SELECT
    /// ••→a,
    /// •••••b
    /// FROM foo
    /// ```
    ///
    /// **Best practice**
    ///
    /// Change the indentation to use a multiple of four spaces. This example also assumes that the indent_unit config value is set to space. If it had instead been set to tab, then the indents would be tabs instead.
    ///
    /// ```sql
    /// SELECT
    /// ••••a,
    /// ••••b
    /// FROM foo
    /// ```
    pub struct RuleLT02 {};

    name = "layout.indent";
    description = "Incorrect Indentation.";
    groups = [RuleGroups::All, RuleGroups::Core, RuleGroups::Layout];
    eval = eval;
    load_from_config = load_from_config;
    is_fix_compatible = true;
    crawl_behaviour = RootOnlyCrawler;
);

fn eval(context: &RuleContext) -> Vec<LintResult> {
    ReflowSequence::from_root(context.segment.clone(), context.config)
        .reindent(context.tables)
        .results()
}

fn load_from_config(
    _config: &ahash::AHashMap<String, crate::core::config::Value>,
) -> Result<crate::core::rules::ErasedRule, String> {
    Ok(RuleLT02 {}.erased())
}