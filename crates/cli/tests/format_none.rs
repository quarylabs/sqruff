use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

/// Tests that the `none` output format produces no output.
///
/// Mirrors SQLFluff's behaviour where `--format none` returns no output and is
/// mostly used for testing and benchmarking. Only the exit code should carry
/// information about whether violations were found.
fn main() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Construct the path to the sqruff binary
    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    let tmp_dir = tempfile::tempdir().unwrap();

    // A file with lint violations should produce no output but a non-zero exit
    // code.
    let failing_path = tmp_dir.path().join("failing.sql");
    fs::write(&failing_path, "SELECT 1 from foo\n").unwrap();

    let mut cmd = Command::new(&sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("none")
        .arg("--dialect")
        .arg("ansi")
        .arg(&failing_path);
    cmd.env("HOME", cargo_folder);
    cmd.current_dir(tmp_dir.path());
    let output = cmd.assert();
    let output = output.get_output();
    assert_eq!(output.status.code().unwrap(), 1);
    assert_eq!(std::str::from_utf8(&output.stdout).unwrap(), "");
    assert_eq!(std::str::from_utf8(&output.stderr).unwrap(), "");

    // A clean file should produce no output and a zero exit code.
    let passing_path = tmp_dir.path().join("passing.sql");
    fs::write(&passing_path, "SELECT 1 FROM foo\n").unwrap();

    let mut cmd = Command::new(&sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("none")
        .arg("--dialect")
        .arg("ansi")
        .arg(&passing_path);
    cmd.env("HOME", cargo_folder);
    cmd.current_dir(tmp_dir.path());
    let output = cmd.assert();
    let output = output.get_output();
    assert_eq!(output.status.code().unwrap(), 0);
    assert_eq!(std::str::from_utf8(&output.stdout).unwrap(), "");
    assert_eq!(std::str::from_utf8(&output.stderr).unwrap(), "");
}
