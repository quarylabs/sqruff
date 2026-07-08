use serde::Serialize;
use sqruff_lib::api::RunReport;

use crate::formatters::json_types::{Diagnostic, DiagnosticCollection};
use crate::reporters::{CliError, display_source_id};

#[derive(Default)]
pub(crate) struct JsonReporter;

impl JsonReporter {
    pub(crate) fn emit(&mut self, report: &RunReport) -> Result<(), CliError> {
        let json_report = JsonReport::from(report);
        serde_json::to_writer(std::io::stdout(), &json_report.diagnostics)?;
        println!();
        Ok(())
    }
}

#[derive(Serialize)]
struct JsonReport {
    diagnostics: DiagnosticCollection,
}

impl From<&RunReport> for JsonReport {
    fn from(report: &RunReport) -> Self {
        let diagnostics = report
            .files
            .iter()
            .map(|file| {
                (
                    display_source_id(&file.source_id),
                    file.diagnostics
                        .iter()
                        .map(Diagnostic::from)
                        .collect::<Vec<_>>(),
                )
            })
            .collect();

        Self { diagnostics }
    }
}
