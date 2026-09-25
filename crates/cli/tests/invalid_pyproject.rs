use std::fs;

use assert_cmd::Command;

mod common;
use common::sqruff_path;

#[test]
fn invalid_pyproject_toml_is_a_concise_user_error() {
    let temp = tempfile::tempdir().unwrap();
    let pyproject = temp.path().join("pyproject.toml");
    let sql = temp.path().join("query.sql");
    fs::write(
        &pyproject,
        "\u{feff}[tool.sqlfluff.core]\ndialect = \"ansi\"\n",
    )
    .unwrap();
    fs::write(&sql, "SELECT 1\n").unwrap();

    let assertion = Command::new(sqruff_path())
        .current_dir(temp.path())
        .env("HOME", temp.path())
        .args(["lint", "--dialect", "ansi"])
        .arg(&sql)
        .assert()
        .code(1);
    let stderr = String::from_utf8_lossy(&assertion.get_output().stderr);

    assert!(stderr.contains("Failed to parse TOML config file"));
    assert!(stderr.contains(pyproject.to_string_lossy().as_ref()));
    assert!(stderr.contains("line 1, column 1"));
    assert!(stderr.contains("UTF-8 BOM"));
    assert!(!stderr.contains("panicked"));
    assert!(!stderr.contains("backtrace"));
}
