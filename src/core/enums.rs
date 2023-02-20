use std::fmt;

pub enum FormatType {
    human,
    json,
    yaml,
    github_annotation,
    github_annotation_native,
}

impl fmt::Display for FormatType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FormatType::human => write!(f, "human"),
            FormatType::json => write!(f, "json"),
            FormatType::yaml => write!(f, "yaml"),
            FormatType::github_annotation => write!(f, "github-annotation"),
            FormatType::github_annotation_native => write!(f, "github-annotation-native"),
        }
    }
}
