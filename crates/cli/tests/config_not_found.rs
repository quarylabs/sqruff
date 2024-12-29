use std::path::PathBuf;

use assert_cmd::Command;
use expect_test::expect_file;

fn main() {
    config_not_found_lint();
}

fn config_not_found_lint() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut lint_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    lint_dir.push("tests/config_not_found");

    let entry_path = lint_dir.as_path().join("example.sql");
    let config_path = lint_dir.as_path().join("non_existant.cfg");

    // Check if the file has a .sql or .hql extension
    // Construct the path to the sqruff binary
    let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // Set up the command with arguments
    let mut cmd = Command::new(sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(&config_path)
        .arg(&entry_path);
    // Set the HOME environment variable to the fake home directory
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    // Run the command and capture the output
    let assert = cmd.assert();

    // Construct the expected output file paths
    let storage_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("config_not_found")
        .join("example.sql");
    let mut expected_output_path_stderr = storage_path.clone();
    expected_output_path_stderr.set_extension("stderr");
    let mut expected_output_path_stdout = storage_path.clone();
    expected_output_path_stdout.set_extension("stdout");
    let mut expected_output_path_exitcode = storage_path.clone();
    expected_output_path_exitcode.set_extension("exitcode");

    // Read the expected output
    let output = assert.get_output();
    let stderr_str = std::str::from_utf8(&output.stderr).unwrap();
    let stdout_str = std::str::from_utf8(&output.stdout).unwrap();
    let exit_code_str = output.status.code().unwrap().to_string();

    let test_dir_str = lint_dir.to_string_lossy().to_string();
    let stderr_normalized: String = stderr_str.replace(&test_dir_str, "tests/config_not_found");
    let stdout_normalized: String = stdout_str.replace(&test_dir_str, "tests/config_not_found");

    expect_file![expected_output_path_stderr].assert_eq(&stderr_normalized);
    expect_file![expected_output_path_stdout].assert_eq(&stdout_normalized);
    expect_file![expected_output_path_exitcode].assert_eq(&exit_code_str);
}
