use std::path::PathBuf;

use assert_cmd::Command;
use expect_test::expect_file;

fn main() {
    configure_rule();
}

///
fn configure_rule() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let file_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/configure_rule");

    // Construct the path to the sqruff binary
    let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("configure_rule")
        .join("example.sql");
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("configure_rule")
        .join("config.cfg");

    // Set up the command with arguments
    let mut cmd = Command::new(sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(&config_path)
        .arg(&path);
    // Set the HOME environment variable to the fake home directory
    cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    // Run the command and capture the output
    let assert = cmd.assert();

    // Construct the expected output file paths
    let mut expected_output_path_stderr = path.clone();
    expected_output_path_stderr.set_extension("stderr");
    let mut expected_output_path_stdout = path.clone();
    expected_output_path_stdout.set_extension("stdout");
    let mut expected_output_path_exitcode = path.clone();
    expected_output_path_exitcode.set_extension("exitcode");

    // Read the expected output
    let output = assert.get_output();
    let stderr_str = std::str::from_utf8(&output.stderr).unwrap();
    let stdout_str = std::str::from_utf8(&output.stdout).unwrap();
    let exit_code_str = output.status.code().unwrap().to_string();

    let test_dir_str = file_dir.to_string_lossy().to_string();
    let stderr_normalized: String = stderr_str.replace(&test_dir_str, "tests/configure_rule");
    let stdout_normalized: String = stdout_str.replace(&test_dir_str, "tests/configure_rule");

    expect_file![expected_output_path_stderr].assert_eq(&stderr_normalized);
    expect_file![expected_output_path_stdout].assert_eq(&stdout_normalized);
    expect_file![expected_output_path_exitcode].assert_eq(&exit_code_str);
}
