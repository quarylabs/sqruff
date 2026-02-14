use smol_str::StrExt;

use super::segments::ErasedSegment;
use crate::dialects::syntax::SyntaxSet;

pub(crate) fn skip_start_index_forward_to_code(
    segments: &[ErasedSegment],
    start_idx: u32,
    max_idx: u32,
) -> u32 {
    let mut idx = start_idx;
    while idx < max_idx {
        if segments[idx as usize].is_code() {
            break;
        }
        idx += 1;
    }
    idx
}

pub(crate) fn skip_stop_index_backward_to_code(
    segments: &[ErasedSegment],
    stop_idx: u32,
    min_idx: u32,
) -> u32 {
    let mut idx = stop_idx;
    while idx > min_idx {
        if segments[idx as usize - 1].is_code() {
            break;
        }
        idx -= 1;
    }
    idx
}

pub(crate) fn first_trimmed_raw(seg: &ErasedSegment) -> String {
    seg.raw()
        .to_uppercase_smolstr()
        .split(char::is_whitespace)
        .next()
        .map(ToString::to_string)
        .unwrap_or_default()
}

pub(crate) fn first_non_whitespace(
    segments: &[ErasedSegment],
    start_idx: u32,
) -> Option<(String, &SyntaxSet)> {
    for segment in segments.iter().skip(start_idx as usize) {
        if let Some(raw) = segment.first_non_whitespace_segment_raw_upper() {
            return Some((raw, segment.class_types()));
        }
    }

    None
}
