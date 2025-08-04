use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;

use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::SegmentSeekerCrawler;
use crate::core::rules::{Erased, LintResult, RuleGroups};
use crate::define_rule;

define_rule!(
    /// **Anti-pattern**
    ///
    /// In this example, the maximum number of empty lines inside a statement is set to 0.
    ///
    /// ```sql
    /// SELECT 'a' AS col
    /// FROM tab
    ///
    ///
    /// WHERE x = 4
    /// ORDER BY y
    ///
    ///
    /// LIMIT 5
    /// ;
    /// ```
    ///
    /// **Best practice**
    ///
    /// ```sql
    /// SELECT 'a' AS col
    /// FROM tab
    /// WHERE x = 4
    /// ORDER BY y
    /// LIMIT 5
    /// ;
    /// ```
    pub struct RuleLT15 {
        maximum_empty_lines_between_statements: usize,
        maximum_empty_lines_inside_statements: usize,
    };

    name = "layout.newlines";
    description = "Too many consecutive blank lines.";
    groups = [RuleGroups::All, RuleGroups::Layout];
    eval = eval;
    load_from_config = load_from_config;
    is_fix_compatible = true;
    crawl_behaviour = SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Newline]) });
);

fn eval(context: &RuleContext) -> Vec<LintResult> {
    if !context.segment.is_type(SyntaxKind::Newline) {
        return Vec::new();
    }

    let rule = RuleLT15 {
        maximum_empty_lines_between_statements: context.config.raw["layout"]["newlines"]["maximum_empty_lines_between_statements"]
            .as_int()
            .map(|v| v as usize)
            .unwrap_or(2),
        maximum_empty_lines_inside_statements: context.config.raw["layout"]["newlines"]["maximum_empty_lines_inside_statements"]
            .as_int()
            .map(|v| v as usize)
            .unwrap_or(1),
    };

    let inside_statement = context
        .parent_stack
        .iter()
        .any(|seg| seg.is_type(SyntaxKind::Statement));

    let maximum_empty_lines = if inside_statement {
        rule.maximum_empty_lines_inside_statements
    } else {
        rule.maximum_empty_lines_between_statements
    };

    let Some(parent) = context.parent_stack.last() else {
        return Vec::new();
    };

    let siblings = parent.segments();
    let Some(current_idx) = siblings.iter().position(|s| s == &context.segment) else {
        return Vec::new();
    };

    // Count consecutive newlines including this one
    let mut consecutive_newlines = 1;

    // Count backwards from current position
    for i in (0..current_idx).rev() {
        if siblings[i].is_type(SyntaxKind::Newline) {
            consecutive_newlines += 1;
        } else {
            break;
        }
    }

    // Too many consecutive newlines means too many empty lines
    if consecutive_newlines > maximum_empty_lines + 1 {
        return vec![LintResult::new(
            context.segment.clone().into(),
            vec![LintFix::delete(context.segment.clone())],
            None,
            None,
        )];
    }

    Vec::new()
}

fn load_from_config(
    config: &ahash::AHashMap<String, crate::core::config::Value>,
) -> Result<crate::core::rules::ErasedRule, String> {
    Ok(RuleLT15 {
        maximum_empty_lines_between_statements: config
            .get("maximum_empty_lines_between_statements")
            .and_then(|v| v.as_int())
            .map(|v| v as usize)
            .unwrap_or(2),
        maximum_empty_lines_inside_statements: config
            .get("maximum_empty_lines_inside_statements")
            .and_then(|v| v.as_int())
            .map(|v| v as usize)
            .unwrap_or(1),
    }
    .erased())
}