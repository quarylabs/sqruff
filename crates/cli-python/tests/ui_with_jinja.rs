use std::path::PathBuf;

use assert_cmd::Command;
use expect_test::expect_file;

fn main() {
    let mut test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    test_dir.push("tests/jinja");

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
    // Set up the command with arguments
    let mut cmd = Command::new(sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg("tests/jinja/.sqruff")
        .arg("tests/jinja");

    // Pass all the environment variables to the command
    for (key, value) in std::env::vars() {
        cmd.env(key, value);
    }

    // Set the HOME environment variable to the fake home directory
    let home_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cmd.env("HOME", home_path);

    // Run the command and capture the output
    let assert = cmd.assert();

    // Construct the expected output file path
    let mut expected_output_path_stderr = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    expected_output_path_stderr.push("tests/jinja/expected_output.stderr");
    let mut expected_output_path_stdout = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    expected_output_path_stdout.push("tests/jinja/expected_output.stdout");

    // Read the expected output
    let output = assert.get_output();
    let stderr_str = std::str::from_utf8(&output.stderr).unwrap();
    let stdout_str = std::str::from_utf8(&output.stdout).unwrap();

    let test_dir_str = test_dir.to_string_lossy().to_string();
    let stderr_normalized: String = stderr_str.replace(&test_dir_str, "tests/jinja");
    let stdout_normalized: String = stdout_str.replace(&test_dir_str, "tests/jinja");

    expect_file![expected_output_path_stderr].assert_eq(&stderr_normalized);
    expect_file![expected_output_path_stdout].assert_eq(&stdout_normalized);
}
