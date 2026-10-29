use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use sqruff_lib::Formatter;
use sqruff_lib::core::linter::linted_file::LintedFile;
use sqruff_lib_core::errors::SQLBaseError;

use crate::commands::AnnotationLevel;

pub(crate) struct GitlabFormatter {
    issues: Mutex<Vec<Issue>>,
    output: Mutex<Box<dyn Write + Send>>,
    level: AnnotationLevel,
}

#[derive(Serialize)]
struct Issue {
    check_name: String,
    description: String,
    severity: &'static str,
    fingerprint: String,
    location: Location,
}

#[derive(Serialize)]
struct Location {
    path: String,
    positions: Positions,
}

#[derive(Serialize)]
struct Positions {
    begin: Position,
    end: Position,
}

#[derive(Serialize)]
struct Position {
    line: usize,
    column: usize,
}

impl Issue {
    fn new(
        path: &str,
        violation: &SQLBaseError,
        end: (usize, usize),
        level: AnnotationLevel,
    ) -> Self {
        let path = path.replace('\\', "/");
        // Parser errors currently have no rule metadata in sqruff.
        let code = violation.rule.as_ref().map_or("PRS", |rule| rule.code);
        let line = violation.line_no.max(1);
        let column = violation.line_pos.max(1);
        let severity = if violation.warning {
            "info"
        } else {
            match level {
                AnnotationLevel::Notice => "info",
                AnnotationLevel::Warning => "major",
                AnnotationLevel::Failure | AnnotationLevel::Error => "critical",
            }
        };
        // Match SQLFluff's Code Climate fingerprint, including normalized paths.
        let fingerprint = format!(
            "{:x}",
            md5::compute(format!("{path}:{code}:{line}:{column}"))
        );
        let (end_line, end_column) = end.max((line, column));
        Self {
            check_name: code.into(),
            description: format!("{code}: {}", violation.description),
            severity,
            fingerprint,
            location: Location {
                path,
                positions: Positions {
                    begin: Position { line, column },
                    end: Position {
                        line: end_line,
                        column: end_column,
                    },
                },
            },
        }
    }

    fn sort_key(&self) -> (&str, usize, usize, &str, &str) {
        (
            &self.location.path,
            self.location.positions.begin.line,
            self.location.positions.begin.column,
            &self.check_name,
            &self.description,
        )
    }
}

impl GitlabFormatter {
    pub(crate) fn new(level: AnnotationLevel, path: Option<&Path>) -> io::Result<Self> {
        let output: Box<dyn Write + Send> = match path {
            Some(path) => Box::new(File::create(path)?),
            None => Box::new(io::stdout()),
        };
        Ok(Self {
            issues: Mutex::default(),
            output: Mutex::new(output),
            level,
        })
    }
}

impl Formatter for GitlabFormatter {
    fn dispatch_file_violations(&self, file: &LintedFile) {
        self.issues
            .lock()
            .unwrap()
            .extend(file.violations().iter().map(|violation| {
                Issue::new(
                    file.path(),
                    violation,
                    file.source_position(violation.source_slice.end),
                    self.level,
                )
            }));
    }

    fn dispatch_file_skip(&self, _fname: &str, _reason: &str) {}

    fn completion_message(&self, _count: usize) {
        let mut issues = self.issues.lock().unwrap();
        issues.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        let mut output = self.output.lock().unwrap();
        serde_json::to_writer_pretty(&mut *output, &*issues).unwrap();
        writeln!(output).unwrap();
        output.flush().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_and_path_match_upstream_parse_error() {
        let error = SQLBaseError {
            line_no: 1,
            line_pos: 1,
            description: "Unparsable section".into(),
            ..Default::default()
        };
        let issue = Issue::new(
            r"test\fixtures\linter\parse_error.sql",
            &error,
            (1, 7),
            AnnotationLevel::Warning,
        );
        let value = serde_json::to_value(issue).unwrap();
        assert_eq!(value["fingerprint"], "9319cfff6469f2165074d8033911e5b7");
        assert_eq!(value["check_name"], "PRS");
        assert_eq!(value["severity"], "major");
        assert_eq!(
            value["location"]["path"],
            "test/fixtures/linter/parse_error.sql"
        );
        assert_eq!(value["location"]["positions"]["end"]["column"], 7);
    }

    #[test]
    fn configured_warnings_remain_info_for_every_annotation_level() {
        let error = SQLBaseError {
            warning: true,
            ..Default::default()
        };
        for level in [
            AnnotationLevel::Notice,
            AnnotationLevel::Warning,
            AnnotationLevel::Failure,
            AnnotationLevel::Error,
        ] {
            assert_eq!(
                Issue::new("query.sql", &error, (1, 1), level).severity,
                "info"
            );
        }
    }
}
