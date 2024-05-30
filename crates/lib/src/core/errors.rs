// use super::pos::PosMarker;

use std::ops::{Deref, DerefMut};

use fancy_regex::Regex;

use super::parser::segments::base::ErasedSegment;
use super::rules::base::ErasedRule;
use crate::core::parser::markers::PositionMarker;
use crate::helpers::Config;

type CheckTuple = (&'static str, usize, usize);

pub trait SqlError {
    fn fixable(&self) -> bool;
    fn rule_code(&self) -> Option<&'static str>;
    fn identifier(&self) -> &'static str;
    /// Get a tuple representing this error. Mostly for testing.
    fn check_tuple(&self) -> CheckTuple;
}

#[derive(Debug, PartialEq, Clone)]
pub struct SQLBaseError {
    pub fatal: bool,
    pub ignore: bool,
    pub warning: bool,
    pub line_no: usize,
    pub line_pos: usize,
    pub description: String,
    pub rule_code: String,
    pub rule: Option<ErasedRule>,
}

impl Default for SQLBaseError {
    fn default() -> Self {
        Self::new()
    }
}

impl SQLBaseError {
    pub fn new() -> Self {
        Self {
            description: String::new(),
            fatal: false,
            ignore: false,
            warning: false,
            line_no: 0,
            line_pos: 0,
            rule_code: "????".into(),
            rule: None,
        }
    }

    pub fn rule_code(&self) -> &str {
        &self.rule_code
    }

    pub fn set_position_marker(&mut self, position_marker: PositionMarker) {
        let (line_no, line_pos) = position_marker.source_position();

        self.line_no = line_no;
        self.line_pos = line_pos;
    }

    pub fn desc(&self) -> &str {
        &self.description
    }
}

impl SqlError for SQLBaseError {
    fn fixable(&self) -> bool {
        false
    }

    fn rule_code(&self) -> Option<&'static str> {
        None
    }

    fn identifier(&self) -> &'static str {
        "base"
    }

    fn check_tuple(&self) -> CheckTuple {
        ("", self.line_no, self.line_pos)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SQLLintError {
    base: SQLBaseError,
}

impl SQLLintError {
    pub fn new(description: &str, segment: ErasedSegment) -> Self {
        Self {
            base: SQLBaseError::new().config(|this| {
                this.description = description.into();
                this.set_position_marker(segment.get_position_marker().unwrap());
            }),
        }
    }
}

impl Deref for SQLLintError {
    type Target = SQLBaseError;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for SQLLintError {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl From<SQLLintError> for SQLBaseError {
    fn from(mut value: SQLLintError) -> Self {
        if let Some(rule) = &value.rule {
            value.base.rule_code = rule.code().into();
        }

        value.base
    }
}

//
// impl Error for SQLLintError {}
//
// impl fmt::Display for SQLLintError {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "An error which occurred during linting.")
//     }
// }
//
// #[derive(Debug)]
// struct SQLFluffUserError;
//
// impl Error for SQLFluffUserError {}
//
// impl fmt::Display for SQLFluffUserError {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "An error which should be fed back to the user.")
//     }
// }

#[derive(Debug, PartialEq, Clone)]
pub struct SQLTemplaterError {}

impl SqlError for SQLTemplaterError {
    fn fixable(&self) -> bool {
        false
    }

    fn rule_code(&self) -> Option<&'static str> {
        None
    }

    fn identifier(&self) -> &'static str {
        "templater"
    }

    fn check_tuple(&self) -> CheckTuple {
        ("", 0, 0)
    }
}

/// An error which should be fed back to the user.
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

// Not from SQLFluff but translates Python value error
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

impl From<SQLParseError> for SQLBaseError {
    fn from(value: SQLParseError) -> Self {
        let (mut line_no, mut line_pos) = Default::default();

        let pos_marker = value.segment.and_then(|segment| segment.get_position_marker());

        if let Some(pos_marker) = pos_marker {
            (line_no, line_pos) = pos_marker.source_position();
        }

        Self::new().config(|this| {
            this.fatal = true;
            this.line_no = line_no;
            this.line_pos = line_pos;
        })
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
