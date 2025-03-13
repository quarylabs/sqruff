use std::fmt::Display;
use std::ops::{Deref, DerefMut, Range};

use fancy_regex::Regex;

use super::parser::segments::base::ErasedSegment;
use crate::helpers::Config;
use crate::lint_fix::LintFix;
use crate::parser::markers::PositionMarker;

type CheckTuple = (&'static str, usize, usize);

pub trait SqlError: Display {
    fn fixable(&self) -> bool;
    fn rule_code(&self) -> Option<&'static str>;
    fn identifier(&self) -> &'static str;
    /// Get a tuple representing this error. Mostly for testing.
    fn check_tuple(&self) -> CheckTuple;
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct SQLBaseError {
    pub fatal: bool,
    pub ignore: bool,
    pub warning: bool,
    pub line_no: usize,
    pub line_pos: usize,
    pub description: String,
    pub rule: Option<ErrorStructRule>,
    pub source_slice: Range<usize>,
    pub fixable: bool,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ErrorStructRule {
    pub name: &'static str,
    pub code: &'static str,
}

impl SQLBaseError {
    pub fn rule_code(&self) -> &'static str {
        self.rule.as_ref().map_or("????", |rule| rule.code)
    }

    pub fn set_position_marker(&mut self, position_marker: PositionMarker) {
        let (line_no, line_pos) = position_marker.source_position();

        self.line_no = line_no;
        self.line_pos = line_pos;

        self.source_slice = position_marker.source_slice.clone();
    }

    pub fn desc(&self) -> &str {
        &self.description
    }
}

impl Display for SQLBaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl SqlError for SQLBaseError {
    fn fixable(&self) -> bool {
        self.fixable
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

#[derive(Debug, Clone)]
pub struct SQLLintError {
    base: SQLBaseError,
    pub fixes: Vec<LintFix>,
}

impl SQLLintError {
    pub fn new(
        description: &str,
        segment: ErasedSegment,
        fixable: bool,
        fixes: Vec<LintFix>,
    ) -> Self {
        Self {
            base: SQLBaseError::default().config(|this| {
                this.description = description.into();
                this.set_position_marker(segment.get_position_marker().unwrap().clone());
                this.fixable = fixable;
            }),
            fixes,
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
    fn from(value: SQLLintError) -> Self {
        value.base
    }
}

impl From<SQLBaseError> for SQLLintError {
    fn from(value: SQLBaseError) -> Self {
        Self {
            base: value,
            fixes: vec![],
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SQLTemplaterError {}

impl Display for SQLTemplaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SQLTemplaterError")
    }
}

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
    pub value: String,
}

impl SQLFluffUserError {
    pub fn new(value: String) -> SQLFluffUserError {
        SQLFluffUserError { value }
    }
}

impl Display for SQLFluffUserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
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
            let msg = format!(
                "Regex pattern did not match.\nRegex: {:?}\nInput: {:?}",
                regexp, value
            );

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

        let pos_marker = value
            .segment
            .as_ref()
            .and_then(|segment| segment.get_position_marker());

        if let Some(pos_marker) = pos_marker {
            (line_no, line_pos) = pos_marker.source_position();
        }

        Self::default().config(|this| {
            this.fatal = true;
            this.line_no = line_no;
            this.line_pos = line_pos;
            this.description = value.description;
            this.fixable = false;
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
        SQLLexError {
            message,
            position_marker,
        }
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
