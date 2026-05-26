use crate::core::config::FluffConfig;
use crate::core::linter::common::RenderedSource;
use crate::core::linter::core::Linter;
use crate::core::linter::linted_file::LintedFile;

use super::{
    EngineOptions, FileReport, LintDiagnostic, Mode, RunReport, RunRequest, Source, SourceId,
    SqruffError,
};

pub struct Engine {
    inner: Linter,
}

impl Engine {
    pub fn new(config: FluffConfig, options: EngineOptions) -> Result<Self, SqruffError> {
        let inner = Linter::new(config, None, options.parse_errors)?;

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
        self.inner = Linter::new(config, None, parse_errors)?;

        Ok(())
    }

    fn lint_source(&self, source: Source<'_>, mode: Mode) -> Result<FileReport, SqruffError> {
        let rendered = self
            .inner
            .render_source(source.text.as_ref(), &source.id, self.inner.config())
            .map_err(SqruffError::from)?;
        let rendered = match rendered {
            RenderedSource::Rendered(rendered) => rendered,
            RenderedSource::Skipped(skipped) => {
                return Ok(FileReport {
                    source_id: source.id,
                    diagnostics: Vec::new(),
                    fixed_source: None,
                    skipped: Some(skipped),
                });
            }
        };

        let linted_file = self
            .inner
            .lint_rendered(rendered, mode)
            .map_err(|error| SqruffError::Lint(error.value))?;

        Ok(file_report_from_linted_file(linted_file, source.id, mode))
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
        .map(|error| LintDiagnostic::from_sql_error(error, linted_file.source()))
        .collect();
    let fixed_source = matches!(mode, Mode::Fix).then(|| linted_file.fix_string());

    FileReport {
        source_id,
        diagnostics,
        fixed_source,
        skipped: None,
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::api::{ParseErrors, SkipReason};
    use crate::templaters::{
        ProcessingMode, Templater, TemplaterError, TemplaterInput, TemplaterOutput,
        TemplaterRuntime,
    };

    use super::*;

    static SKIPPING_TEMPLATER: SkippingTemplater = SkippingTemplater;

    struct SkippingTemplater;

    impl Templater for SkippingTemplater {
        fn name(&self) -> &'static str {
            "skipping"
        }

        fn description(&self) -> &'static str {
            "test templater that skips every source"
        }

        fn processing_mode(&self) -> ProcessingMode {
            ProcessingMode::Sequential
        }

        fn process(
            &self,
            files: &[TemplaterInput<'_>],
            _config: &FluffConfig,
        ) -> Vec<Result<TemplaterOutput, TemplaterError>> {
            files
                .iter()
                .map(|_| {
                    Ok(TemplaterOutput::Skipped(SkipReason {
                        message: "disabled by templater".into(),
                    }))
                })
                .collect()
        }
    }

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

    #[test]
    fn check_source_reports_templater_skip() {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = ansi
"#,
            None,
        );
        let engine = Engine {
            inner: Linter::new(
                config,
                Some(TemplaterRuntime::custom(&SKIPPING_TEMPLATER)),
                ParseErrors::Include,
            )
            .unwrap(),
        };

        let report = engine
            .check_source(Source {
                id: SourceId::Virtual("disabled.sql".into()),
                text: Cow::Borrowed("select  1\n"),
            })
            .unwrap();

        assert_eq!(report.source_id, SourceId::Virtual("disabled.sql".into()));
        assert!(report.diagnostics.is_empty());
        assert!(report.fixed_source.is_none());
        assert_eq!(
            report
                .skipped
                .as_ref()
                .map(|reason| reason.message.as_str()),
            Some("disabled by templater")
        );
    }
}
