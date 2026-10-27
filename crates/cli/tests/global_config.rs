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

fn lsp_format(mut command: Command, workspace: &std::path::Path) -> String {
    let workspace = workspace.canonicalize().unwrap();
    let path = workspace.to_string_lossy().replace('\\', "/");
    let root_uri = if cfg!(windows) {
        format!("file:///{path}")
    } else {
        format!("file://{path}")
    };
    let uri = format!("{root_uri}/query.sql");
    let messages = [
        serde_json::json!({"id": 1, "method": "initialize", "params": {
            "processId": null, "rootUri": root_uri, "capabilities": {}
        }}),
        serde_json::json!({"method": "initialized", "params": {}}),
        serde_json::json!({"method": "textDocument/didOpen", "params": {"textDocument": {
            "uri": uri, "languageId": "sql", "version": 1, "text": "select  1;\n"
        }}}),
        serde_json::json!({"id": 2, "method": "textDocument/formatting", "params": {
            "textDocument": {"uri": uri}, "options": {"tabSize": 4, "insertSpaces": true}
        }}),
        serde_json::json!({"id": 3, "method": "shutdown", "params": null}),
        serde_json::json!({"method": "exit", "params": null}),
    ];
    let mut input = String::new();
    for mut message in messages {
        message["jsonrpc"] = "2.0".into();
        let body = serde_json::to_string(&message).unwrap();
        input.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    }
    let assert = command
        .arg("lsp")
        .timeout(std::time::Duration::from_secs(10))
        .write_stdin(input)
        .assert()
        .success();
    let mut output = assert.get_output().stdout.as_slice();
    while !output.is_empty() {
        let header_end = output
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .unwrap();
        let header = std::str::from_utf8(&output[..header_end]).unwrap();
        let length: usize = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        let body_start = header_end + 4;
        let response: serde_json::Value =
            serde_json::from_slice(&output[body_start..body_start + length]).unwrap();
        if response["id"] == 2 {
            return response["result"][0]["newText"]
                .as_str()
                .unwrap()
                .to_owned();
        }
        output = &output[body_start + length..];
    }
    panic!("LSP did not return a formatting response");
}

#[test]
fn lsp_loads_global_defaults_and_workspace_overrides() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let (command, app) = app_config_command(temp.path());
    fs::write(
        app.join(".sqruff"),
        "[sqruff]\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    )
    .unwrap();
    assert_eq!(lsp_format(command, &workspace), "SELECT  1;\n");

    fs::write(
        workspace.join(".sqruff"),
        "[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = lower\n",
    )
    .unwrap();
    let (command, _) = app_config_command(temp.path());
    assert_eq!(lsp_format(command, &workspace), "select  1;\n");
}

#[test]
fn lsp_ignores_discovered_config_but_loads_explicit_config() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let (mut command, app) = app_config_command(temp.path());
    for dir in [&app, &workspace] {
        fs::write(dir.join("sqruff.toml"), "invalid toml [").unwrap();
    }
    let extra = temp.path().join("extra.cfg");
    fs::write(
        &extra,
        "[sqruff]\nrules = CP01\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n",
    )
    .unwrap();
    command
        .arg("--ignore-local-config")
        .arg("--config")
        .arg(&extra);
    assert_eq!(lsp_format(command, &workspace), "SELECT  1;\n");
}

#[test]
fn ignore_local_config_skips_config_ignore_paths_for_lint_and_fix() {
    for (filename, contents) in [
        (".sqlfluff", "[sqlfluff]\nignore_paths = *.sql\n"),
        (
            "pyproject.toml",
            "[tool.sqlfluff.core]\nignore_paths = '*.sql'\n",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join(filename), contents).unwrap();
        fs::write(temp.path().join("query.sql"), "select  1;\n").unwrap();
        let extra = temp.path().join("extra.cfg");
        fs::write(&extra, "[sqruff]\nrules = LT01\n").unwrap();
        for args in [
            vec!["query.sql"],
            vec!["."],
            vec!["--stdin-filename", "virtual.sql", "-"],
        ] {
            let (mut command, _) = app_config_command(temp.path());
            let assert = command
                .args(["lint", "--ignore-local-config", "--config"])
                .arg(&extra)
                .args(["-f", "json"])
                .args(args)
                .write_stdin("select  1;\n")
                .assert()
                .code(1);
            let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
            assert!(stdout.contains("LT01"), "{filename}: {stdout}");
        }

        let (mut command, _) = app_config_command(temp.path());
        command
            .args(["fix", "--ignore-local-config", "--config"])
            .arg(&extra)
            .arg("query.sql")
            .assert()
            .success();
        assert_eq!(
            fs::read_to_string(temp.path().join("query.sql")).unwrap(),
            "select 1;\n"
        );

        let (mut command, _) = app_config_command(temp.path());
        command
            .args(["fix", "--ignore-local-config", "--config"])
            .arg(&extra)
            .args(["--stdin-filename", "virtual.sql", "-"])
            .write_stdin("select  1;\n")
            .assert()
            .success()
            .stdout("select 1;\n");
    }
}

#[test]
fn ignore_local_config_keeps_dedicated_ignore_files() {
    for filename in [".sqruffignore", ".sqlfluffignore"] {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join(filename), "*.sql\n").unwrap();
        fs::write(temp.path().join("query.sql"), "select  1;\n").unwrap();
        let (mut command, _) = app_config_command(temp.path());
        command
            .args(["lint", "--ignore-local-config", "-f", "json", "query.sql"])
            .assert()
            .success()
            .stdout("{}\n");
    }
}
