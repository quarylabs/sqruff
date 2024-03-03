use crate::core::parser::segments::base::Segment;

pub fn join_segments_raw(segments: &[Box<dyn Segment>]) -> String {
    segments.iter().filter_map(|s| s.get_raw()).collect::<Vec<_>>().concat()
}

pub fn check_still_complete(
    segments_in: &[Box<dyn Segment>],
    matched_segments: &[Box<dyn Segment>],
    unmatched_segments: &[Box<dyn Segment>],
) {
    let initial_str = join_segments_raw(segments_in);
    let current_str = join_segments_raw(&[matched_segments, unmatched_segments].concat());

    if initial_str != current_str {
        panic!("Parse completeness check fail: {current_str:?} != {initial_str:?}")
    }
}

/// Take segments and split off surrounding non-code segments as appropriate.
pub fn trim_non_code_segments(
    segments: &[Box<dyn Segment>],
) -> (&[Box<dyn Segment>], &[Box<dyn Segment>], &[Box<dyn Segment>]) {
    let seg_len = segments.len();
    let mut pre_idx = 0;
    let mut post_idx = seg_len;

    if !segments.is_empty() {
        // Trim the start
        while pre_idx < seg_len && !segments[pre_idx].is_code() {
            pre_idx += 1;
        }

        // Trim the end
        while post_idx > pre_idx && !segments[post_idx - 1].is_code() {
            post_idx -= 1;
        }
    }

    (&segments[..pre_idx], &segments[pre_idx..post_idx], &segments[post_idx..])
}

#[cfg(test)]
mod test {
    use crate::core::parser::helpers::trim_non_code_segments;
    use crate::core::parser::segments::test_functions::generate_test_segments_func;

    #[test]
    fn test__parser__helper_trim_non_code_segments() {
        let test_cases = vec![
            (vec!["bar", ".", "bar"], 0, 3, 0),
            (vec![], 0, 0, 0),
            (vec!["  ", "\n", "\t", "bar", ".", "bar", "  ", "\n", "\t"], 3, 3, 3),
        ];

        for (token_list, pre_len, mid_len, post_len) in test_cases {
            let seg_list = generate_test_segments_func(token_list);
            let (pre, mid, post) = trim_non_code_segments(&seg_list);

            // Assert lengths
            assert_eq!((pre.len(), mid.len(), post.len()), (pre_len, mid_len, post_len));

            // Assert content
            let pre_raw: Vec<_> = pre.iter().map(|s| s.get_raw()).collect();
            assert_eq!(
                pre_raw,
                seg_list[..pre_len].iter().map(|s| s.get_raw()).collect::<Vec<_>>()
            );

            let mid_raw: Vec<_> = mid.iter().map(|s| s.get_raw()).collect();
            assert_eq!(
                mid_raw,
                seg_list[pre_len..pre_len + mid_len]
                    .iter()
                    .map(|s| s.get_raw())
                    .collect::<Vec<_>>()
            );

            let post_raw: Vec<_> = post.iter().map(|s| s.get_raw()).collect();
            assert_eq!(
                post_raw,
                seg_list[seg_list.len() - post_len..]
                    .iter()
                    .map(|s| s.get_raw())
                    .collect::<Vec<_>>()
            );
        }
    }
}
