use core::str;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn main() {
    parse_errors();
    parse_errors_do_not_rewrite_files();
    multiple_add_column_errors();
}

fn parse_errors() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // STDIN - do nothing
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("fix")
        .arg("-f")
        .arg("human")
        .arg("--parsing-errors")
        .arg("-");
    cmd.current_dir(cargo_folder);
    cmd.write_stdin("SelEc");

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert_eq!(stdout_str, "SelEc\n\n");
    assert_eq!(
        stderr_str,
        "== [<string>] FAIL\nL:   1 | P:   1 | ???? | Unparsable section\nL:   1 | P:   1 | LT12 | Files must end with a single trailing newline.\n                       | [layout.end_of_file]\n"
    );
    assert_eq!(output.status.code().unwrap(), 1);
}

fn parse_errors_do_not_rewrite_files() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    let temp_dir = tempfile::tempdir().unwrap();
    let sql_path = temp_dir.path().join("parse_error.sql");
    let original = "SelEc";
    fs::write(&sql_path, original).unwrap();

    let mut cmd = Command::new(sqruff_path);
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("fix")
        .arg("-f")
        .arg("human")
        .arg("--parsing-errors")
        .arg(&sql_path);
    cmd.current_dir(cargo_folder);

    let assert = cmd.assert();
    let output = assert.get_output();

    assert_eq!(output.status.code().unwrap(), 1);
    assert_eq!(fs::read_to_string(&sql_path).unwrap(), original);
}

fn multiple_add_column_errors() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    let sql = "ALTER TABLE workflows.executions\nADD COLUMN IF NOT EXISTS workflow_group VARCHAR(50)\nADD COLUMN IF NOT EXISTS workflow_name VARCHAR(50)\nADD COLUMN IF NOT EXISTS workflow_version VARCHAR(50);";

    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("fix")
        .arg("-f")
        .arg("human")
        .arg("--parsing-errors")
        .arg("-");
    cmd.current_dir(cargo_folder);
    cmd.write_stdin(sql);

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert!(stdout_str.contains("ALTER TABLE workflows.executions"));
    assert!(stderr_str.contains("Unparsable section"));
    assert_eq!(output.status.code().unwrap(), 1);
}
