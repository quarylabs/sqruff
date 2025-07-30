use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::SegmentSeekerCrawler;
use crate::core::rules::{Erased, LintResult, RuleGroups};
use crate::define_rule;
use crate::utils::reflow::sequence::{ReflowSequence, TargetSide};

define_rule!(
    /// **Anti-pattern**
    ///
    /// In this example, if line_position = leading (or unspecified, as is the default), then the operator + should not be at the end of the second line.
    ///
    /// ```sql
    /// SELECT
    ///     a +
    ///     b
    /// FROM foo
    /// ```
    ///
    /// **Best practice**
    ///
    /// If line_position = leading (or unspecified, as this is the default), place the operator after the newline.
    ///
    /// ```sql
    /// SELECT
    ///     a
    ///     + b
    /// FROM foo
    /// ```
    ///
    /// If line_position = trailing, place the operator before the newline.
    ///
    /// ```sql
    /// SELECT
    ///     a +
    ///     b
    /// FROM foo
    /// ```
    pub struct RuleLT03 {};

    name = "layout.operators";
    description = "Operators should follow a standard for being before/after newlines.";
    groups = [RuleGroups::All, RuleGroups::Layout];
    eval = eval;
    load_from_config = load_from_config;
    is_fix_compatible = true;
    crawl_behaviour = SegmentSeekerCrawler::new(
        const { SyntaxSet::new(&[SyntaxKind::BinaryOperator, SyntaxKind::ComparisonOperator]) }
    );
);

fn eval(context: &RuleContext) -> Vec<LintResult> {
    if context.segment.is_type(SyntaxKind::ComparisonOperator) {
        let comparison_positioning =
            context.config.raw["layout"]["type"]["comparison_operator"]["line_position"]
                .as_string()
                .unwrap();

        if check_trail_lead_shortcut(
            &context.segment,
            context.parent_stack.last().unwrap(),
            comparison_positioning,
        ) {
            return vec![LintResult::new(None, Vec::new(), None, None)];
        }
    } else if context.segment.is_type(SyntaxKind::BinaryOperator) {
        let binary_positioning =
            context.config.raw["layout"]["type"]["binary_operator"]["line_position"]
                .as_string()
                .unwrap();

        if check_trail_lead_shortcut(
            &context.segment,
            context.parent_stack.last().unwrap(),
            binary_positioning,
        ) {
            return vec![LintResult::new(None, Vec::new(), None, None)];
        }
    }

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
    Ok(RuleLT03 {}.erased())
}

pub(crate) fn check_trail_lead_shortcut(
    segment: &ErasedSegment,
    parent: &ErasedSegment,
    line_position: &str,
) -> bool {
    let idx = parent
        .segments()
        .iter()
        .position(|it| it == segment)
        .unwrap();

    // Shortcut #1: Leading.
    if line_position == "leading" {
        if seek_newline(parent.segments(), idx, Direction::Backward) {
            return true;
        }
        // If we didn't find a newline before, if there's _also_ not a newline
        // after, then we can also shortcut. i.e., it's a comma "mid line".
        if !seek_newline(parent.segments(), idx, Direction::Forward) {
            return true;
        }
    }
    // Shortcut #2: Trailing.
    else if line_position == "trailing" {
        if seek_newline(parent.segments(), idx, Direction::Forward) {
            return true;
        }
        // If we didn't find a newline after, if there's _also_ not a newline
        // before, then we can also shortcut. i.e., it's a comma "mid line".
        if !seek_newline(parent.segments(), idx, Direction::Backward) {
            return true;
        }
    }

    false
}

fn seek_newline(segments: &[ErasedSegment], idx: usize, direction: Direction) -> bool {
    let segments: &mut dyn Iterator<Item = _> = match direction {
        Direction::Forward => &mut segments[idx + 1..].iter(),
        Direction::Backward => &mut segments.iter().take(idx).rev(),
    };

    for segment in segments {
        if segment.is_type(SyntaxKind::Newline) {
            return true;
        } else if !segment.is_type(SyntaxKind::Whitespace)
            && !segment.is_type(SyntaxKind::Indent)
            && !segment.is_type(SyntaxKind::Implicit)
            && !segment.is_type(SyntaxKind::Comment)
            && !segment.is_type(SyntaxKind::InlineComment)
            && !segment.is_type(SyntaxKind::BlockComment)
        {
            break;
        }
    }

    false
}

#[derive(Debug)]
enum Direction {
    Forward,
    Backward,
}