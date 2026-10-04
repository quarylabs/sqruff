use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

mod common;
use common::{manifest_dir, sqruff_path};

fn lt01_config() -> std::path::PathBuf {
    manifest_dir().join("tests/quiet_output/lt01.cfg")
}

#[test]
fn lint_quiet_suppresses_status_but_preserves_diagnostics() {
    let dir = tempdir().unwrap();
    let mut clean = Command::new(sqruff_path());
    clean
        .current_dir(dir.path())
        .args(["lint", "--dialect", "ansi", "--quiet", "-"])
        .write_stdin("SELECT 1\n");
    let output = clean.assert().code(0).get_output().clone();
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let mut invalid = Command::new(sqruff_path());
    invalid
        .current_dir(dir.path())
        .args(["lint", "--config"])
        .arg(lt01_config())
        .args(["-q", "-"])
        .write_stdin("SELECT 1 ,4\n");
    let output = invalid.assert().code(1).get_output().clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("LT01"), "{stderr}");
    assert!(!stderr.contains("All Finished"), "{stderr}");
}

#[test]
fn lint_quiet_preserves_machine_output() {
    let dir = tempdir().unwrap();
    let mut command = Command::new(sqruff_path());
    command
        .current_dir(dir.path())
        .args(["lint", "--config"])
        .arg(lt01_config())
        .args(["-q", "-f", "json", "-"])
        .write_stdin("SELECT 1 ,4\n");
    let output = command.assert().code(1).get_output().clone();
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.is_object());
    assert!(output.stderr.is_empty());
}

#[test]
fn lint_quiet_preserves_warnings_without_failing() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("warnings.cfg");
    fs::write(
        &config_path,
        "[sqruff]\ndialect = ansi\nrules = LT01\nwarnings = LT01\n",
    )
    .unwrap();
    let mut command = Command::new(sqruff_path());
    command
        .current_dir(dir.path())
        .args(["lint", "--config"])
        .arg(&config_path)
        .args(["-q", "-"])
        .write_stdin("SELECT 1 ,4\n");
    let output = command.assert().code(0).get_output().clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("WARNING"), "{stderr}");
    assert!(stderr.contains("LT01"), "{stderr}");
    assert!(!stderr.contains("All Finished"), "{stderr}");
}

#[test]
fn fix_quiet_preserves_stdin_sql_and_unfixable_diagnostics() {
    let dir = tempdir().unwrap();
    let mut fixable = Command::new(sqruff_path());
    fixable
        .current_dir(dir.path())
        .args(["fix", "--config"])
        .arg(lt01_config())
        .args(["-q", "-"])
        .write_stdin("SELECT 1 ,4\n");
    let output = fixable.assert().code(0).get_output().clone();
    assert_eq!(output.stdout, b"SELECT 1, 4\n");
    assert!(output.stderr.is_empty());

    let mut invalid = Command::new(sqruff_path());
    invalid
        .current_dir(dir.path())
        .args(["fix", "--dialect", "ansi", "-q", "-"])
        .write_stdin("SELECT 1 FROM\n");
    let output = invalid.assert().code(1).get_output().clone();
    assert_eq!(output.stdout, b"SELECT 1 FROM\n");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Unparsable"), "{stderr}");
    assert!(!stderr.contains("All Finished"), "{stderr}");
}

#[test]
fn fix_quiet_preserves_machine_output_for_clean_files() {
    let dir = tempdir().unwrap();
    let sql_path = dir.path().join("clean.sql");
    fs::write(&sql_path, "SELECT 1\n").unwrap();
    let mut command = Command::new(sqruff_path());
    command
        .current_dir(dir.path())
        .args(["fix", "--dialect", "ansi", "-q", "-f", "json"])
        .arg(&sql_path);
    let output = command.assert().code(0).get_output().clone();
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.is_object());
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_to_string(&sql_path).unwrap(), "SELECT 1\n");
}

#[test]
fn fix_quiet_updates_file_without_routine_output() {
    let dir = tempdir().unwrap();
    let sql_path = dir.path().join("fix.sql");
    fs::write(&sql_path, "SELECT 1 ,4\n").unwrap();
    let mut command = Command::new(sqruff_path());
    command
        .current_dir(dir.path())
        .args(["fix", "--config"])
        .arg(lt01_config())
        .arg("-q")
        .arg(&sql_path);
    let output = command.assert().code(0).get_output().clone();
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_to_string(&sql_path).unwrap(), "SELECT 1, 4\n");
}

#[test]
fn quiet_overrides_configured_verbosity_and_conflicts_with_verbose() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("verbose.cfg");
    fs::write(&config_path, "[sqruff]\ndialect = ansi\nverbose = 2\n").unwrap();

    let mut quiet = Command::new(sqruff_path());
    quiet
        .current_dir(dir.path())
        .args(["lint", "--config"])
        .arg(&config_path)
        .args(["-q", "-"])
        .write_stdin("SELECT 1\n");
    let output = quiet.assert().code(0).get_output().clone();
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    for command in ["lint", "fix"] {
        let mut conflict = Command::new(sqruff_path());
        conflict
            .current_dir(dir.path())
            .args([command, "-q", "-v", "-"])
            .write_stdin("SELECT 1\n");
        conflict.assert().failure();
    }
}
