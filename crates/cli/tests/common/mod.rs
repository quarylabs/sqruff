use std::path::PathBuf;

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir().unwrap().join(path)
    }
}

#[allow(dead_code)]
pub(crate) fn manifest_dir() -> PathBuf {
    if let Some(manifest) = std::env::var_os("SQRUFF_TEST_MANIFEST") {
        return absolute_path(PathBuf::from(manifest))
            .parent()
            .unwrap()
            .to_path_buf();
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub(crate) fn sqruff_path() -> PathBuf {
    std::env::var_os("SQRUFF_BIN")
        .map(PathBuf::from)
        .map(absolute_path)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_sqruff")))
}
