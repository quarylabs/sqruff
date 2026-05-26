use thiserror::Error;

/// Errors that can occur when constructing or validating configuration.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    #[error("invalid section '{section}': {reason}")]
    InvalidSection { section: String, reason: String },
    #[error("invalid value for '{field}': {reason}")]
    InvalidField { field: &'static str, reason: String },
    #[error("unknown config section: {0}")]
    UnknownSection(String),
    #[error("unknown dialect: {0}")]
    UnknownDialect(String),
    #[error("unsupported templater: {0}")]
    UnsupportedTemplater(String),
}
