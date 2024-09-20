use smol_str::SmolStr;

use super::segments::base::ErasedSegment;

pub(crate) fn join_segments_raw(segments: &[ErasedSegment]) -> SmolStr {
    SmolStr::from_iter(segments.iter().map(|s| s.raw().as_str()))
}

pub(crate) fn check_still_complete(
    segments_in: &[ErasedSegment],
    matched_segments: &[ErasedSegment],
    unmatched_segments: &[ErasedSegment],
) {
    let initial_str = join_segments_raw(segments_in);
    let current_str = join_segments_raw(&[matched_segments, unmatched_segments].concat());

    pretty_assertions::assert_eq!(initial_str, current_str);
}
