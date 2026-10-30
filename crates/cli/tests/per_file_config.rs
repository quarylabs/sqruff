use std::fs;
use std::path::Path;

use assert_cmd::Command;

mod common;
use common::sqruff_path;

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn fixes_mixed_dialects_from_monorepo_root_without_dialect_flag() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(
        root,
        ".sqruff",
        "[sqruff]\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    );
    write(
        root,
        "bq/sqruff.toml",
        "[tool.sqlfluff.core]\ndialect = 'bigquery'\n",
    );
    write(root, "pg/.sqlfluff", "[sqlfluff]\ndialect = postgres\n");
    write(
        root,
        "bq/query.sql",
        "select * from `project.dataset.table`;\n",
    );
    write(
        root,
        "pg/query.sql",
        "select distinct on (id) id from items;\n",
    );

    Command::new(sqruff_path())
        .current_dir(root)
        .args(["fix", "."])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(root.join("bq/query.sql")).unwrap(),
        "SELECT * FROM `project.dataset.table`;\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("pg/query.sql")).unwrap(),
        "SELECT DISTINCT ON (id) id FROM items;\n"
    );
    Command::new(sqruff_path())
        .current_dir(root)
        .args(["lint", "--parsing-errors", "."])
        .assert()
        .success();
}

#[test]
fn parent_config_is_found_from_nested_working_directory() {
    let temp = tempfile::tempdir().unwrap();
    write(
        temp.path(),
        ".sqruff",
        "[sqruff]\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    );
    write(temp.path(), "child/grandchild/query.sql", "select 1;\n");
    Command::new(sqruff_path())
        .current_dir(temp.path().join("child/grandchild"))
        .args(["fix", "."])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(temp.path().join("child/grandchild/query.sql")).unwrap(),
        "SELECT 1;\n"
    );
}

#[test]
fn explicit_config_overrides_but_does_not_disable_nested_settings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(
        root,
        "child/.sqruff.ini",
        "[sqruff]\ndialect = bigquery\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = lower\n",
    );
    write(
        root,
        "extra.cfg",
        "[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    );
    write(
        root,
        "child/query.sql",
        "select * from `project.dataset.table`;\n",
    );
    Command::new(sqruff_path())
        .current_dir(root)
        .args(["fix", "--config", "extra.cfg", "."])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(root.join("child/query.sql")).unwrap(),
        "SELECT * FROM `project.dataset.table`;\n"
    );
}

#[test]
fn cli_dialect_wins_even_when_it_matches_root_config() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(
        root,
        ".sqruff",
        "[sqruff]\ndialect = bigquery\nrules = LT12\n",
    );
    write(root, "child/.sqruff", "[sqruff]\ndialect = postgres\n");
    write(
        root,
        "child/query.sql",
        "SELECT * FROM `project.dataset.table`\n",
    );
    Command::new(sqruff_path())
        .current_dir(root)
        .args(["lint", "--parsing-errors", "--dialect", "bigquery", "."])
        .assert()
        .success();
}

#[test]
fn ignore_local_config_skips_invalid_nested_config() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(root, "child/sqruff.toml", "invalid [");
    write(root, "child/query.sql", "select 1;\n");
    write(
        root,
        "extra.cfg",
        "[sqruff]\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    );
    Command::new(sqruff_path())
        .current_dir(root)
        .args(["fix", "--ignore-local-config", "--config", "extra.cfg", "."])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(root.join("child/query.sql")).unwrap(),
        "SELECT 1;\n"
    );
}

#[test]
fn nested_config_errors_are_reported() {
    let temp = tempfile::tempdir().unwrap();
    write(temp.path(), "child/sqruff.toml", "invalid [");
    write(temp.path(), "child/query.sql", "SELECT 1;\n");
    let output = Command::new(sqruff_path())
        .current_dir(temp.path())
        .args(["lint", "."])
        .assert()
        .failure();
    assert!(String::from_utf8_lossy(&output.get_output().stderr).contains("sqruff.toml"));
}

#[test]
fn parse_uses_file_configuration() {
    let temp = tempfile::tempdir().unwrap();
    write(
        temp.path(),
        "child/.sqruff",
        "[sqruff]\ndialect = bigquery\n",
    );
    write(
        temp.path(),
        "child/query.sql",
        "SELECT * FROM `project.dataset.table`\n",
    );
    Command::new(sqruff_path())
        .current_dir(temp.path())
        .args(["parse", "child/query.sql"])
        .assert()
        .success();
}

#[test]
fn mixed_dialects_do_not_require_a_root_config() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(
        root,
        "bq/.sqruff",
        "[sqruff]\ndialect = bigquery\nrules = LT12\n",
    );
    write(
        root,
        "pg/.sqruff",
        "[sqruff]\ndialect = postgres\nrules = LT12\n",
    );
    write(
        root,
        "bq/query.sql",
        "SELECT * FROM `project.dataset.table`\n",
    );
    write(
        root,
        "pg/query.sql",
        "SELECT DISTINCT ON (id) id FROM items\n",
    );
    Command::new(sqruff_path())
        .current_dir(root)
        .args(["lint", "--parsing-errors", "."])
        .assert()
        .success();
}
