use smol_str::SmolStr;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::rules::base::LintResult;

/// Return a list of fixes from an iterable of LintResult.
pub fn fixes_from_results(
    results: impl Iterator<Item = LintResult>,
) -> impl Iterator<Item = LintFix> {
    results.flat_map(|result| result.fixes)
}

pub fn pretty_segment_name(segment: &ErasedSegment) -> String {
    if segment.is_type(SyntaxKind::Symbol) {
        format!(
            "{} {:?}",
            segment.get_type().as_str().replace('_', " "),
            segment.raw()
        )
    } else if segment.is_type(SyntaxKind::Keyword) {
        format!("{:?} keyword", segment.raw())
    } else {
        segment.get_type().as_str().replace('_', " ")
    }
}

/// Given a raw segment, deduce the indent of its line.
pub fn deduce_line_indent(raw_segment: &ErasedSegment, root_segment: &ErasedSegment) -> SmolStr {
    let seg_idx = root_segment
        .get_raw_segments()
        .iter()
        .position(|seg| seg == raw_segment);
    let mut indent_seg = None;
    let raw_segments = root_segment.get_raw_segments();
    if let Some(idx) = seg_idx {
        for seg in raw_segments[..=idx].iter().rev() {
            if seg.is_code() {
                indent_seg = None;
            } else if seg.is_type(SyntaxKind::Whitespace) {
                indent_seg = Some(seg);
            } else if seg.is_type(SyntaxKind::Newline) {
                break;
            }
        }
    }

    indent_seg.map(|seg| seg.raw().clone()).unwrap_or_default()
}
