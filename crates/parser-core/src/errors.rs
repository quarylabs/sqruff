use fancy_regex::Regex;
use thiserror::Error;

use crate::parser::token::TokenSpan;

#[derive(Debug, Error)]
#[error("{description}")]
pub struct SQLParseError {
    pub description: String,
    pub span: Option<TokenSpan>,
}

impl SQLParseError {
    pub fn matches(&self, regexp: &str) -> bool {
        let value = &self.description;
        let regex = Regex::new(regexp).expect("Invalid regex pattern");

        if let Ok(true) = regex.is_match(value) {
            true
        } else {
            let msg = format!("Regex pattern did not match.\nRegex: {regexp:?}\nInput: {value:?}");

            if regexp == value {
                panic!("{msg}\nDid you mean to escape the regex?");
            } else {
                panic!("{}", msg);
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct SQLLexError {
    pub message: String,
    pub span: TokenSpan,
}

impl SQLLexError {
    pub fn new(message: String, span: TokenSpan) -> SQLLexError {
        SQLLexError { message, span }
    }
}
