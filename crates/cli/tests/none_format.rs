use core::str;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn main() {
    none_format();
}

/// The `none` output format produces no stdout output. It is used mostly for
/// testing. Mirrors SQLFluff's `none` format type (sqlfluff#4704).
fn none_format() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // LINT - violations present, but `none` format produces no output while
    // still returning a non-zero exit code.
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("lint").arg("-f").arg("none").arg("-");
    cmd.current_dir(cargo_folder);
    cmd.write_stdin("SELECT foo bar FROM tabs");

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(stdout_str, "");
    assert_eq!(output.status.code().unwrap(), 1);

    // LINT - no violations, `none` format produces no output, exit code 0.
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    cmd.arg("lint").arg("-f").arg("none").arg("-");
    cmd.current_dir(cargo_folder);
    cmd.write_stdin("SELECT foo AS bar FROM tabs\n");

    let assert = cmd.assert();
    let output = assert.get_output();

    let stdout_str = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(stdout_str, "");
    assert_eq!(output.status.code().unwrap(), 0);
}
