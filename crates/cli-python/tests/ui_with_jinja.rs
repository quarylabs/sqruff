use expect_test::expect_file;

mod common;
use common::{copy_dir, manifest_dir, sqruff_command};

#[test]
fn ui_with_jinja() {
    let source_dir = manifest_dir().join("tests/jinja");
    let temp_dir = tempfile::tempdir().unwrap();
    let test_dir = temp_dir.path().join("jinja");
    copy_dir(&source_dir, &test_dir);

    // Set up the command with arguments
    let mut cmd = sqruff_command();
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(test_dir.join(".sqruff"))
        .arg(&test_dir);

    // Pass all the environment variables to the command
    for (key, value) in std::env::vars() {
        cmd.env(key, value);
    }

    // Set the HOME environment variable to the fake home directory
    let home_path = manifest_dir();
    cmd.env("HOME", home_path);

    // Run the command and capture the output
    let assert = cmd.assert();

    // Construct the expected output file path
    let mut expected_output_path_stderr = manifest_dir();
    expected_output_path_stderr.push("tests/jinja/expected_output.stderr");
    let mut expected_output_path_stdout = manifest_dir();
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
