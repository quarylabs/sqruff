use crate::core::config::FluffConfig;
use crate::core::linter::core::Linter;
use crate::core::linter::linted_file::LintedFile;
use sqruff_lib_core::errors::{SQLBaseError, SQLFluffUserError};

use super::{
    EngineOptions, FileReport, LintDiagnostic, Mode, RunReport, RunRequest, Source, SourceId,
    SqruffError,
};

pub struct Engine {
    inner: Linter,
}

impl Engine {
    pub fn new(config: FluffConfig, options: EngineOptions) -> Result<Self, SqruffError> {
        let inner =
            Linter::new(config, None, options.parse_errors).map_err(SQLFluffUserError::new)?;

        Ok(Self { inner })
    }

    pub fn check_source(&self, source: Source<'_>) -> Result<FileReport, SqruffError> {
        self.lint_source(source, Mode::Check)
    }

    pub fn fix_source(&self, source: Source<'_>) -> Result<FileReport, SqruffError> {
        self.lint_source(source, Mode::Fix)
    }

    pub fn run(&self, request: RunRequest<'_>) -> Result<RunReport, SqruffError> {
        let mut files = Vec::with_capacity(request.sources.len());

        for source in request.sources {
            let report = match request.mode {
                Mode::Check => self.check_source(source)?,
                Mode::Fix => self.fix_source(source)?,
            };
            files.push(report);
        }

        Ok(RunReport { files })
    }

    pub fn reload_config(&mut self, config: FluffConfig) -> Result<(), SqruffError> {
        let parse_errors = self.inner.parse_errors();
        self.inner = Linter::new(config, None, parse_errors).map_err(SQLFluffUserError::new)?;

        Ok(())
    }

    fn lint_source(&self, source: Source<'_>, mode: Mode) -> Result<FileReport, SqruffError> {
        let filename = filename_for_source_id(&source.id);
        let linted_file = self
            .inner
            .lint_string(source.text.as_ref(), filename, mode)?;

        Ok(file_report_from_linted_file(linted_file, source.id, mode))
    }
}

fn filename_for_source_id(source_id: &SourceId) -> Option<String> {
    match source_id {
        SourceId::Stdin => None,
        SourceId::Path(path) => Some(path.to_string_lossy().into_owned()),
        SourceId::Virtual(name) => Some(name.clone()),
    }
}

fn file_report_from_linted_file(
    linted_file: LintedFile,
    source_id: SourceId,
    mode: Mode,
) -> FileReport {
    let diagnostics = linted_file
        .violations()
        .iter()
        .map(lint_diagnostic_from_error)
        .collect();
    let fixed_source = matches!(mode, Mode::Fix).then(|| linted_file.fix_string());

    FileReport {
        source_id,
        diagnostics,
        fixed_source,
        skipped: None,
    }
}

fn lint_diagnostic_from_error(error: &SQLBaseError) -> LintDiagnostic {
    LintDiagnostic {
        message: error.desc().to_string(),
        code: error.rule.as_ref().map(|rule| rule.code.to_string()),
        line: error.line_no,
        column: error.line_pos,
        source_range: error.source_slice.clone(),
        fixable: error.fixable,
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::api::ParseErrors;

    use super::*;

    fn test_engine() -> Engine {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = ansi
rules = LT01
"#,
            None,
        );

        Engine::new(
            config,
            EngineOptions {
                parse_errors: ParseErrors::Include,
            },
        )
        .unwrap()
    }

    #[test]
    fn check_source_reports_diagnostics() {
        let report = test_engine()
            .check_source(Source {
                id: SourceId::Virtual("query.sql".into()),
                text: Cow::Borrowed("select  1\n"),
            })
            .unwrap();

        assert_eq!(report.source_id, SourceId::Virtual("query.sql".into()));
        assert!(report.fixed_source.is_none());
        assert!(report.skipped.is_none());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some("LT01"))
        );
    }

    #[test]
    fn fix_source_returns_fixed_source() {
        let report = test_engine()
            .fix_source(Source {
                id: SourceId::Stdin,
                text: Cow::Borrowed("select  1\n"),
            })
            .unwrap();

        assert_eq!(report.source_id, SourceId::Stdin);
        assert_eq!(report.fixed_source.as_deref(), Some("select 1\n"));
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some("LT01"))
        );
    }
}
