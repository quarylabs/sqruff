use assert_cmd::Command;
use std::io::Write;
use tempfile::NamedTempFile;

mod common;
use common::sqruff_path;

const SQL: &str = "SELECT a, b FROM example";

#[test]
fn default_build_prints_cst_from_files_and_stdin() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{SQL}").unwrap();

    let file_output = Command::new(sqruff_path())
        .args(["parse", "--dialect", "ansi"])
        .arg(file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    for paths in [vec!["-"], vec![]] {
        let stdin_output = Command::new(sqruff_path())
            .args(["parse", "--dialect", "ansi"])
            .args(paths)
            .write_stdin(SQL)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let file_tree = String::from_utf8_lossy(&file_output);
        let stdin_tree = String::from_utf8_lossy(&stdin_output);
        // The first line identifies the input; the tree itself must be identical.
        assert_eq!(
            file_tree.lines().skip(1).collect::<Vec<_>>(),
            stdin_tree.lines().skip(1).collect::<Vec<_>>()
        );
        for segment_type in ["select_clause:", "comma:", "from_clause:"] {
            assert!(stdin_tree.contains(segment_type), "missing {segment_type}");
        }
    }
}

#[test]
fn parse_reports_invalid_sql_while_printing_the_cst() {
    let output = Command::new(sqruff_path())
        .args(["parse", "--dialect", "ansi", "-"])
        .write_stdin("SELECT 1 frm example")
        .assert()
        .failure()
        .get_output()
        .clone();
    assert!(String::from_utf8_lossy(&output.stdout).contains("unparsable:"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("PRS"));
}

#[test]
fn parse_rejects_mixing_stdin_and_files() {
    let output = Command::new(sqruff_path())
        .args(["parse", "--dialect", "ansi", "-", "query.sql"])
        .assert()
        .failure()
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Cannot mix stdin flag"));
}
