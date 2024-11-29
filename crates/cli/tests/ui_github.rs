use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use expect_test::expect_file;

fn main() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut lint_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    lint_dir.push("tests/github");

    // Iterate over each test file in the directory
    for entry in fs::read_dir(&lint_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        // Check if the file has a .sql or .hql extension
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map_or(false, |ext| ext == "sql" || ext == "hql")
        {
            // Construct the path to the sqruff binary
            let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            sqruff_path.push(format!("../../target/{}/sqruff", profile));

            // Set up the command with arguments
            let mut cmd = Command::new(sqruff_path);

            cmd.arg("lint");
            cmd.arg(path.to_str().unwrap());
            // Set the HOME environment variable to the fake home directory
            cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));
            cmd.env("GITHUB_ACTIONS", "true");

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

            let test_dir_str = lint_dir.to_string_lossy().to_string();
            let stderr_normalized: String = stderr_str.replace(&test_dir_str, "tests/lint");
            let stdout_normalized: String = stdout_str.replace(&test_dir_str, "tests/lint");

            expect_file![expected_output_path_stderr].assert_eq(&stderr_normalized);
            expect_file![expected_output_path_stdout].assert_eq(&stdout_normalized);
            expect_file![expected_output_path_exitcode].assert_eq(&exit_code_str);
        }
    }
}
