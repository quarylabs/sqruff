use std::path::Path;

use assert_cmd::Command;

fn main() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sqruff_path = crate_dir.join("../../.venv/bin/sqruff");
    assert!(
        sqruff_path.is_file(),
        "sqruff script not found in .venv/bin; run `maturin develop` first"
    );

    let fixture_dir = crate_dir.join("tests/library_path");
    let library_path = fixture_dir.join("custom_library");

    let mut cmd = Command::new(&sqruff_path);
    cmd.current_dir(crate_dir)
        .env("HOME", crate_dir)
        .arg("lint")
        .arg("--format")
        .arg("none")
        .arg("--config")
        .arg(fixture_dir.join(".sqruff"))
        .arg("--library-path")
        .arg(library_path)
        .arg(fixture_dir.join("query.sql"));

    cmd.assert().success();

    let mut disabled_cmd = Command::new(sqruff_path);
    disabled_cmd
        .current_dir(crate_dir)
        .env("HOME", crate_dir)
        .arg("lint")
        .arg("--format")
        .arg("none")
        .arg("--config")
        .arg(fixture_dir.join(".sqruff"))
        .arg("--library-path")
        .arg("none")
        .arg(fixture_dir.join("query.sql"));

    disabled_cmd.assert().failure();
}
