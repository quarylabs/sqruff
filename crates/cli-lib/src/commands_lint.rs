use crate::baseline::{Baseline, BaselineStats, filter_violations_against_baseline};
use crate::commands::{Format, LintArgs};
use crate::formatters::OutputStreamFormatter;
use crate::formatters::github_annotation_native_formatter::GithubAnnotationNativeFormatter;
use crate::formatters::json::JsonFormatter;
use sqruff_lib::Formatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib::core::linter::linted_file::LintedFile;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) fn run_lint(
    args: LintArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    let LintArgs {
        paths,
        format,
        baseline,
    } = args;

    match baseline {
        Some(baseline_path) => run_lint_with_baseline(
            paths,
            format,
            baseline_path,
            config,
            ignorer,
            collect_parse_errors,
        ),
        None => run_lint_without_baseline(paths, format, config, ignorer, collect_parse_errors),
    }
}

fn run_lint_without_baseline(
    paths: Vec<PathBuf>,
    format: Format,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    let mut linter = crate::linter(config, format, collect_parse_errors);
    let result = linter.lint_paths(paths, false, &ignorer);

    linter.formatter().unwrap().completion_message(result.len());

    result.has_violations() as i32
}

fn run_lint_with_baseline(
    paths: Vec<PathBuf>,
    format: Format,
    baseline_path: PathBuf,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    // Load the baseline
    let baseline = match Baseline::load(&baseline_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "Error loading baseline from {}: {}",
                baseline_path.display(),
                e
            );
            return 1;
        }
    };

    eprintln!(
        "Using baseline: {} ({} violations in {} files)",
        baseline_path.display(),
        baseline.total_violations(),
        baseline.file_count()
    );

    // Create a linter WITHOUT a formatter (we'll dispatch manually after filtering)
    let mut linter = Linter::new(config.clone(), None, None, collect_parse_errors);
    let result = linter.lint_paths(paths, false, &ignorer);

    // Create the formatter
    let formatter: Arc<dyn Formatter> = create_formatter(format, &config);

    // Track aggregate statistics
    let mut total_stats = BaselineStats::default();
    let mut files_with_new_violations = 0;
    let file_count = result.len();

    // Process each file, filter violations against baseline, and dispatch
    for file in result {
        let filtered = filter_violations_against_baseline(&file, &baseline);

        total_stats.suppressed += filtered.stats.suppressed;
        total_stats.new_violations += filtered.stats.new_violations;
        total_stats.fixed += filtered.stats.fixed;

        if !filtered.new_violations.is_empty() {
            files_with_new_violations += 1;

            // Create a new LintedFile with only the new violations
            let filtered_file = create_filtered_linted_file(&file, filtered.new_violations);
            formatter.dispatch_file_violations(&filtered_file);
        }
    }

    // Output completion message
    formatter.completion_message(file_count);

    // Output baseline summary
    print_baseline_summary(&total_stats, files_with_new_violations);

    // Return non-zero if there are new violations
    (total_stats.new_violations > 0) as i32
}

pub(crate) fn run_lint_stdin(
    config: FluffConfig,
    format: Format,
    baseline: Option<PathBuf>,
    collect_parse_errors: bool,
) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    match baseline {
        Some(baseline_path) => run_lint_stdin_with_baseline(
            &read_in,
            format,
            baseline_path,
            config,
            collect_parse_errors,
        ),
        None => run_lint_stdin_without_baseline(&read_in, format, config, collect_parse_errors),
    }
}

fn run_lint_stdin_without_baseline(
    sql: &str,
    format: Format,
    config: FluffConfig,
    collect_parse_errors: bool,
) -> i32 {
    let linter = crate::linter(config, format, collect_parse_errors);
    let result = linter.lint_string(sql, None, false);

    linter.formatter().unwrap().completion_message(1);

    result.has_violations() as i32
}

fn run_lint_stdin_with_baseline(
    sql: &str,
    format: Format,
    baseline_path: PathBuf,
    config: FluffConfig,
    collect_parse_errors: bool,
) -> i32 {
    // Load the baseline
    let baseline = match Baseline::load(&baseline_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "Error loading baseline from {}: {}",
                baseline_path.display(),
                e
            );
            return 1;
        }
    };

    // Create a linter WITHOUT a formatter
    let linter = Linter::new(config.clone(), None, None, collect_parse_errors);
    let file = linter.lint_string(sql, None, false);

    // Create the formatter
    let formatter: Arc<dyn Formatter> = create_formatter(format, &config);

    // Filter violations
    let filtered = filter_violations_against_baseline(&file, &baseline);

    if !filtered.new_violations.is_empty() {
        let filtered_file = create_filtered_linted_file(&file, filtered.new_violations);
        formatter.dispatch_file_violations(&filtered_file);
    }

    formatter.completion_message(1);

    // Output baseline summary
    print_baseline_summary(
        &filtered.stats,
        (filtered.stats.new_violations > 0) as usize,
    );

    (filtered.stats.new_violations > 0) as i32
}

fn create_formatter(format: Format, config: &FluffConfig) -> Arc<dyn Formatter> {
    match format {
        Format::Human => {
            let output_stream = std::io::stderr().into();
            let formatter = OutputStreamFormatter::new(
                output_stream,
                config.get("nocolor", "core").as_bool().unwrap_or_default(),
                config.get("verbose", "core").as_int().unwrap_or_default(),
            );
            Arc::new(formatter)
        }
        Format::GithubAnnotationNative => {
            let output_stream = std::io::stderr();
            let formatter = GithubAnnotationNativeFormatter::new(output_stream);
            Arc::new(formatter)
        }
        Format::Json => {
            let formatter = JsonFormatter::default();
            Arc::new(formatter)
        }
    }
}

fn create_filtered_linted_file(
    original: &LintedFile,
    new_violations: Vec<sqruff_lib_core::errors::SQLBaseError>,
) -> LintedFile {
    // We need to create a new LintedFile with only the filtered violations.
    // Since LintedFile::new requires a TemplatedFile which we can't easily clone,
    // we'll use a workaround by creating a simple mock.
    //
    // For display purposes, we only need path and violations, so we create
    // a minimal LintedFile.
    LintedFile::new(
        original.path().to_string(),
        Vec::new(), // No patches for linting (not fixing)
        sqruff_lib_core::templaters::TemplatedFile::from(original.path()),
        new_violations,
        None,
    )
}

fn print_baseline_summary(stats: &BaselineStats, files_with_new_violations: usize) {
    // Only print if we actually used the baseline
    if stats.suppressed > 0 || stats.fixed > 0 || stats.new_violations > 0 {
        eprintln!();
        eprintln!("Baseline summary:");

        if stats.suppressed > 0 {
            eprintln!("  {} violation(s) suppressed by baseline", stats.suppressed);
        }

        if stats.new_violations > 0 {
            eprintln!(
                "  {} new violation(s) in {} file(s)",
                stats.new_violations, files_with_new_violations
            );
        } else {
            eprintln!("  No new violations introduced");
        }

        if stats.fixed > 0 {
            eprintln!("  {} baseline violation(s) have been fixed", stats.fixed);
        }
    }
}
