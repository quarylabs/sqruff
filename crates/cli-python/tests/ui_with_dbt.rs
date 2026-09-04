use expect_test::expect_file;

mod common;
use common::{copy_dir, manifest_dir, sqruff_command};

#[test]
fn ui_with_dbt() {
    let source_dbt_dir = manifest_dir()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/cli-python/tests/dbt_sample/");
    let temp_dir = tempfile::tempdir().unwrap();
    let sample_dbt_dir = temp_dir.path().join("dbt_sample");
    copy_dir(&source_dbt_dir, &sample_dbt_dir);
    let output_dir = manifest_dir().join("tests/dbt");
    // Create the output directory
    std::fs::create_dir_all(&output_dir).unwrap();

    let mut cmd = sqruff_command();
    cmd.current_dir(&sample_dbt_dir);
    for (key, value) in std::env::vars() {
        cmd.env(key, value);
    }
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--parsing-errors")
        .arg("models/");

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
