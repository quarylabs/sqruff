use crate::core::parser::segments::base::Segment;
use std::ops::Deref;

pub fn join_segments_raw(segments: Vec<Box<dyn Segment>>) -> String {
    segments
        .iter()
        .filter_map(|s| s.get_raw())
        .collect::<Vec<_>>()
        .concat()
}

/// Take segments and split off surrounding non-code segments as appropriate.
///
/// We use slices to avoid creating too many unnecessary Vecs.
pub fn trim_non_code_segments(
    segments: Vec<Box<dyn Segment>>,
) -> (
    Vec<Box<dyn Segment>>,
    Vec<Box<dyn Segment>>,
    Vec<Box<dyn Segment>>,
) {
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

    (
        segments[..pre_idx].iter().cloned().collect(),
        segments[pre_idx..post_idx].iter().cloned().collect(),
        segments[post_idx..].iter().cloned().collect(),
    )
}

// TODO Implement these tests
// mod test {
//     use crate::core::parser::helpers::trim_non_code_segments;
//     use crate::core::parser::markers::PositionMarker;
//     use crate::core::parser::segments::base::Segment;
//     use crate::core::templaters::base::TemplatedFile;
//
//     fn generate_test_segments(elems: Vec<&str>) -> Vec<dyn Segment> {
//         let mut buff = Vec::new();
//         let raw_file: String = elems.concat();
//         let templated_file = TemplatedFile::from_string(raw_file); // Assuming TemplatedFile has a from_string method
//         let mut idx = 0;
//
//         for elem in elems {
//             match elem {
//                 "<indent>" => buff.push(Segment::Indent(PositionMarker { start: idx, end: idx, templated_file: templated_file.clone() })),
//                 "<dedent>" => buff.push(Segment::Dedent(PositionMarker { start: idx, end: idx, templated_file: templated_file.clone() })),
//                 _ => {
//                     let (seg_class, seg_type) = if elem.chars().all(|c| c.is_whitespace()) {
//                         (Segment::Whitespace, "whitespace")
//                     } else if elem == "\n" {
//                         (Segment::Newline, "newline")
//                     } else {
//                         match elem {
//                             "(" => (Segment::Symbol, "bracket_open"),
//                             ")" => (Segment::Symbol, "bracket_close"),
//                             _ if elem.starts_with("--") => (Segment::Comment, "inline_comment"),
//                             _ if elem.starts_with('"') => (Segment::Code, "double_quote"),
//                             _ if elem.starts_with('\'') => (Segment::Code, "single_quote"),
//                             _ => (Segment::Code, "code"),
//                         }
//                     };
//
//                     buff.push(seg_class(PositionMarker { start: idx, end: idx + elem.len(), templated_file: templated_file.clone() }, seg_type.to_string()));
//                     idx += elem.len();
//                 }
//             }
//         }
//
//         buff
//     }
//
//     #[test]
//     fn test_trim_non_code_segments() {
//         let test_cases = vec![
//             (vec!["bar", ".", "bar"], 0, 3, 0),
//             (vec![], 0, 0, 0),
//             (vec!["  ", "\n", "\t", "bar", ".", "bar", "  ", "\n", "\t"], 3, 3, 3),
//         ];
//
//         for (token_list, pre_len, mid_len, post_len) in test_cases {
//             let seg_list = generate_test_segments(token_list);
//             let (pre, mid, post) = trim_non_code_segments(&seg_list);
//
//             // Assert lengths
//             assert_eq!((pre.len(), mid.len(), post.len()), (pre_len, mid_len, post_len));
//
//             // Assert content
//             let pre_raw: Vec<_> = pre.iter().map(|s| s.raw()).collect();
//             assert_eq!(pre_raw, seg_list[..pre_len].iter().map(|s| s.raw()).collect::<Vec<_>>());
//
//             let mid_raw: Vec<_> = mid.iter().map(|s| s.raw()).collect();
//             assert_eq!(mid_raw, seg_list[pre_len..pre_len + mid_len].iter().map(|s| s.raw()).collect::<Vec<_>>());
//
//             let post_raw: Vec<_> = post.iter().map(|s| s.raw()).collect();
//             assert_eq!(post_raw, seg_list[seg_list.len() - post_len..].iter().map(|s| s.raw()).collect::<Vec<_>>());
//         }
//     }
// }
