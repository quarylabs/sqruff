use sqruff_lib::api::RunReport;
use sqruff_lib::core::config::FluffConfig;

use crate::formatters::OutputStreamFormatter;
use crate::reporters::{CliError, display_source_id};

pub(crate) struct HumanReporter {
    formatter: OutputStreamFormatter,
}

impl HumanReporter {
    pub(crate) fn new(config: &FluffConfig) -> Self {
        Self {
            formatter: OutputStreamFormatter::new(
                std::io::stderr().into(),
                config.get("nocolor", "core").as_bool().unwrap_or_default(),
                config.get("verbose", "core").as_int().unwrap_or_default(),
            ),
        }
    }

    pub(crate) fn emit(&mut self, report: &RunReport) -> Result<(), CliError> {
        self.emit_diagnostics(report)?;
        self.formatter.emit_completion(report.files.len());
        Ok(())
    }

    pub(crate) fn emit_diagnostics(&mut self, report: &RunReport) -> Result<(), CliError> {
        for file in &report.files {
            let source_id = display_source_id(&file.source_id);

            if let Some(reason) = &file.skipped {
                self.formatter.dispatch_file_skip(&source_id, reason);
            } else {
                self.formatter
                    .dispatch_file_diagnostics(&source_id, &file.diagnostics);
            }
        }

        Ok(())
    }
}
