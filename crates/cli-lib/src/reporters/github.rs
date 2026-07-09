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
        let code = diagnostic.code.as_deref().unwrap_or("????");
        let message = format!(
            "::error title=sqruff,file={},line={},col={}::{}: {}\n",
            filename, diagnostic.line, diagnostic.column, code, diagnostic.message
        );

        let mut output_stream = self.output_stream.lock();
        output_stream.write_all(message.as_bytes())?;
        output_stream.flush()?;
        Ok(())
    }
}
