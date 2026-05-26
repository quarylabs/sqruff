use crate::config::FluffConfig;
use crate::core::linter::common::RenderedSource;
use crate::core::linter::core::Linter;
use crate::core::linter::linted_file::LintedFile;
use sqruff_lib_core::errors::SQLBaseError;
use sqruff_lib_core::parser::segments::Tables;

use super::{
    EngineOptions, FileReport, LexDebugReport, LintDiagnostic, Mode, ParsedDebugReport,
    RenderDebugReport, RunReport, RunRequest, Source, SourceId, SqruffError,
};

pub struct Engine {
    inner: Linter,
    options: EngineOptions,
}

impl Engine {
    pub fn new(config: FluffConfig, options: EngineOptions) -> Result<Self, SqruffError> {
        let inner = Linter::new(config, None, options.parse_errors)?;

        Ok(Self { inner, options })
    }

    pub fn options(&self) -> EngineOptions {
        self.options
    }

    pub fn check_source(&self, source: Source<'_>) -> Result<FileReport, SqruffError> {
        self.lint_source(source, Mode::Check)
    }

    pub fn fix_source(&self, source: Source<'_>) -> Result<FileReport, SqruffError> {
        self.lint_source(source, Mode::Fix)
    }

    pub fn run(&self, request: RunRequest<'_>) -> Result<RunReport, SqruffError> {
        let rendered = self
            .inner
            .render_sources(&request.sources, self.inner.config())
            .map_err(SqruffError::from)?;
        let files = rendered
            .into_iter()
            .map(|rendered| self.lint_rendered_source(rendered, request.mode))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(RunReport { files })
    }

    pub fn parse_source(&self, source: Source<'_>) -> Result<ParsedDebugReport, SqruffError> {
        let config = self.inner.config().for_source(source.text.as_ref())?;
        let rendered = self
            .inner
            .render_source(source.text.as_ref(), &source.id, &config)
            .map_err(SqruffError::from)?;

        self.parse_rendered_for_debug(rendered)
    }

    pub fn render_source_for_debug(
        &self,
        source: Source<'_>,
    ) -> Result<RenderDebugReport, SqruffError> {
        let config = self.inner.config().for_source(source.text.as_ref())?;
        let rendered = self
            .inner
            .render_source(source.text.as_ref(), &source.id, &config)
            .map_err(SqruffError::from)?;

        Ok(match rendered {
            RenderedSource::Rendered {
                source_id,
                rendered,
            } => RenderDebugReport {
                source_id,
                diagnostics: rendered
                    .templater_violations
                    .iter()
                    .cloned()
                    .map(SQLBaseError::from)
                    .map(|error| LintDiagnostic::from_sql_error(&error, &rendered.source_str))
                    .collect(),
                templated_file: Some(rendered.templated_file),
                skipped: None,
            },
            RenderedSource::Skipped { source_id, reason } => RenderDebugReport {
                source_id,
                templated_file: None,
                diagnostics: Vec::new(),
                skipped: Some(reason),
            },
        })
    }

    pub fn lex_source_for_debug(&self, source: Source<'_>) -> Result<LexDebugReport, SqruffError> {
        let config = self.inner.config().for_source(source.text.as_ref())?;
        let rendered = self
            .inner
            .render_source(source.text.as_ref(), &source.id, &config)
            .map_err(SqruffError::from)?;

        let (source_id, rendered) = match rendered {
            RenderedSource::Rendered {
                source_id,
                rendered,
            } => (source_id, rendered),
            RenderedSource::Skipped { source_id, reason } => {
                return Ok(LexDebugReport {
                    source_id,
                    segments: Vec::new(),
                    diagnostics: Vec::new(),
                    skipped: Some(reason),
                });
            }
        };
        let source = rendered.source_str.clone();
        let mut debug_diagnostics = rendered
            .templater_violations
            .iter()
            .cloned()
            .map(SQLBaseError::from)
            .map(|error| LintDiagnostic::from_sql_error(&error, &source))
            .collect::<Vec<_>>();
        let (segments, diagnostics) = Linter::lex_templated_file(
            &Tables::default(),
            rendered.templated_file,
            self.inner.config().dialect(),
        );
        debug_diagnostics.extend(
            diagnostics
                .into_iter()
                .map(SQLBaseError::from)
                .map(|error| LintDiagnostic::from_sql_error(&error, &source)),
        );

        Ok(LexDebugReport {
            source_id,
            segments: segments.unwrap_or_default(),
            diagnostics: debug_diagnostics,
            skipped: None,
        })
    }

    fn lint_source(&self, source: Source<'_>, mode: Mode) -> Result<FileReport, SqruffError> {
        let config = self.inner.config().for_source(source.text.as_ref())?;
        let rendered = self
            .inner
            .render_source(source.text.as_ref(), &source.id, &config)
            .map_err(SqruffError::from)?;
        self.lint_rendered_source(rendered, mode)
    }

    fn lint_rendered_source(
        &self,
        rendered: RenderedSource,
        mode: Mode,
    ) -> Result<FileReport, SqruffError> {
        let (source_id, rendered) = match rendered {
            RenderedSource::Rendered {
                source_id,
                rendered,
            } => (source_id, rendered),
            RenderedSource::Skipped { source_id, reason } => {
                return Ok(FileReport {
                    source_id,
                    diagnostics: Vec::new(),
                    fixed_source: None,
                    skipped: Some(reason),
                });
            }
        };
        let linted_file = self
            .inner
            .lint_rendered(rendered, mode)
            .map_err(|error| SqruffError::Lint(error.value))?;

        Ok(file_report_from_linted_file(linted_file, source_id, mode))
    }

    fn parse_rendered_for_debug(
        &self,
        rendered: RenderedSource,
    ) -> Result<ParsedDebugReport, SqruffError> {
        let (source_id, rendered) = match rendered {
            RenderedSource::Rendered {
                source_id,
                rendered,
            } => (source_id, rendered),
            RenderedSource::Skipped { source_id, reason } => {
                return Ok(ParsedDebugReport {
                    source_id,
                    tree: None,
                    diagnostics: Vec::new(),
                    skipped: Some(reason),
                });
            }
        };
        let source = rendered.source_str.clone();
        let parsed = self.inner.parse_rendered(&Tables::default(), rendered);

        Ok(parsed_debug_report_from_parsed(source_id, parsed, &source))
    }
}

fn parsed_debug_report_from_parsed(
    source_id: SourceId,
    parsed: crate::core::linter::common::ParsedString,
    source: &str,
) -> ParsedDebugReport {
    ParsedDebugReport {
        source_id,
        tree: parsed.tree,
        diagnostics: lint_diagnostics_from_sql_errors(&parsed.violations, source),
        skipped: None,
    }
}

fn lint_diagnostics_from_sql_errors(errors: &[SQLBaseError], source: &str) -> Vec<LintDiagnostic> {
    errors
        .iter()
        .map(|error| LintDiagnostic::from_sql_error(error, source))
        .collect()
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
    use std::sync::Mutex;

    use crate::api::{ParseErrors, SkipReason};
    use crate::templaters::{
        ProcessingMode, Templater, TemplaterError, TemplaterInput, TemplaterOutput,
        TemplaterRuntime,
    };

    use super::*;

    static SKIPPING_TEMPLATER: SkippingTemplater = SkippingTemplater;
    static RECORDING_BATCH_TEMPLATER: RecordingBatchTemplater = RecordingBatchTemplater {
        calls: Mutex::new(Vec::new()),
    };

    struct SkippingTemplater;
    struct RecordingBatchTemplater {
        calls: Mutex<Vec<usize>>,
    }

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

    impl RecordingBatchTemplater {
        fn take_calls(&self) -> Vec<usize> {
            std::mem::take(&mut *self.calls.lock().unwrap())
        }
    }

    impl Templater for RecordingBatchTemplater {
        fn name(&self) -> &'static str {
            "recording-batch"
        }

        fn description(&self) -> &'static str {
            "test batch templater that records batch sizes"
        }

        fn processing_mode(&self) -> ProcessingMode {
            ProcessingMode::Batch
        }

        fn process(
            &self,
            files: &[TemplaterInput<'_>],
            _config: &FluffConfig,
        ) -> Vec<Result<TemplaterOutput, TemplaterError>> {
            self.calls.lock().unwrap().push(files.len());
            files
                .iter()
                .map(|file| {
                    sqruff_lib_core::templaters::TemplatedFile::new(
                        file.source.to_string(),
                        match file.source_id {
                            SourceId::Stdin => "<stdin>".to_string(),
                            SourceId::Path(path) => path.to_string_lossy().into_owned(),
                            SourceId::Virtual(name) => name.clone(),
                        },
                        None,
                        None,
                        None,
                    )
                    .map(TemplaterOutput::Rendered)
                    .map_err(|error| {
                        TemplaterError::Failed(sqruff_lib_core::errors::SQLFluffUserError::new(
                            format!("templater error: {error}"),
                        ))
                    })
                })
                .collect()
        }
    }

    fn test_engine() -> Engine {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = ansi
rules = LT01
"#,
            None,
        )
        .unwrap();

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
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = ansi
"#,
            None,
        )
        .unwrap();
        let engine = Engine {
            inner: Linter::new(
                config,
                Some(TemplaterRuntime::custom(&SKIPPING_TEMPLATER)),
                ParseErrors::Include,
            )
            .unwrap(),
            options: EngineOptions {
                parse_errors: ParseErrors::Include,
            },
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

    #[test]
    fn run_batches_sources_for_batch_templaters() {
        RECORDING_BATCH_TEMPLATER.take_calls();
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = ansi
"#,
            None,
        )
        .unwrap();
        let engine = Engine {
            inner: Linter::new(
                config,
                Some(TemplaterRuntime::custom(&RECORDING_BATCH_TEMPLATER)),
                ParseErrors::Include,
            )
            .unwrap(),
            options: EngineOptions {
                parse_errors: ParseErrors::Include,
            },
        };

        let report = engine
            .run(RunRequest {
                mode: Mode::Check,
                sources: vec![
                    Source {
                        id: SourceId::Virtual("one.sql".into()),
                        text: Cow::Borrowed("select 1\n"),
                    },
                    Source {
                        id: SourceId::Virtual("two.sql".into()),
                        text: Cow::Borrowed("select 2\r\n"),
                    },
                ],
            })
            .unwrap();

        assert_eq!(RECORDING_BATCH_TEMPLATER.take_calls(), vec![2]);
        assert_eq!(report.files.len(), 2);
        assert_eq!(
            report.files[0].source_id,
            SourceId::Virtual("one.sql".into())
        );
        assert_eq!(
            report.files[1].source_id,
            SourceId::Virtual("two.sql".into())
        );
    }

    #[test]
    fn parse_source_returns_debug_tree() {
        let report = test_engine()
            .parse_source(Source {
                id: SourceId::Virtual("debug.sql".into()),
                text: Cow::Borrowed("select 1\n"),
            })
            .unwrap();

        assert_eq!(report.source_id, SourceId::Virtual("debug.sql".into()));
        assert!(report.tree.is_some());
        assert!(report.skipped.is_none());
    }

    #[test]
    fn render_source_for_debug_returns_templated_file() {
        let report = test_engine()
            .render_source_for_debug(Source {
                id: SourceId::Virtual("debug.sql".into()),
                text: Cow::Borrowed("select 1\n"),
            })
            .unwrap();

        assert_eq!(report.source_id, SourceId::Virtual("debug.sql".into()));
        assert_eq!(
            report.templated_file.as_ref().map(|file| file.templated()),
            Some("select 1\n")
        );
        assert!(report.skipped.is_none());
    }

    #[test]
    fn lex_source_for_debug_returns_segments() {
        let report = test_engine()
            .lex_source_for_debug(Source {
                id: SourceId::Virtual("debug.sql".into()),
                text: Cow::Borrowed("select 1\n"),
            })
            .unwrap();

        assert_eq!(report.source_id, SourceId::Virtual("debug.sql".into()));
        assert!(!report.segments.is_empty());
        assert!(report.skipped.is_none());
    }
}
