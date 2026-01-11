use core::str;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

fn main() {
    test_baseline_generation();
    test_baseline_lint_suppresses_existing();
    test_baseline_lint_detects_new_violations();
    test_baseline_lint_reports_fixed_violations();
    test_baseline_file_not_found();
}

fn get_sqruff_path() -> PathBuf {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));
    sqruff_path
}

/// Test that baseline generation works and creates valid JSON.
fn test_baseline_generation() {
    let sqruff_path = get_sqruff_path();
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Create a temporary directory with a SQL file that has violations
    let temp_dir = TempDir::new().unwrap();
    let sql_file = temp_dir.path().join("test.sql");
    std::fs::write(&sql_file, "select a,b from foo").unwrap();

    let mut cmd = Command::new(sqruff_path);
    cmd.env("HOME", PathBuf::from(cargo_folder));
    cmd.arg("baseline").arg(temp_dir.path());
    cmd.current_dir(cargo_folder);

    let assert = cmd.assert();
    let output = assert.get_output();

    // Should exit with 0
    assert_eq!(output.status.code().unwrap(), 0);

    // stdout should contain valid JSON
    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    let baseline: serde_json::Value = serde_json::from_str(stdout_str).expect("Should be valid JSON");

    // Check baseline structure
    assert_eq!(baseline["version"], "1");
    assert!(baseline["files"].is_object());

    // Should have exactly one file with LT01 violation
    let files = baseline["files"].as_object().unwrap();
    assert_eq!(files.len(), 1);

    // Check violation count
    for (_path, violations) in files {
        assert!(violations["LT01"].as_u64().unwrap() >= 1);
    }
}

/// Test that linting with a baseline suppresses existing violations.
fn test_baseline_lint_suppresses_existing() {
    let sqruff_path = get_sqruff_path();
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();

    // Create a SQL file with violations
    let sql_file = temp_dir.path().join("test.sql");
    std::fs::write(&sql_file, "select a,b from foo").unwrap();

    // Generate baseline
    let baseline_file = temp_dir.path().join(".sqruff-baseline");
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(cargo_folder));
    cmd.arg("baseline")
        .arg("-o")
        .arg(&baseline_file)
        .arg(temp_dir.path());
    cmd.current_dir(cargo_folder);
    cmd.assert().success();

    // Lint with baseline - should pass (exit 0) since all violations are baselined
    let mut cmd = Command::new(sqruff_path);
    cmd.env("HOME", PathBuf::from(cargo_folder));
    cmd.arg("lint")
        .arg("--baseline")
        .arg(&baseline_file)
        .arg(temp_dir.path());
    cmd.current_dir(cargo_folder);

    let assert = cmd.assert();
    let output = assert.get_output();

    // Should exit with 0 (no new violations)
    assert_eq!(output.status.code().unwrap(), 0);

    // stderr should mention suppressed violations
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr_str.contains("suppressed by baseline"));
    assert!(stderr_str.contains("No new violations"));
}

/// Test that linting with a baseline detects new violations.
fn test_baseline_lint_detects_new_violations() {
    let sqruff_path = get_sqruff_path();
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();

    // Create a SQL file with one violation
    let sql_file = temp_dir.path().join("test.sql");
    std::fs::write(&sql_file, "select a,b from foo").unwrap();

    // Generate baseline
    let baseline_file = temp_dir.path().join(".sqruff-baseline");
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(cargo_folder));
    cmd.arg("baseline")
        .arg("-o")
        .arg(&baseline_file)
        .arg(temp_dir.path());
    cmd.current_dir(cargo_folder);
    cmd.assert().success();

    // Add a new violation to the file
    std::fs::write(&sql_file, "select a,b,c from foo").unwrap();

    // Lint with baseline - should fail (exit 1) due to new violation
    let mut cmd = Command::new(sqruff_path);
    cmd.env("HOME", PathBuf::from(cargo_folder));
    cmd.arg("lint")
        .arg("--baseline")
        .arg(&baseline_file)
        .arg(temp_dir.path());
    cmd.current_dir(cargo_folder);

    let assert = cmd.assert();
    let output = assert.get_output();

    // Should exit with 1 (new violations)
    assert_eq!(output.status.code().unwrap(), 1);

    // stderr should mention new violations
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr_str.contains("new violation"));
}

/// Test that linting with a baseline reports fixed violations.
fn test_baseline_lint_reports_fixed_violations() {
    let sqruff_path = get_sqruff_path();
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();

    // Create a SQL file with violations
    let sql_file = temp_dir.path().join("test.sql");
    std::fs::write(&sql_file, "select a,b from foo").unwrap();

    // Generate baseline
    let baseline_file = temp_dir.path().join(".sqruff-baseline");
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(cargo_folder));
    cmd.arg("baseline")
        .arg("-o")
        .arg(&baseline_file)
        .arg(temp_dir.path());
    cmd.current_dir(cargo_folder);
    cmd.assert().success();

    // Fix the violation in the file
    std::fs::write(&sql_file, "SELECT a, b FROM foo").unwrap();

    // Lint with baseline - should pass and report fixed violations
    let mut cmd = Command::new(sqruff_path);
    cmd.env("HOME", PathBuf::from(cargo_folder));
    cmd.arg("lint")
        .arg("--baseline")
        .arg(&baseline_file)
        .arg(temp_dir.path());
    cmd.current_dir(cargo_folder);

    let assert = cmd.assert();
    let output = assert.get_output();

    // Should exit with 0
    assert_eq!(output.status.code().unwrap(), 0);

    // stderr should mention fixed violations
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr_str.contains("have been fixed"));
}

/// Test that linting with a non-existent baseline file errors appropriately.
fn test_baseline_file_not_found() {
    let sqruff_path = get_sqruff_path();
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Create a temporary directory with a SQL file
    let temp_dir = TempDir::new().unwrap();
    let sql_file = temp_dir.path().join("test.sql");
    std::fs::write(&sql_file, "select a,b from foo").unwrap();

    // Try to lint with non-existent baseline
    let mut cmd = Command::new(sqruff_path);
    cmd.env("HOME", PathBuf::from(cargo_folder));
    cmd.arg("lint")
        .arg("--baseline")
        .arg(temp_dir.path().join("nonexistent.baseline"))
        .arg(temp_dir.path());
    cmd.current_dir(cargo_folder);

    let assert = cmd.assert();
    let output = assert.get_output();

    // Should exit with 1 (error)
    assert_eq!(output.status.code().unwrap(), 1);

    // stderr should mention error loading baseline
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr_str.contains("Error loading baseline"));
}
