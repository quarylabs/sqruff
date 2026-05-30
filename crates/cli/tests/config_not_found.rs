use std::fs;
use std::path::{Path, PathBuf};

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

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut lint_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    lint_dir.push("tests/config_not_found");

    let entry_path = lint_dir.as_path().join("example.sql");
    let config_path = lint_dir.as_path().join("non_existant.cfg");

    let target_dir = cargo_folder.join("../../target").join(profile);
    let mut sqruff_source_path = target_dir.join("sqruff");
    if cfg!(windows) {
        sqruff_source_path.set_extension("exe");
    }

    let temp_dir = tempfile::tempdir().unwrap();
    let sqruff_path = temp_dir
        .path()
        .join(sqruff_source_path.file_name().unwrap());
    fs::copy(sqruff_source_path, &sqruff_path).unwrap();

    let mut cmd = Command::new(sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(config_path)
        .arg(entry_path)
        .current_dir(cargo_folder)
        .env("HOME", cargo_folder);

    if cfg!(windows) {
        let mut paths = vec![target_dir.join("deps"), target_dir.clone()];
        if let Some(path) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&path));
        }
        cmd.env("PATH", std::env::join_paths(paths).unwrap());
    }

    let assert = cmd.assert();
    let output = assert.get_output();

    let storage_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("config_not_found")
        .join("example.sql");
    let mut expected_output_path_stderr = storage_path.clone();
    expected_output_path_stderr.set_extension("stderr");
    let mut expected_output_path_stdout = storage_path.clone();
    expected_output_path_stdout.set_extension("stdout");
    let mut expected_output_exitcode = storage_path.clone();
    expected_output_exitcode.set_extension("exitcode");

    let test_dir_str = lint_dir.to_string_lossy().to_string();
    let stderr_str = std::str::from_utf8(&output.stderr).unwrap();
    let stdout_str = std::str::from_utf8(&output.stdout).unwrap();
    let stderr_normalized = stderr_str
        .replace(&test_dir_str, "tests/config_not_found")
        .replace('\\', "/");
    let stdout_normalized = stdout_str
        .replace(&test_dir_str, "tests/config_not_found")
        .replace('\\', "/");
    let exit_code_str = output.status.code().unwrap().to_string();

    expect_file![expected_output_path_stderr].assert_eq(&stderr_normalized);
    expect_file![expected_output_path_stdout].assert_eq(&stdout_normalized);
    expect_file![expected_output_exitcode].assert_eq(&exit_code_str);
}
