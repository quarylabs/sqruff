use std::path::PathBuf;

use assert_cmd::Command;

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir().unwrap().join(path)
    }
}

pub(crate) fn manifest_dir() -> PathBuf {
    if let Some(manifest) = std::env::var_os("SQRUFF_PYTHON_TEST_MANIFEST") {
        return absolute_path(PathBuf::from(manifest))
            .parent()
            .unwrap()
            .to_path_buf();
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sqruff_path() -> PathBuf {
    let path = std::env::var_os("SQRUFF_PYTHON_BIN")
        .map(PathBuf::from)
        .map(absolute_path)
        .unwrap_or_else(|| manifest_dir().join("../../.venv/bin/sqruff"));

    assert!(
        path.is_file(),
        "sqruff Python CLI not found; run `maturin develop` or set SQRUFF_PYTHON_BIN"
    );
    path
}

pub(crate) fn sqruff_command() -> Command {
    let mut command = Command::new(sqruff_path());
    command.current_dir(manifest_dir());
    if let Some(python_path) = std::env::var_os("SQRUFF_PYTHONPATH") {
        command.env("PYTHONPATH", absolute_path(PathBuf::from(python_path)));
    }
    command
}

#[allow(dead_code)]
pub(crate) fn copy_dir(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).unwrap();

    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path);
        } else {
            std::fs::copy(source_path, destination_path).unwrap();
        }
    }
}
