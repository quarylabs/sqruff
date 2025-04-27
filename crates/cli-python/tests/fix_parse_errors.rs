use core::str;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn main() {
    parse_errors();
}

fn parse_errors() {
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Check if we have a virtual environment at the project root
    let mut venv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    venv_path.push("../../.venv");
    if !venv_path.exists() {
        panic!(
            "Virtual environment not found at project root. Please create a .venv directory and run 'maturin develop'"
        );
    }
    // Check if sqruff script exists in the virtual environment
    let mut sqruff_path = venv_path.clone();
    sqruff_path.push("bin/sqruff");
    if !sqruff_path.exists() {
        panic!(
            "sqruff script not found in .venv/bin/sqruff. Please run 'maturin develop' in the virtual environment"
        );
    }

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
