use core::str;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn main() {
    fix_return_code();
}

fn fix_return_code() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    // Tests needed
    // STDIN
    // - Fix, do nothing -> 0
    // - Fix, fix everything -> 0
    // - Fix, fix some not all -> 1
    // TODO File
    // - Fix, do nothing -> 0
    // - Fix, fix everything -> 0
    // - Fix, fix some not all -> 1

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // STDIN - do nothing
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("fix").arg("-f").arg("human").arg("-");
    cmd.current_dir(cargo_folder);
    cmd.write_stdin("SELECT foo FROM bar;\n");

    // Run the command and capture the output
    // let assert = cmd.assert();
    // let output = assert.get_output();

    // assert!(output.status.code().unwrap().to_string() == "0");

    // let stdout_str = str::from_utf8(&output.stdout).unwrap();
    // let stderr_str = str::from_utf8(&output.stderr).unwrap();
    // assert_eq!(stdout_str, "SELECT foo FROM bar;\n\n");
    // assert_eq!(stderr_str, "");

    // STDIN - nothing to fix
    let config_file = cargo_folder.join("tests/fix_return_code/fix_everything.cfg");
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("fix")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(&config_file)
        .arg("-");
    cmd.write_stdin("SELECT foo AS bar FROM tabs");

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert_eq!(stdout_str, "SELECT foo AS bar FROM tabs\n");
    assert_eq!(stderr_str, "");
    assert_eq!(output.status.code().unwrap(), 0);

    // STDIN - fix everything
    let config_file = cargo_folder.join("tests/fix_return_code/fix_everything.cfg");
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("fix")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(&config_file)
        .arg("-");
    cmd.write_stdin("SELECT foo bar FROM tabs");

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert_eq!(stdout_str, "SELECT foo AS bar FROM tabs\n");
    assert_eq!(
        stderr_str,
        "== [<string>] FAIL\nL:   1 | P:  12 | AL02 | Implicit/explicit aliasing of columns.\n                       | [aliasing.column]\n"
    );
    assert_eq!(output.status.code().unwrap(), 0);

    // STDIN - fix some not all
    let config_file = cargo_folder.join("tests/fix_return_code/fix_some.cfg");
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("fix")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(&config_file)
        .arg("-");
    cmd.write_stdin("SELECT foo bar, * FROM tabs");

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    let stderr_str = str::from_utf8(&output.stderr).unwrap();
    assert_eq!(stdout_str, "SELECT foo AS bar, * FROM tabs\n");
    assert_eq!(
        stderr_str,
        "== [<string>] FAIL\nL:   1 | P:   1 | AM04 | Outermost query should produce known number of columns.\n                       | [ambiguous.column_count]\nL:   1 | P:  12 | AL02 | Implicit/explicit aliasing of columns.\n                       | [aliasing.column]\n"
    );
    assert_eq!(output.status.code().unwrap(), 1);
}
