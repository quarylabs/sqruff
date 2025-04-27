use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use expect_test::expect_file;

fn main() {
    let mut lint_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    lint_dir.push("tests/lint");

    // Iterate over each test file in the directory
    for entry in fs::read_dir(&lint_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        // Check if the file has a .sql or .hql extension
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext == "sql" || ext == "hql")
        {
            // Construct the path to the sqruff binary
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
            // Set up the command with arguments
            let mut cmd = Command::new(sqruff_path);
            cmd.arg("lint").arg("-f").arg("human").arg(&path);
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

            let test_dir_str = lint_dir.to_string_lossy().to_string();
            let stderr_normalized: String = stderr_str.replace(&test_dir_str, "tests/lint");
            let stdout_normalized: String = stdout_str.replace(&test_dir_str, "tests/lint");

            expect_file![expected_output_path_stderr].assert_eq(&stderr_normalized);
            expect_file![expected_output_path_stdout].assert_eq(&stdout_normalized);
            expect_file![expected_output_path_exitcode].assert_eq(&exit_code_str);
        }
    }

    // Add a new test case for stdin input
    {
        // Simple SQL input
        let sql_input = "SELECT * FROM users;";

        // Construct the path to the sqruff binary
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        sqruff_path.push(format!("../../target/{}/sqruff", profile));

        // Set up the command with arguments
        let mut cmd = Command::new(sqruff_path);
        cmd.arg("lint").arg("-f").arg("human").arg("-"); // Use '-' to indicate stdin
        cmd.env("HOME", PathBuf::from(env!("CARGO_MANIFEST_DIR")));

        // Provide input via stdin
        cmd.write_stdin(sql_input);

        // Run the command and capture the output
        let assert = cmd.assert();

        // Expected output paths
        let mut lint_dir = lint_dir.clone();
        assert!(lint_dir.pop());
        let lint_dir = lint_dir.join("stdin");

        let expected_output_path_stderr = lint_dir.join("stdin.stderr");
        let expected_output_path_stdout = lint_dir.join("stdin.stdout");
        let expected_output_path_exitcode = lint_dir.join("stdin.exitcode");

        // Read the output
        let output = assert.get_output();
        let stderr_str = std::str::from_utf8(&output.stderr).unwrap();
        let stdout_str = std::str::from_utf8(&output.stdout).unwrap();
        let exit_code_str = output.status.code().unwrap().to_string();

        // Assert outputs
        expect_file![expected_output_path_stderr].assert_eq(stderr_str);
        expect_file![expected_output_path_stdout].assert_eq(stdout_str);
        expect_file![expected_output_path_exitcode].assert_eq(&exit_code_str);
    }
}
