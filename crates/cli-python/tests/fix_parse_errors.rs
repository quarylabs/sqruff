mod common;

use core::str;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn main() {
    parse_errors();
}

fn parse_errors() {
    let cargo_folder = Path::new(common::manifest_dir());
    let sqruff_path = common::sqruff_path();

    // STDIN - do nothing
    let mut cmd = Command::new(sqruff_path.clone());
    cmd.env("HOME", PathBuf::from(common::manifest_dir()));
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
    assert_eq!(stdout_str, "SelEc");
    assert_eq!(
        stderr_str,
        "== [<string>] FAIL\nL:   1 | P:   1 | ???? | Unparsable section\nL:   1 | P:   1 | LT12 | Files must end with a single trailing newline.\n                       | [layout.end_of_file]\n"
    );
    assert_eq!(output.status.code().unwrap(), 1);
}
