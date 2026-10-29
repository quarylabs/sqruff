use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::{Value, json};

mod common;
use common::sqruff_path;

fn command(root: &Path) -> Command {
    let mut cmd = Command::new(sqruff_path());
    cmd.current_dir(root).args([
        "--ignore-local-config",
        "--config",
        "report.cfg",
        "lint",
        "--format",
        "gitlab",
    ]);
    cmd
}

fn fixture(warnings: bool) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let warnings = if warnings { "warnings = CP01\n" } else { "" };
    fs::write(temp.path().join("report.cfg"), format!(
        "[sqruff]\ndialect = ansi\nrules = CP01\n{warnings}[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n"
    )).unwrap();
    temp
}

#[test]
fn reports_multiple_files_with_stable_fingerprints_and_file_output() {
    let temp = fixture(false);
    for file in ["first.sql", "second.sql"] {
        fs::write(temp.path().join(file), "select 1;\n").unwrap();
    }
    let assert = command(temp.path())
        .args(["second.sql", "first.sql"])
        .assert()
        .code(1);
    let results: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    let issues = results.as_array().unwrap();
    assert_eq!(issues.len(), 2);
    for (issue, (path, fingerprint)) in issues.iter().zip([
        ("first.sql", "1ab745e80b6530959360b5ac98e3649c"),
        ("second.sql", "922df242f237397a59d087a0509963a7"),
    ]) {
        assert_eq!(issue["check_name"], "CP01");
        assert!(issue["description"].as_str().unwrap().starts_with("CP01: "));
        assert_eq!(issue["severity"], "major");
        assert_eq!(issue["fingerprint"], fingerprint);
        assert_eq!(
            issue["location"],
            json!({"path": path, "positions": {
                "begin": {"line": 1, "column": 1}, "end": {"line": 1, "column": 7}
            }})
        );
    }
    command(temp.path())
        .args(["first.sql", "second.sql", "--write-output", "report.json"])
        .assert()
        .code(1)
        .stdout("");
    let report: Value =
        serde_json::from_slice(&fs::read(temp.path().join("report.json")).unwrap()).unwrap();
    assert_eq!(report, results);
}

#[test]
fn annotation_levels_and_warning_overrides_match_upstream() {
    let temp = fixture(false);
    fs::write(temp.path().join("query.sql"), "select 1;\n").unwrap();
    let mut failure = None;
    for (level, severity) in [
        ("notice", "info"),
        ("warning", "major"),
        ("failure", "critical"),
        ("error", "critical"),
    ] {
        let assert = command(temp.path())
            .args(["query.sql", "--annotation-level", level])
            .assert()
            .code(1);
        let output = assert.get_output().stdout.clone();
        let report: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(report[0]["severity"], severity);
        if level == "failure" {
            failure = Some(output.clone());
        }
        if level == "error" {
            assert_eq!(Some(output), failure);
        }
    }
    let temp = fixture(true);
    fs::write(temp.path().join("query.sql"), "select 1;\n").unwrap();
    let assert = command(temp.path())
        .args(["query.sql", "--annotation-level", "error"])
        .assert()
        .success();
    let report: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(report[0]["severity"], "info");
}

#[test]
fn parse_errors_have_code_and_source_span_without_an_extra_flag() {
    let temp = fixture(false);
    fs::write(temp.path().join("query.sql"), "SELECT").unwrap();
    let assert = command(temp.path())
        .args(["query.sql", "--annotation-level", "warning"])
        .assert()
        .code(1);
    let report: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(report.as_array().unwrap().len(), 1);
    assert_eq!(report[0]["check_name"], "PRS");
    assert_eq!(report[0]["severity"], "major");
    assert_eq!(
        report[0]["location"]["positions"],
        json!({
            "begin": {"line": 1, "column": 1}, "end": {"line": 1, "column": 7}
        })
    );
}

#[test]
fn empty_reports_and_stdin_are_valid_json() {
    let temp = fixture(false);
    let assert = command(temp.path())
        .arg("-")
        .write_stdin("SELECT 1;\n")
        .assert()
        .success();
    assert_eq!(
        serde_json::from_slice::<Value>(&assert.get_output().stdout).unwrap(),
        json!([])
    );
    let assert = command(temp.path())
        .arg("-")
        .write_stdin("select 1;\n")
        .assert()
        .code(1);
    let report: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(report[0]["location"]["path"], "<string>");
}
