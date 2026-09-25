#![cfg(feature = "parser")]

use assert_cmd::Command;
use std::io::Write;
use tempfile::NamedTempFile;

mod common;
use common::sqruff_path;

const INVALID_SQL_WITH_NOQA: &str = "SeLeCt  1 frm tBl ;    -- noqa\n";

#[test]
fn parse_respects_inline_noqa_for_parser_violations() {
    let output = Command::new(sqruff_path())
        .args(["parse", "--dialect", "mariadb", "--format", "none", "-"])
        .write_stdin(INVALID_SQL_WITH_NOQA)
        .assert()
        .success()
        .get_output()
        .clone();

    assert!(!String::from_utf8_lossy(&output.stderr).contains("Parse violations:"));
}

#[test]
fn parse_disable_noqa_reports_parser_violations() {
    let output = Command::new(sqruff_path())
        .args([
            "parse",
            "--dialect",
            "mariadb",
            "--disable-noqa",
            "--format",
            "none",
            "-",
        ])
        .write_stdin(INVALID_SQL_WITH_NOQA)
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Parse violations:"));
    assert!(stderr.contains("PRS"));
    assert!(stderr.contains("Unparsable section"));
}

#[test]
fn parse_omits_malformed_noqa_configured_as_warning() {
    let mut config = NamedTempFile::new().unwrap();
    writeln!(config, "[sqruff]\ndialect = ansi\nwarnings = PRS").unwrap();

    let output = Command::new(sqruff_path())
        .arg("--config")
        .arg(config.path())
        .args(["parse", "--format", "none", "-"])
        .write_stdin("select 1 --noqa missing semicolon\n")
        .assert()
        .success()
        .get_output()
        .clone();

    assert!(!String::from_utf8_lossy(&output.stderr).contains("Parse violations:"));
}
