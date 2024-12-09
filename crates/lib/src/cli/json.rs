use serde::Serialize;
use sqruff_lib_core::errors::SQLBaseError;
use std::collections::BTreeMap;
use std::io::Write;
use std::{
    io::Stderr,
    sync::atomic::{AtomicBool, Ordering},
};

use super::formatters::Formatter;

struct JSONFormatter {
    output_stream: Stderr,
    diagnostics: DiagnosticCollection,
    has_fail: AtomicBool,
}

impl JSONFormatter {
    pub fn new(stderr: Stderr) -> Self {
        Self {
            diagnostics: <_>::default(),
            output_stream: stderr,
            has_fail: AtomicBool::new(false),
        }
    }

    fn dispatch(&self, s: &str) {
        let _ignored = self.output_stream.lock().write_all(s.as_bytes()).unwrap();
    }
}

impl Formatter for JSONFormatter {
    fn dispatch_template_header(
        &self,
        f_name: String,
        linter_config: crate::core::config::FluffConfig,
        file_config: crate::core::config::FluffConfig,
    ) {
    }

    fn dispatch_parse_header(&self, f_name: String) {}

    fn dispatch_file_violations(
        &mut self,
        linted_file: &crate::core::linter::linted_file::LintedFile,
        only_fixable: bool,
    ) {
        let mut violations = linted_file.get_violations(None);

        violations.sort_by(|a, b| {
            a.line_no
                .cmp(&b.line_no)
                .then_with(|| a.line_pos.cmp(&b.line_pos))
                .then_with(|| {
                    let b_code = b.rule.as_ref().unwrap().code;
                    let a_code = a.rule.as_ref().unwrap().code;
                    a_code.cmp(b_code)
                })
        });

        self.diagnostics.insert(
            linted_file.path.to_string(),
            violations
                .iter()
                .map(|v| Diagnostic::from(v.clone()))
                .collect(),
        );
    }

    fn has_fail(&self) -> bool {
        self.has_fail.load(Ordering::SeqCst)
    }

    fn completion_message(&self) {
        let diagnostics_json = serde_json::to_string(&self.diagnostics).unwrap();

        self.dispatch(&diagnostics_json);
    }
}

impl From<SQLBaseError> for Diagnostic {
    fn from(value: SQLBaseError) -> Self {
        Diagnostic {
            range: Range {
                start: Position::new(value.line_no as u32, value.line_pos as u32),
                end: Position::new(value.line_no as u32, value.line_pos as u32),
            },
            message: value.description,
            severity: if value.warning {
                DiagnosticSeverity::Warning
            } else {
                DiagnosticSeverity::Error
            },
            source: Some("sqruff".to_string()),
            // code: todo!(),
            // source: Some(value.get_source().to_string()),
            // code: Some(DiagnosticCode {
            //     value: value.rule_code().to_string(),
            //     target: Uri::new("".to_string()),
            // }),
            // related_information: Vec::new(),
            // tags: Vec::new(),
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

impl Range {
    /// Creates a new `Range` instance.
    fn new(start: Position, end: Position) -> Result<Self, &'static str> {
        if start.line > end.line || (start.line == end.line && start.character > end.character) {
            return Err("The start position must be before or equal to the end position.");
        }
        Ok(Self { start, end })
    }
}

/// Represents a diagnostic, such as a compiler error or warning. Diagnostic objects are only valid in the scope of a file.
#[derive(Serialize)]
struct Diagnostic {
    /// The range to which this diagnostic applies.
    range: Range,
    /// The human-readable message.
    message: String,
    /// The severity, default is {@link DiagnosticSeverity::Error error}.
    severity: DiagnosticSeverity,
    /// A human-readable string describing the source of this diagnostic, e.g. 'typescript' or 'super lint'.
    source: Option<String>,
    // A code or identifier for this diagnostic. Should be used for later processing, e.g. when providing {@link CodeActionContext code actions}.
    // code: Option<DiagnosticCode>,
    // TODO Maybe implement
    // An array of related diagnostic information, e.g. when symbol-names within a scope collide all definitions can be marked via this property.
    // related_information: Vec<DiagnosticRelatedInformation>,
    // Additional metadata about the diagnostic.
    // tags: Vec<DiagnosticTag>,
}

/// Represents a related message and source code location for a diagnostic. This should be used to point to code locations that cause or are related to a diagnostics, e.g when duplicating a symbol in a scope.
#[derive(Serialize)]
struct DiagnosticCode {
    /// A code or identifier for this diagnostic.
    value: String,
    // TODO Maybe implement
    // A target URI to open with more information about the diagnostic error.
    // target: Uri,
}

/// Represents the severity of diagnostics.
#[derive(Serialize)]
enum DiagnosticSeverity {
    /// Something not allowed by the rules of a language or other means.
    Error = 0,
    /// Something suspicious but allowed.
    Warning = 1,
    /// Something to inform about but not a problem.
    Information = 2,
    /// Something to hint to a better way of doing it, like proposing a refactoring.
    Hint = 3,
}

type DiagnosticCollection = BTreeMap<String, Vec<Diagnostic>>;
