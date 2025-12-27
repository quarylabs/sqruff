pub fn capitalize(s: &str) -> String {
    assert!(s.is_ascii());

    let mut chars = s.chars();
    let Some(first_char) = chars.next() else {
        return String::new();
    };

    first_char
        .to_uppercase()
        .chain(chars.map(|ch| ch.to_ascii_lowercase()))
        .collect()
}
