/// String Helpers for the parser module.
/// Yields all the positions sbstr within in_str https://stackoverflow.com/questions/4664850/how-to-find-all-occurrences-of-a-substring
pub fn find_all(substr: &str, in_str: &str) -> Vec<usize> {
    // Return nothing if one of the inputs is trivial
    if substr.is_empty() || in_str.is_empty() {
        return Vec::new();
    }
    in_str.match_indices(substr).map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_all() {
        vec![
            ("", "", vec![]),
            ("a", "a", vec![0]),
            ("foobar", "o", vec![1, 2]),
            ("bar bar bar bar", "bar", vec![0, 4, 8, 12]),
        ]
        .into_iter()
        .for_each(|(in_str, substr, expected)| assert_eq!(expected, find_all(substr, in_str)));
    }
}
