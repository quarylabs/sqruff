/// A templated SQL file.
///
/// This is the response of a `templater`'s `.process()` method
/// and contains both references to the original file and also
/// the capability to split up that file when lexing.
pub struct TemplatedFile;

/// Find the indices of all newlines in a string.
pub fn iter_indices_of_newlines(raw_str: &str) -> impl Iterator<Item = usize> + '_ {
    // TODO: This may be optimize-able by not doing it all up front.
    raw_str.match_indices('\n').map(|(idx, _)| idx).into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_indices_of_newlines() {
        vec![
            ("", vec![]),
            ("foo", vec![]),
            ("foo\nbar", vec![3]),
            ("\nfoo\n\nbar\nfoo\n\nbar\n", vec![0, 4, 5, 9, 13, 14, 18]),
        ]
        .into_iter()
        .for_each(|(in_str, expected)| {
            assert_eq!(
                expected,
                iter_indices_of_newlines(in_str).collect::<Vec<usize>>()
            )
        });
    }
}
