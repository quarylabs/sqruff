use std::path::PathBuf;

use assert_cmd::Command;
use expect_test::expect_file;

fn main() {
    let sample_dbt_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/cli-python/tests/dbt_sample/");
    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/dbt");
    // Create the output directory
    std::fs::create_dir_all(&output_dir).unwrap();

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

    let mut cmd = Command::new(sqruff_path);
    cmd.current_dir(&sample_dbt_dir);
    for (key, value) in std::env::vars() {
        cmd.env(key, value);
    }
    cmd.arg("lint").arg("-f").arg("human").arg("models/");

    // Run the command and capture the output
    let assert = cmd.assert();

    // Construct the expected output file path
    let expected_output_path_stderr = output_dir.join("output.stderr");
    let expected_output_path_stdout = output_dir.join("output.stdout");
    let exepcted_code = output_dir.join("output.code");

    // Read the expected output
    let output = assert.get_output();

    let stderr_str = std::str::from_utf8(&output.stderr).unwrap();
    let stdout_str = std::str::from_utf8(&output.stdout).unwrap();

    let stderr_normalized: String =
        stderr_str.replace(&sample_dbt_dir.to_string_lossy().to_string(), "tests/dbt");
    let stdout_normalized: String =
        stdout_str.replace(&sample_dbt_dir.to_string_lossy().to_string(), "tests/dbt");

    expect_file![expected_output_path_stderr].assert_eq(&stderr_normalized);
    expect_file![expected_output_path_stdout].assert_eq(&stdout_normalized);
    expect_file![exepcted_code].assert_eq(&output.status.code().unwrap().to_string());
}
