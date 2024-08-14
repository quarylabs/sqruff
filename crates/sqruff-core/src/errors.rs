use fancy_regex::Regex;

use super::parser::segments::base::ErasedSegment;
use crate::parser::markers::PositionMarker;

#[derive(Debug)]
pub struct SQLParseError {
    pub description: String,
    pub segment: Option<ErasedSegment>,
}

impl SQLParseError {
    pub fn matches(&self, regexp: &str) -> bool {
        let value = &self.description;
        let regex = Regex::new(regexp).expect("Invalid regex pattern");

        if let Ok(true) = regex.is_match(value) {
            true
        } else {
            let msg =
                format!("Regex pattern did not match.\nRegex: {:?}\nInput: {:?}", regexp, value);

            if regexp == value {
                panic!("{}\nDid you mean to escape the regex?", msg);
            } else {
                panic!("{}", msg);
            }
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct SQLLexError {
    message: String,
    position_marker: PositionMarker,
}

impl SQLLexError {
    pub fn new(message: String, position_marker: PositionMarker) -> SQLLexError {
        SQLLexError { message, position_marker }
    }
}

#[derive(Debug)]
pub struct SQLFluffSkipFile {
    #[allow(dead_code)]
    value: String,
}

impl SQLFluffSkipFile {
    pub fn new(value: String) -> SQLFluffSkipFile {
        SQLFluffSkipFile { value }
    }
}

#[derive(Debug)]
pub struct ValueError {
    #[allow(dead_code)]
    value: String,
}

impl ValueError {
    pub fn new(value: String) -> ValueError {
        ValueError { value }
    }
}

#[derive(Debug)]
pub struct SQLFluffUserError {
    #[allow(dead_code)]
    value: String,
}

impl SQLFluffUserError {
    pub fn new(value: String) -> SQLFluffUserError {
        SQLFluffUserError { value }
    }
}
