use super::segments::base::ErasedSegment;

pub fn join_segments_raw(segments: &[ErasedSegment]) -> String {
    segments.iter().map(|s| s.raw()).collect::<Vec<_>>().concat()
}

pub fn check_still_complete(
    segments_in: &[ErasedSegment],
    matched_segments: &[ErasedSegment],
    unmatched_segments: &[ErasedSegment],
) {
    let initial_str = join_segments_raw(segments_in);
    let current_str = join_segments_raw(&[matched_segments, unmatched_segments].concat());

    pretty_assertions::assert_eq!(initial_str, current_str);
}
