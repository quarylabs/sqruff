use std::io::{Stderr, Write};

use sqruff_lib::api::{LintDiagnostic, RunReport};

use crate::reporters::{CliError, display_source_id};

pub(crate) struct GithubReporter {
    output_stream: Stderr,
}

impl GithubReporter {
    pub(crate) fn new() -> Self {
        Self {
            output_stream: std::io::stderr(),
        }
    }

    pub(crate) fn emit(&mut self, report: &RunReport) -> Result<(), CliError> {
        for file in &report.files {
            let filename = display_source_id(&file.source_id);
            for diagnostic in &file.diagnostics {
                self.emit_diagnostic(&filename, diagnostic)?;
            }
        }

        Ok(())
    }

    fn emit_diagnostic(
        &mut self,
        filename: &str,
        diagnostic: &LintDiagnostic,
    ) -> Result<(), CliError> {
        let diagnostic = GithubAnnotation::from(diagnostic);
        let message = format!(
            "::error title=sqruff,file={},line={},col={}::{}: {}\n",
            escape_property(filename),
            diagnostic.line,
            diagnostic.column,
            escape_property(&diagnostic.code),
            escape_data(&diagnostic.message)
        );

        let mut output_stream = self.output_stream.lock();
        output_stream.write_all(message.as_bytes())?;
        output_stream.flush()?;
        Ok(())
    }
}

fn escape_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(value: &str) -> String {
    escape_data(value).replace(':', "%3A").replace(',', "%2C")
}

struct GithubAnnotation {
    code: String,
    line: usize,
    column: usize,
    message: String,
}

impl From<&LintDiagnostic> for GithubAnnotation {
    fn from(diagnostic: &LintDiagnostic) -> Self {
        Self {
            code: diagnostic
                .code
                .clone()
                .unwrap_or_else(|| "????".to_string()),
            line: diagnostic.line,
            column: diagnostic.column,
            message: diagnostic.message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqruff_lib::api::LintDiagnostic;

    #[test]
    fn escapes_github_annotation_properties() {
        assert_eq!(
            escape_property("dir:with,commas%/query\n.sql"),
            "dir%3Awith%2Ccommas%25/query%0A.sql"
        );
    }

    #[test]
    fn escapes_github_annotation_data() {
        assert_eq!(
            escape_data("bad %, newline\nand\rreturn: comma, ok"),
            "bad %25, newline%0Aand%0Dreturn: comma, ok"
        );
    }

    #[test]
    fn github_annotation_escapes_file_code_and_message() {
        let diagnostic = LintDiagnostic {
            message: "bad % message\nwith newline".to_string(),
            code: Some("LT:01,odd%".to_string()),
            line: 1,
            column: 2,
            end_line: 1,
            end_column: 5,
            source_range: 0..4,
            fixable: false,
        };
        let diagnostic = GithubAnnotation::from(&diagnostic);
        let message = format!(
            "::error title=sqruff,file={},line={},col={}::{}: {}\n",
            escape_property("path:with,meta%/query.sql"),
            diagnostic.line,
            diagnostic.column,
            escape_property(&diagnostic.code),
            escape_data(&diagnostic.message)
        );

        assert_eq!(
            message,
            "::error title=sqruff,file=path%3Awith%2Cmeta%25/query.sql,line=1,col=2::LT%3A01%2Codd%25: bad %25 message%0Awith newline\n"
        );
    }
}
