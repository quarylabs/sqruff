use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum SqruffError {
    #[error("config error: {0}")]
    Config(String),

    #[error("templater error: {0}")]
    Templater(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("lint error: {0}")]
    Lint(String),

    #[error("I/O error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("unsupported operation: {0}")]
    Unsupported(&'static str),
}

impl SqruffError {
    pub fn message(&self) -> String {
        match self {
            Self::Config(message)
            | Self::Templater(message)
            | Self::Parse(message)
            | Self::Lint(message) => message.clone(),
            Self::Io { .. } | Self::Unsupported(_) => self.to_string(),
        }
    }
}
