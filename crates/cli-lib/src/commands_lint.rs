use crate::commands::{Format, LintArgs};
use crate::reporters::Reporter;
use sqruff_lib::api::{
    Engine, EngineOptions, FileReport, IgnoreMatcher, Mode, ParseErrors, PathDiscoveryOptions,
    RunRequest, Source, SourceId, Workspace,
};
use sqruff_lib::core::config::FluffConfig;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub(crate) struct LintCommand {
    pub mode: Mode,
    pub input: Input,
    pub apply: ApplyFixes,
    pub format: Format,
}

pub(crate) enum Input {
    Paths(Vec<PathBuf>),
    Stdin(String),
}

pub(crate) enum ApplyFixes {
    Never,
    ToDisk,
    Stdout,
}

pub(crate) fn run_lint(
    args: LintArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    let LintArgs { paths, format } = args;
    run_lint_command(
        LintCommand {
            mode: Mode::Check,
            input: Input::Paths(paths),
            apply: ApplyFixes::Never,
            format,
        },
        config,
        ignorer,
        collect_parse_errors,
    )
}

pub(crate) fn run_lint_stdin(
    config: FluffConfig,
    format: Format,
    collect_parse_errors: bool,
) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    run_lint_command(
        LintCommand {
            mode: Mode::Check,
            input: Input::Stdin(read_in),
            apply: ApplyFixes::Never,
            format,
        },
        config,
        |_| false,
        collect_parse_errors,
    )
}

pub(crate) fn run_lint_command(
    command: LintCommand,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    let mut reporter = Reporter::new(command.format, &config);
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let workspace = match Workspace::new(workspace_root.clone()) {
        Ok(workspace) => workspace,
        Err(e) => {
            eprintln!("{}", e.value);
            return 1;
        }
    };
    let loaded_sources = match load_sources(&command.input, &workspace, &workspace_root, &ignorer) {
        Ok(sources) => sources,
        Err(e) => {
            eprintln!("{}", e.value);
            return 1;
        }
    };
    let engine = match Engine::new(
        config,
        EngineOptions {
            parse_errors: if collect_parse_errors {
                ParseErrors::Include
            } else {
                ParseErrors::Suppress
            },
        },
    ) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("{}", e.value);
            return 1;
        }
    };
    let sources = loaded_sources
        .iter()
        .map(|loaded| Source {
            id: loaded.id.clone(),
            text: Cow::Borrowed(loaded.text.as_ref()),
        })
        .collect();
    let report = match engine.run(RunRequest {
        mode: command.mode,
        sources,
    }) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("{}", e.value);
            return 1;
        }
    };

    let files = report.files.len();
    let has_violations = report.files.iter().any(|file| !file.diagnostics.is_empty());

    match command.apply {
        ApplyFixes::Never => {
            if let Err(error) = reporter.emit(&report) {
                eprintln!("{error}");
                return 1;
            }
            has_violations as i32
        }
        ApplyFixes::Stdout => {
            let any_unfixable_errors = report.files.iter().any(has_unfixable_diagnostics);
            if let Err(error) = reporter.emit_diagnostics(&report) {
                eprintln!("{error}");
                return 1;
            }
            for file in report.files {
                if let Some(fixed_source) = file.fixed_source {
                    println!("{fixed_source}");
                }
            }

            any_unfixable_errors as i32
        }
        ApplyFixes::ToDisk => {
            if !has_violations {
                println!("{files} files processed, nothing to fix.");
                return 0;
            }

            let any_unfixable_errors = report.files.iter().any(has_unfixable_diagnostics);

            if let Err(e) = workspace.apply_fixes(&report) {
                eprintln!("{}", e.value);
                return 1;
            }

            if let Err(error) = reporter.emit(&report) {
                eprintln!("{error}");
                return 1;
            }
            any_unfixable_errors as i32
        }
    }
}

fn load_sources(
    input: &Input,
    workspace: &Workspace,
    working_dir: &Path,
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
) -> Result<Vec<Source<'static>>, sqruff_lib::api::SqruffError> {
    match input {
        Input::Stdin(text) => Ok(vec![Source {
            id: SourceId::Stdin,
            text: Cow::Owned(text.clone()),
        }]),
        Input::Paths(paths) => {
            let ignore_matcher = ClosureIgnoreMatcher { ignorer };
            let options = PathDiscoveryOptions {
                ignore_file_name: ".sqruffignore",
                ignore_non_existent_files: false,
                ignore_files: true,
                working_dir: working_dir.to_path_buf(),
                ignorer: Some(&ignore_matcher),
            };
            workspace.discover_sources(paths, &options)
        }
    }
}

struct ClosureIgnoreMatcher<'a> {
    ignorer: &'a (dyn Fn(&Path) -> bool + Send + Sync),
}

impl IgnoreMatcher for ClosureIgnoreMatcher<'_> {
    fn is_ignored(&self, path: &Path) -> bool {
        (self.ignorer)(path)
    }
}

fn has_unfixable_diagnostics(file: &FileReport) -> bool {
    file.diagnostics
        .iter()
        .any(|diagnostic| !diagnostic.fixable)
}
