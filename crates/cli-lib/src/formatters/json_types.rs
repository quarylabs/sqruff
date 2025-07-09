use std::collections::BTreeMap;

use serde::Serialize;
use sqruff_lib_core::errors::SQLBaseError;

impl From<SQLBaseError> for Diagnostic {
    fn from(value: SQLBaseError) -> Self {
        let code = value.rule.map(|rule| rule.code.to_string());
        Diagnostic {
            range: Range {
                start: Position::new(value.line_no as u32, value.line_pos as u32),
                end: Position::new(value.line_no as u32, value.line_pos as u32),
            },
            message: value.description,
            severity: DiagnosticSeverity::Warning,
            source: Some("sqruff".to_string()),
            code,
        }
    }
}

/// Represents a line and character position, such as the position of the cursor.
#[derive(Serialize)]
struct Position {
    /// The zero-based line value.
    line: u32,
    /// The zero-based character value.
    character: u32,
}

impl Position {
    /// Creates a new `Position` instance.
    fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// A range represents an ordered pair of two positions. It is guaranteed that `start` is before or equal to `end`.
#[derive(Serialize)]
struct Range {
    /// The start position. It is before or equal to `end`.
    start: Position,
    /// The end position. It is after or equal to `start`.
    end: Position,
}

/// Represents a diagnostic, such as a compiler error or warning. Diagnostic objects are only valid in the scope of a file.
#[derive(Serialize)]
pub(crate) struct Diagnostic {
    /// The range to which this diagnostic applies.
    range: Range,
    /// The human-readable message.
    message: String,
    /// The severity, default is {@link DiagnosticSeverity::Error error}.
    pub(crate) severity: DiagnosticSeverity,
    /// A human-readable string describing the source of this diagnostic, e.g. 'typescript' or 'super lint'.
    source: Option<String>,
    // The diagnostic's code, which might appear in the user interface.
    code: Option<String>,
    // An optional property to describe the error code.
    // code_description: Option<CodeDescription>,
    // TODO Maybe implement
    // An array of related diagnostic information, e.g. when symbol-names within a scope collide all definitions can be marked via this property.
    // related_information: Vec<DiagnosticRelatedInformation>,
    // Additional metadata about the diagnostic.
    // tags: Vec<DiagnosticTag>,
}

// Structure to capture a description for an error code.
// #[derive(Serialize)]
// pub(crate) struct CodeDescription {
//     /// An URI to open with more information about the diagnostic error.
//     href: String,
// }

// /// Represents a related message and source code location for a diagnostic. This should be used to point to code locations that cause or are related to a diagnostics, e.g when duplicating a symbol in a scope.
// #[derive(Serialize)]
// struct DiagnosticCode {
//     /// A code or identifier for this diagnostic.
//     value: String,
//     // TODO Maybe implement
//     // A target URI to open with more information about the diagnostic error.
//     // target: Uri,
// }

/// Represents the severity of diagnostics.
#[derive(Serialize)]
pub(crate) enum DiagnosticSeverity {
    /// Something suspicious but allowed.
    Warning = 1,
}

pub(crate) type DiagnosticCollection = BTreeMap<String, Vec<Diagnostic>>;
