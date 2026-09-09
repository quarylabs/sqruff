use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) fn manifest_dir() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        std::env::var_os("SQRUFF_TEST_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
    })
}

pub(crate) fn sqruff_path() -> PathBuf {
    let path = std::env::var_os("SQRUFF_PYTHON_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir().join("../../.venv/bin/sqruff"));
    assert!(
        path.is_file(),
        "Python sqruff launcher missing; run maturin develop for Cargo tests"
    );
    path
}
