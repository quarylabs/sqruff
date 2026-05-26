use core::str;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn main() {
    ignored_directories_are_not_traversed();
    explicit_ignored_files_still_work();
    missing_paths_return_controlled_errors();
}

fn sqruff_path(cargo_folder: &Path) -> PathBuf {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));
    sqruff_path
}

fn ignored_directories_are_not_traversed() {
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp_dir = tempfile::tempdir().unwrap();
    let project = temp_dir.path();

    fs::write(project.join(".sqruffignore"), "ignored/\n").unwrap();
    fs::write(project.join("regular.sql"), "SELECT 1;\n").unwrap();
    fs::create_dir_all(project.join("ignored").join("nested")).unwrap();
    fs::write(
        project.join("ignored").join("nested").join("hidden.sql"),
        "SELECT FROM\n",
    )
    .unwrap();

    let mut cmd = Command::new(sqruff_path(cargo_folder));
    cmd.arg("lint")
        .arg("-f")
        .arg("json")
        .arg(project)
        .current_dir(project)
        .env("HOME", cargo_folder);
    let output = cmd.assert();
    let stdout = str::from_utf8(&output.get_output().stdout).unwrap();
    let stderr = str::from_utf8(&output.get_output().stderr).unwrap();

    assert!(!stdout.contains("hidden.sql"));
    assert!(!stderr.contains("hidden.sql"));
    assert!(stdout.contains("regular.sql") || stderr.contains("regular.sql"));
}

fn explicit_ignored_files_still_work() {
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp_dir = tempfile::tempdir().unwrap();
    let project = temp_dir.path();
    let ignored = project.join("ignored.sql");

    fs::write(project.join(".sqruffignore"), "ignored.sql\n").unwrap();
    fs::write(&ignored, "SELECT  1;\n").unwrap();

    let mut cmd = Command::new(sqruff_path(cargo_folder));
    cmd.arg("lint")
        .arg("-f")
        .arg("json")
        .arg(&ignored)
        .current_dir(project)
        .env("HOME", cargo_folder);
    let output = cmd.assert();
    let stdout = str::from_utf8(&output.get_output().stdout).unwrap();

    assert_eq!(output.get_output().status.code().unwrap(), 1);
    assert!(stdout.contains("ignored.sql"));
    assert!(stdout.contains("LT01"));
}

fn missing_paths_return_controlled_errors() {
    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp_dir = tempfile::tempdir().unwrap();
    let missing = temp_dir.path().join("missing.sql");

    let mut cmd = Command::new(sqruff_path(cargo_folder));
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg(&missing)
        .current_dir(temp_dir.path())
        .env("HOME", cargo_folder);
    let output = cmd.assert();
    let stderr = str::from_utf8(&output.get_output().stderr).unwrap();

    assert_eq!(output.get_output().status.code().unwrap(), 1);
    assert!(stderr.contains("Specified path does not exist"));
    assert!(!stderr.contains("panicked at"));
}
