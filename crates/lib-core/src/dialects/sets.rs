#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialectSetKey<'a> {
    Named(&'a str),
    BracketPairs,
    AngleBracketPairs,
}

impl<'a> DialectSetKey<'a> {
    pub fn parse(value: &'a str) -> Self {
        match value {
            "bracket_pairs" => Self::BracketPairs,
            "angle_bracket_pairs" => Self::AngleBracketPairs,
            other => Self::Named(other),
        }
    }

    pub fn as_set_name(self) -> Option<&'a str> {
        match self {
            Self::Named(name) => Some(name),
            Self::BracketPairs | Self::AngleBracketPairs => None,
        }
    }

    pub const fn as_bracket_set_name(self) -> Option<&'static str> {
        match self {
            Self::Named(_) => None,
            Self::BracketPairs => Some("bracket_pairs"),
            Self::AngleBracketPairs => Some("angle_bracket_pairs"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DialectSetKey;

    #[test]
    fn parses_named_and_bracket_set_keys() {
        assert_eq!(
            DialectSetKey::parse("reserved_keywords"),
            DialectSetKey::Named("reserved_keywords")
        );
        assert_eq!(
            DialectSetKey::parse("bracket_pairs"),
            DialectSetKey::BracketPairs
        );
        assert_eq!(
            DialectSetKey::parse("angle_bracket_pairs"),
            DialectSetKey::AngleBracketPairs
        );
    }
}
