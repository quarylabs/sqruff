mod common;
use common::{manifest_dir, sqruff_command};

#[test]
fn library_path() {
    let crate_dir = manifest_dir();
    let fixture_dir = crate_dir.join("tests/library_path");
    let library_path = fixture_dir.join("custom_library");

    let mut cmd = sqruff_command();
    cmd.current_dir(&crate_dir)
        .env("HOME", &crate_dir)
        .arg("lint")
        .arg("--format")
        .arg("none")
        .arg("--config")
        .arg(fixture_dir.join(".sqruff"))
        .arg("--library-path")
        .arg(library_path)
        .arg(fixture_dir.join("query.sql"));

    cmd.assert().success();

    let mut configured_filter_cmd = sqruff_command();
    configured_filter_cmd
        .current_dir(&crate_dir)
        .env("HOME", &crate_dir)
        .arg("lint")
        .arg("--format")
        .arg("none")
        .arg("--config")
        .arg(fixture_dir.join(".sqruff"))
        .arg(fixture_dir.join("filter_query.sql"));

    configured_filter_cmd.assert().success();

    let mut disabled_cmd = sqruff_command();
    disabled_cmd
        .current_dir(&crate_dir)
        .env("HOME", &crate_dir)
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
