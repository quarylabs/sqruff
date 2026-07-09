use crate::commands::{Format, LintArgs};
use crate::reporters::Reporter;
use sqruff_lib::api::{
    Engine, EngineOptions, FileReport, Mode, ParseErrors, RunRequest, Source, SourceId,
};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib_core::helpers;
use std::borrow::Cow;
use std::collections::BTreeSet;
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

struct LoadedSource {
    id: SourceId,
    text: String,
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
    let loaded_sources = match load_sources(&command.input, &config, &ignorer) {
        Ok(sources) => sources,
        Err(e) => {
            eprintln!("{e}");
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
            text: Cow::Borrowed(loaded.text.as_str()),
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

            for (file, loaded) in report.files.iter().zip(loaded_sources.iter()) {
                write_fix_to_disk(file, loaded);
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
    config: &FluffConfig,
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
) -> Result<Vec<LoadedSource>, String> {
    match input {
        Input::Stdin(text) => Ok(vec![LoadedSource {
            id: SourceId::Stdin,
            text: text.clone(),
        }]),
        Input::Paths(paths) => load_path_sources(paths.clone(), config, ignorer),
    }
}

fn load_path_sources(
    mut paths: Vec<PathBuf>,
    config: &FluffConfig,
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
) -> Result<Vec<LoadedSource>, String> {
    if paths.is_empty() {
        paths.push(std::env::current_dir().unwrap());
    }

    let mut expanded_paths = Vec::new();

    for path in paths {
        if path.is_file() {
            expanded_paths.push((path, true));
        } else {
            expanded_paths.extend(
                paths_from_path(path, config.sql_file_exts(), ignorer)?
                    .into_iter()
                    .map(|path| (path, false)),
            );
        };
    }

    expanded_paths
        .into_iter()
        .filter(|(path, is_explicit)| *is_explicit || !ignorer(path))
        .map(|(path, _)| {
            std::fs::read_to_string(&path)
                .map(|text| LoadedSource {
                    id: SourceId::Path(path),
                    text,
                })
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn paths_from_path(
    path: PathBuf,
    sql_file_exts: &[String],
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
) -> Result<Vec<PathBuf>, String> {
    let Ok(metadata) = std::fs::metadata(&path) else {
        return Err(format!(
            "Specified path does not exist. Check it/they exist(s): {path:?}"
        ));
    };

    let mut buffer = BTreeSet::new();

    if metadata.is_file() {
        buffer.insert(helpers::normalize(&path));
    } else {
        collect_sql_paths(&path, sql_file_exts, ignorer, &mut buffer)?;
    }

    Ok(buffer.into_iter().collect())
}

fn collect_sql_paths(
    dir: &Path,
    sql_file_exts: &[String],
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
    buffer: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    if ignorer(dir) {
        log::debug!(
            "Skipping directory '{}' during file discovery traversal",
            dir.display()
        );
        return Ok(());
    }

    let entries = std::fs::read_dir(dir).map_err(|error| error.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;

        if file_type.is_dir() {
            collect_sql_paths(&path, sql_file_exts, ignorer, buffer)?;
        } else if file_type.is_file() && path_is_sql_file(&path, sql_file_exts) && !ignorer(&path) {
            buffer.insert(helpers::normalize(&path));
        }
    }

    Ok(())
}

fn path_is_sql_file(path: &Path, sql_file_exts: &[String]) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let file_name = file_name.to_lowercase();

    sql_file_exts.iter().any(|ext| file_name.ends_with(ext))
}

fn write_fix_to_disk(file: &FileReport, loaded: &LoadedSource) {
    if file
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.is_none())
    {
        return;
    }

    let Some(fixed_source) = &file.fixed_source else {
        return;
    };

    if fixed_source == &loaded.text {
        return;
    }

    let SourceId::Path(path) = &file.source_id else {
        return;
    };

    std::fs::write(path, fixed_source).unwrap();
}

fn has_unfixable_diagnostics(file: &FileReport) -> bool {
    file.diagnostics
        .iter()
        .any(|diagnostic| !diagnostic.fixable)
}
