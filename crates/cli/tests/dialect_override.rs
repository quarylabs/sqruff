use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

/// Tests that the dialect override works.
///
/// It tests the same file that is invalid ANSI but valid Postgres in the cases:
/// 1. with no config file to ensure it defaults to ANSI and fails
/// 2. with no config file but with a dialect override to ensure it succeeds
/// 3. with a config file set to ANSI to ensure it fails
/// 4. with a config file set to ANSI but with a dialect override to ensure it succeeds
fn main() {
    dialect_override();
}

fn dialect_override() {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };

    let cargo_folder = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Construct the path to the sqruff binary
    let mut sqruff_path = PathBuf::from(cargo_folder);
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // Temporary directory and SQL file used in all test cases
    let tmp_dir = tempfile::tempdir().unwrap();
    let sql_path = tmp_dir.path().join("example.sql");
    fs::write(&sql_path, STATEMENT).unwrap();

    // 1. No config file - defaults to ANSI and fails
    let mut cmd = Command::new(&sqruff_path);
    cmd.arg("lint").arg("-f").arg("human").arg(&sql_path);
    cmd.env("HOME", cargo_folder);
    cmd.current_dir(tmp_dir.path());
    let output = cmd.assert();
    assert_eq!(output.get_output().status.code().unwrap(), 1);

    // 2. No config but override dialect to Postgres - succeeds
    let mut cmd = Command::new(&sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--dialect")
        .arg("postgres")
        .arg(&sql_path);
    cmd.env("HOME", cargo_folder);
    cmd.current_dir(tmp_dir.path());
    let output = cmd.assert();
    assert_eq!(output.get_output().status.code().unwrap(), 0);

    // Prepare config file set to ANSI
    let cfg_path = tmp_dir.path().join("sqruff.cfg");
    fs::write(&cfg_path, "[sqruff]\ndialect = ansi\n").unwrap();

    // 3. Config file set to ANSI - fails
    let mut cmd = Command::new(&sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(&cfg_path)
        .arg(&sql_path);
    cmd.env("HOME", cargo_folder);
    cmd.current_dir(tmp_dir.path());
    let output = cmd.assert();
    assert_eq!(output.get_output().status.code().unwrap(), 1);

    // 4. Config file set to ANSI with dialect override - succeeds
    let mut cmd = Command::new(&sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("human")
        .arg("--config")
        .arg(&cfg_path)
        .arg("--dialect")
        .arg("postgres")
        .arg(&sql_path);
    cmd.env("HOME", cargo_folder);
    cmd.current_dir(tmp_dir.path());
    let output = cmd.assert();
    assert_eq!(output.get_output().status.code().unwrap(), 0);
}

const STATEMENT: &str = "SELECT DISTINCT ON (customer_id)\n    customer_id, total, created_at\nFROM orders\nORDER BY customer_id, created_at DESC;\n";

