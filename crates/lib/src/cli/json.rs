/// Represents a line and character position, such as the position of the cursor.
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
struct Diagnostic {
    /// The range to which this diagnostic applies.
    range: Range,
    /// The human-readable message.
    message: String,
    /// The severity, default is {@link DiagnosticSeverity::Error error}.
    severity: DiagnosticSeverity,
    /// A human-readable string describing the source of this diagnostic, e.g. 'typescript' or 'super lint'.
    source: Option<String>,
    /// A code or identifier for this diagnostic. Should be used for later processing, e.g. when providing {@link CodeActionContext code actions}.
    code: Option<DiagnosticCode>,
    /// An array of related diagnostic information, e.g. when symbol-names within a scope collide all definitions can be marked via this property.
    related_information: Vec<DiagnosticRelatedInformation>,
    /// Additional metadata about the diagnostic.
    tags: Vec<DiagnosticTag>,
}

/// Represents a related message and source code location for a diagnostic. This should be used to point to code locations that cause or are related to a diagnostics, e.g when duplicating a symbol in a scope.
struct DiagnosticCode {
    /// A code or identifier for this diagnostic.
    value: String,
    /// A target URI to open with more information about the diagnostic error.
    target: Uri,
}

/// Represents the severity of diagnostics.
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