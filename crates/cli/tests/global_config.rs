use std::fs;

use assert_cmd::Command;

mod common;
use common::sqruff_path;

fn app_config_command(root: &std::path::Path) -> (Command, std::path::PathBuf) {
    let mut command = Command::new(sqruff_path());
    // Keep user home configuration out of these application-directory tests.
    command.env_remove("HOME").env_remove("USERPROFILE");
    let base = root.join("config");
    let app = if cfg!(windows) {
        command.env("LOCALAPPDATA", &base);
        base.join("sqruff/sqruff")
    } else {
        command.env("XDG_CONFIG_HOME", &base);
        base.join("sqruff")
    };
    fs::create_dir_all(&app).unwrap();
    command.current_dir(root);
    (command, app)
}

#[test]
fn application_config_is_loaded_outside_home() {
    let temp = tempfile::tempdir().unwrap();
    let (mut command, app) = app_config_command(temp.path());
    fs::write(
        app.join(".sqruff"),
        "[sqruff]\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    )
    .unwrap();

    command
        .args(["fix", "-"])
        .write_stdin("select 1;\n")
        .assert()
        .success()
        .stdout("SELECT 1;\n");

    let sql = temp.path().join("query.sql");
    fs::write(&sql, "select 1;\n").unwrap();
    let (mut command, _) = app_config_command(temp.path());
    command.arg("fix").arg(&sql).assert().success();
    assert_eq!(fs::read_to_string(sql).unwrap(), "SELECT 1;\n");
}

#[test]
fn project_and_explicit_config_override_global_values() {
    let temp = tempfile::tempdir().unwrap();
    let (_, app) = app_config_command(temp.path());
    fs::write(
        app.join(".sqruff"),
        "[sqruff]\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = lower\n",
    )
    .unwrap();
    fs::write(
        temp.path().join(".sqruff"),
        "[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    )
    .unwrap();
    let extra = temp.path().join("extra.cfg");
    fs::write(
        &extra,
        "[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = lower\n",
    )
    .unwrap();

    let (mut command, _) = app_config_command(temp.path());
    command
        .args(["fix", "-"])
        .write_stdin("select 1;\n")
        .assert()
        .success()
        .stdout("SELECT 1;\n");

    let (mut command, _) = app_config_command(temp.path());
    command
        .args(["fix", "--config"])
        .arg(&extra)
        .arg("-")
        .write_stdin("SELECT  1;\n")
        .assert()
        .success()
        .stdout("select  1;\n");
}

#[test]
fn ignore_local_config_still_loads_explicit_config_for_stdin() {
    let temp = tempfile::tempdir().unwrap();
    let (_, app) = app_config_command(temp.path());
    // Invalid discovered files must not be read when discovery is disabled.
    for dir in [&app, temp.path()] {
        fs::write(dir.join("sqruff.toml"), "invalid toml [").unwrap();
    }
    let extra = temp.path().join("extra.cfg");
    fs::write(
        &extra,
        "[sqruff]\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    )
    .unwrap();
    for stdin_args in [vec!["-"], vec!["--stdin-filename", "nested/query.sql", "-"]] {
        let (mut command, _) = app_config_command(temp.path());
        command
            .args(["fix", "--ignore-local-config", "--config"])
            .arg(&extra)
            .args(stdin_args)
            .write_stdin("select 1;\n")
            .assert()
            .success()
            .stdout("SELECT 1;\n");
    }
}
