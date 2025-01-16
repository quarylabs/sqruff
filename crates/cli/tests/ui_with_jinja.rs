use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use expect_test::expect_file;

fn main() {
    let mut test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    test_dir.push("tests/jinja");

    // Iterate over each test file in the directory
    for entry in fs::read_dir(&test_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        // Check if the file has a .sql or .hql extension
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map_or(false, |ext| ext == "sql" || ext == "hql")
        {
            // Create a temporary directory
            let temp_dir = tempfile::tempdir().unwrap();
            let sqruff_path = temp_dir.path().join("debug").join("sqruff");

            // Make sure sqruff is built with python feature
            // Build the binary with the python feature in the test directory
            Command::new("cargo")
                .args([
                    "build",
                    "--features",
                    "python",
                    "--target-dir",
                    &temp_dir.path().to_string_lossy(),
                ])
                .assert();

            // Set up the command with arguments
            let mut cmd = Command::new(sqruff_path);
            cmd.arg("lint")
                .arg("-f")
                .arg("human")
                .arg("--config")
                .arg("tests/jinja/.sqruff")
                .arg(&path);

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
            let mut expected_output_path_stderr = path.clone();
            expected_output_path_stderr.set_extension("stderr");
            let mut expected_output_path_stdout = path.clone();
            expected_output_path_stdout.set_extension("stdout");

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
    }
}
