use serde::{Deserialize, Serialize};
use sqruff_lib::core::linter::linted_file::LintedFile;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;

/// The current baseline format version.
const BASELINE_VERSION: &str = "1";

/// Default baseline filename.
pub const DEFAULT_BASELINE_FILENAME: &str = ".sqruff-baseline";

/// Represents a baseline of known violations.
///
/// The baseline uses a count-based approach similar to elm-review and ESLint's
/// native implementation. This is more stable than line-number-based approaches
/// because it doesn't get invalidated by unrelated code edits.
///
/// For each file, we track the count of violations per rule code. When comparing
/// against a baseline, we allow up to that many violations of each rule type
/// per file before reporting them as new issues.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Baseline {
    /// The version of the baseline format.
    version: String,
    /// Map of file paths to their violation counts per rule.
    /// Structure: { file_path: { rule_code: count } }
    files: BTreeMap<String, BTreeMap<String, usize>>,
}

/// Statistics about a baseline comparison.
#[derive(Debug, Default)]
pub struct BaselineStats {
    /// Number of violations that were in the baseline (suppressed).
    pub suppressed: usize,
    /// Number of violations that are new (not in baseline).
    pub new_violations: usize,
    /// Number of violations that were fixed (in baseline but not in current).
    pub fixed: usize,
}

impl Baseline {
    /// Creates a new empty baseline.
    pub fn new() -> Self {
        Self {
            version: BASELINE_VERSION.to_string(),
            files: BTreeMap::new(),
        }
    }

    /// Creates a baseline from linted files.
    pub fn from_linted_files<'a>(files: impl IntoIterator<Item = &'a LintedFile>) -> Self {
        let mut baseline = Self::new();

        for file in files {
            let path = normalize_path(file.path());
            let violations = file.violations();

            if violations.is_empty() {
                continue;
            }

            let rule_counts = baseline.files.entry(path).or_default();

            for violation in violations {
                let rule_code = violation.rule_code().to_string();
                *rule_counts.entry(rule_code).or_insert(0) += 1;
            }
        }

        baseline
    }

    /// Loads a baseline from a file path.
    pub fn load(path: &Path) -> io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let baseline: Baseline = serde_json::from_str(&content).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse baseline file: {}", e),
            )
        })?;

        // Version check
        if baseline.version != BASELINE_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Unsupported baseline version '{}'. Expected '{}'.",
                    baseline.version, BASELINE_VERSION
                ),
            ));
        }

        Ok(baseline)
    }

    /// Saves the baseline to a file path.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }

    /// Writes the baseline to stdout.
    pub fn write_to_stdout(&self) -> io::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        println!("{}", content);
        Ok(())
    }

    /// Gets the violation count for a specific file and rule.
    pub fn get_count(&self, file_path: &str, rule_code: &str) -> usize {
        let normalized = normalize_path(file_path);
        self.files
            .get(&normalized)
            .and_then(|rules| rules.get(rule_code))
            .copied()
            .unwrap_or(0)
    }

    /// Returns the total number of violations in the baseline.
    pub fn total_violations(&self) -> usize {
        self.files.values().flat_map(|rules| rules.values()).sum()
    }

    /// Returns the number of files in the baseline.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Returns an iterator over all files in the baseline.
    pub fn files(&self) -> impl Iterator<Item = &String> {
        self.files.keys()
    }

    /// Checks if the baseline is empty.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Represents counts of violations by rule for filtering.
#[derive(Debug, Default)]
struct RuleViolationCounts {
    counts: BTreeMap<String, usize>,
}

impl RuleViolationCounts {
    /// Try to consume a violation. Returns true if the violation is within
    /// the baseline allowance (should be suppressed), false if it's a new violation.
    fn try_consume(&mut self, rule_code: &str) -> bool {
        if let Some(count) = self.counts.get_mut(rule_code) {
            if *count > 0 {
                *count -= 1;
                return true;
            }
        }
        false
    }
}

/// Result of filtering violations against a baseline.
pub struct FilteredViolations {
    /// Violations that are new (not in baseline).
    pub new_violations: Vec<sqruff_lib_core::errors::SQLBaseError>,
    /// Statistics about the filtering.
    pub stats: BaselineStats,
}

/// Filters violations from a linted file against a baseline.
///
/// This function implements the count-based filtering logic:
/// - For each file/rule combination, we allow up to `baseline_count` violations
/// - Violations beyond that count are considered new
/// - Violations are processed in order (line number, then column)
pub fn filter_violations_against_baseline(
    file: &LintedFile,
    baseline: &Baseline,
) -> FilteredViolations {
    let path = normalize_path(file.path());
    let violations = file.violations();

    // Get the baseline counts for this file
    let baseline_rules = baseline.files.get(&path);

    let mut rule_counts = RuleViolationCounts::default();
    if let Some(rules) = baseline_rules {
        rule_counts.counts = rules.clone();
    }

    let mut new_violations = Vec::new();
    let mut suppressed = 0;

    // Process violations in order
    for violation in violations {
        let rule_code = violation.rule_code();
        if rule_counts.try_consume(rule_code) {
            suppressed += 1;
        } else {
            new_violations.push(violation.clone());
        }
    }

    // Calculate how many baseline violations were fixed
    // (remaining counts in baseline that weren't consumed)
    let fixed: usize = rule_counts.counts.values().sum();
    let new_violation_count = new_violations.len();

    FilteredViolations {
        new_violations,
        stats: BaselineStats {
            suppressed,
            new_violations: new_violation_count,
            fixed,
        },
    }
}

/// Normalizes a file path for consistent comparison.
/// Converts backslashes to forward slashes and removes leading "./"
fn normalize_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_new() {
        let baseline = Baseline::new();
        assert_eq!(baseline.version, BASELINE_VERSION);
        assert!(baseline.is_empty());
    }

    #[test]
    fn test_baseline_serialization() {
        let mut baseline = Baseline::new();
        baseline
            .files
            .entry("test.sql".to_string())
            .or_default()
            .insert("AL01".to_string(), 2);
        baseline
            .files
            .entry("test.sql".to_string())
            .or_default()
            .insert("CP01".to_string(), 1);

        let json = serde_json::to_string_pretty(&baseline).unwrap();
        let parsed: Baseline = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, BASELINE_VERSION);
        assert_eq!(parsed.get_count("test.sql", "AL01"), 2);
        assert_eq!(parsed.get_count("test.sql", "CP01"), 1);
        assert_eq!(parsed.get_count("test.sql", "XX99"), 0);
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("./foo/bar.sql"), "foo/bar.sql");
        assert_eq!(normalize_path("foo\\bar.sql"), "foo/bar.sql");
        assert_eq!(normalize_path("foo/bar.sql"), "foo/bar.sql");
    }

    #[test]
    fn test_rule_violation_counts() {
        let mut counts = RuleViolationCounts::default();
        counts.counts.insert("AL01".to_string(), 2);

        // First two consumptions should succeed
        assert!(counts.try_consume("AL01"));
        assert!(counts.try_consume("AL01"));
        // Third should fail (exceeded baseline)
        assert!(!counts.try_consume("AL01"));
        // Unknown rule should fail
        assert!(!counts.try_consume("XX99"));
    }

    #[test]
    fn test_total_violations() {
        let mut baseline = Baseline::new();
        baseline
            .files
            .entry("a.sql".to_string())
            .or_default()
            .insert("AL01".to_string(), 2);
        baseline
            .files
            .entry("a.sql".to_string())
            .or_default()
            .insert("CP01".to_string(), 1);
        baseline
            .files
            .entry("b.sql".to_string())
            .or_default()
            .insert("AL01".to_string(), 3);

        assert_eq!(baseline.total_violations(), 6);
        assert_eq!(baseline.file_count(), 2);
    }
}
