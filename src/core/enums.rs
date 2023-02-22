use std::fmt;

pub enum FormatType {
    Human,
    Json,
    Yaml,
    GithubAnnotation,
    GithubAnnotationNative,
}

impl fmt::Display for FormatType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FormatType::Human => write!(f, "human"),
            FormatType::Json => write!(f, "json"),
            FormatType::Yaml => write!(f, "yaml"),
            FormatType::GithubAnnotation => write!(f, "github-annotation"),
            FormatType::GithubAnnotationNative => write!(f, "github-annotation-native"),
        }
    }
}
