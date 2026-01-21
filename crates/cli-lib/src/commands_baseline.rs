use crate::baseline::Baseline;
use crate::commands::BaselineArgs;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use std::path::Path;

/// Run the baseline generation command.
///
/// This scans the specified paths for SQL files, lints them, and generates
/// a baseline file containing all current violations. The baseline can then
/// be used with `sqruff lint --baseline` to only report new violations.
pub(crate) fn run_baseline(
    args: BaselineArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    let BaselineArgs { paths, output } = args;

    // Create a linter WITHOUT a formatter (we don't want to output violations during baseline generation)
    let mut linter = Linter::new(config, None, None, collect_parse_errors);

    eprintln!("Scanning files to generate baseline...");

    // Lint the paths
    let result = linter.lint_paths(paths, false, &ignorer);

    // Collect all linted files
    let files: Vec<_> = result.into_iter().collect();

    // Create baseline from violations
    let baseline = Baseline::from_linted_files(files.iter());

    // Output summary
    let file_count = baseline.file_count();
    let violation_count = baseline.total_violations();

    if baseline.is_empty() {
        eprintln!("No violations found. Baseline is empty.");
    } else {
        eprintln!(
            "Found {} violation(s) across {} file(s).",
            violation_count, file_count
        );
    }

    // Save or output the baseline
    match output {
        Some(path) => {
            if let Err(e) = baseline.save(&path) {
                eprintln!("Error saving baseline to {}: {}", path.display(), e);
                return 1;
            }
            eprintln!("Baseline saved to: {}", path.display());
        }
        None => {
            if let Err(e) = baseline.write_to_stdout() {
                eprintln!("Error writing baseline: {}", e);
                return 1;
            }
        }
    }

    0
}

/// Run baseline generation from stdin.
pub(crate) fn run_baseline_stdin(config: FluffConfig, output: Option<std::path::PathBuf>) -> i32 {
    let read_in = match crate::stdin::read_std_in() {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading from stdin: {}", e);
            return 1;
        }
    };

    // Create a linter WITHOUT a formatter (we don't want to output violations during baseline generation)
    let linter = Linter::new(config, None, None, false);
    let result = linter.lint_string(&read_in, None, false);

    // Create baseline from the single linted file
    let baseline = Baseline::from_linted_files(std::iter::once(&result));

    // Output summary
    let violation_count = baseline.total_violations();
    eprintln!("Found {} violation(s) in stdin.", violation_count);

    // Save or output the baseline
    match output {
        Some(path) => {
            if let Err(e) = baseline.save(&path) {
                eprintln!("Error saving baseline to {}: {}", path.display(), e);
                return 1;
            }
            eprintln!("Baseline saved to: {}", path.display());
        }
        None => {
            if let Err(e) = baseline.write_to_stdout() {
                eprintln!("Error writing baseline: {}", e);
                return 1;
            }
        }
    }

    0
}
