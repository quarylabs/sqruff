use core::str;
use std::io::Write;

use assert_cmd::Command;
use tempfile::NamedTempFile;

mod common;
use common::{manifest_dir, sqruff_path};

#[test]
fn fix_parse_errors() {
    parse_errors();
    parse_errors_with_fix_opt_in();
    multiple_add_column_errors();
}

fn parse_errors() {
    let cargo_folder = manifest_dir();
    let sqruff_path = sqruff_path();

    // STDIN - do nothing
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", &cargo_folder);
    cmd.arg("fix")
        .arg("-f")
        .arg("human")
        .arg("--parsing-errors")
        .arg("-");
    cmd.current_dir(&cargo_folder);
    cmd.write_stdin("SelEc");

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert_eq!(stdout_str, "SelEc");
    assert_eq!(
        stderr_str,
        "== [<string>] FAIL\nL:   1 | P:   1 | ???? | Unparsable section\nL:   1 | P:   1 | LT12 | Files must end with a single trailing newline.\n                       | [layout.end_of_file]\n"
    );
    assert_eq!(output.status.code().unwrap(), 1);
}

fn parse_errors_with_fix_opt_in() {
    let cargo_folder = manifest_dir();
    let sqruff_path = sqruff_path();
    let mut config = NamedTempFile::new().unwrap();
    writeln!(
        config,
        "[sqruff]\ndialect = ansi\nfix_even_unparsable = True"
    )
    .unwrap();

    let mut cmd = Command::new(sqruff_path);
    cmd.env("HOME", &cargo_folder);
    cmd.arg("--config")
        .arg(config.path())
        .arg("--parsing-errors")
        .arg("fix")
        .arg("-f")
        .arg("none")
        .arg("-");
    cmd.current_dir(&cargo_folder);
    cmd.write_stdin("SelEc");

    let assert = cmd.assert();
    let output = assert.get_output();

    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "SelEc\n");
    assert_eq!(output.status.code().unwrap(), 1);
}

fn multiple_add_column_errors() {
    let cargo_folder = manifest_dir();
    let sqruff_path = sqruff_path();

    let sql = "ALTER TABLE workflows.executions\nADD COLUMN IF NOT EXISTS workflow_group VARCHAR(50)\nADD COLUMN IF NOT EXISTS workflow_name VARCHAR(50)\nADD COLUMN IF NOT EXISTS workflow_version VARCHAR(50);";

    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", &cargo_folder);
    cmd.arg("fix")
        .arg("-f")
        .arg("human")
        .arg("--parsing-errors")
        .arg("-");
    cmd.current_dir(&cargo_folder);
    cmd.write_stdin(sql);

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert!(stdout_str.contains("ALTER TABLE workflows.executions"));
    assert!(stderr_str.contains("Unparsable section"));
    assert_eq!(output.status.code().unwrap(), 1);
}
