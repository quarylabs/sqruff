use std::ops::{Deref, DerefMut, Range};

use fancy_regex::Regex;
use thiserror::Error;

use super::parser::segments::ErasedSegment;
use crate::helpers::Config;
use crate::parser::markers::PositionMarker;

#[derive(Debug, PartialEq, Clone, Default, Error)]
#[error("{description}")]
pub struct SQLBaseError {
    pub fixable: bool,
    pub warning: bool,
    pub line_no: usize,
    pub line_pos: usize,
    pub description: String,
    pub rule: Option<ErrorStructRule>,
    pub source_slice: Range<usize>,
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

    pub fn rule_name(&self) -> &'static str {
        self.rule.as_ref().map_or("????", |rule| rule.name)
    }

    pub fn warning_if_in<'a>(&mut self, warning_iterable: impl IntoIterator<Item = &'a str>) {
        let rule_code = self.rule_code();
        let rule_name = self.rule_name();
        if warning_iterable
            .into_iter()
            .any(|warning| warning == rule_code || warning == rule_name)
        {
            self.warning = true;
        }
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

#[derive(Debug, Clone, Error)]
#[error(transparent)]
pub struct SQLLintError {
    base: SQLBaseError,
}

impl SQLLintError {
    pub fn new(description: &str, segment: ErasedSegment, fixable: bool) -> Self {
        Self {
            base: SQLBaseError::default().config(|this| {
                this.description = description.into();
                this.set_position_marker(segment.get_position_marker().unwrap().clone());
                this.fixable = fixable;
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
    fn from(value: SQLLintError) -> Self {
        value.base
    }
}

impl From<SQLBaseError> for SQLLintError {
    fn from(value: SQLBaseError) -> Self {
        Self { base: value }
    }
}

#[derive(Debug, PartialEq, Clone, Error)]
#[error("{message}")]
pub struct SQLTemplaterError {
    pub message: String,
}

impl SQLTemplaterError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl From<SQLTemplaterError> for SQLBaseError {
    fn from(value: SQLTemplaterError) -> Self {
        Self {
            description: value.message,
            rule: Some(ErrorStructRule {
                name: "templater",
                code: "TMP",
            }),
            ..Default::default()
        }
    }
}

/// An error which should be fed back to the user.
#[derive(Debug, Error)]
#[error("{value}")]
pub struct SQLFluffUserError {
    pub value: String,
}

impl SQLFluffUserError {
    pub fn new(value: String) -> SQLFluffUserError {
        SQLFluffUserError { value }
    }
}

#[derive(Debug, Error)]
#[error("{description}")]
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
            let msg = format!("Regex pattern did not match.\nRegex: {regexp:?}\nInput: {value:?}");

            if regexp == value {
                panic!("{msg}\nDid you mean to escape the regex?");
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
            this.line_no = line_no;
            this.line_pos = line_pos;
            this.description = value.description;
        })
    }
}

#[derive(PartialEq, Eq, Debug, Error)]
#[error("{message}")]
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

#[derive(Debug, Error)]
#[error("{value}")]
pub struct SQLFluffSkipFile {
    value: String,
}

impl SQLFluffSkipFile {
    pub fn new(value: String) -> SQLFluffSkipFile {
        SQLFluffSkipFile { value }
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorStructRule, SQLBaseError};

    #[test]
    fn warnings_match_rule_codes_and_names() {
        let mut by_code = SQLBaseError {
            rule: Some(ErrorStructRule {
                code: "LT01",
                name: "layout.spacing",
            }),
            ..Default::default()
        };
        by_code.warning_if_in(["LT01"]);
        assert!(by_code.warning);

        let mut by_name = SQLBaseError {
            rule: by_code.rule.clone(),
            ..Default::default()
        };
        by_name.warning_if_in(["layout.spacing"]);
        assert!(by_name.warning);
    }
}
