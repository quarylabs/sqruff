mod common;

use std::path::Path;

use assert_cmd::Command;

fn main() {
    let crate_dir = Path::new(common::manifest_dir());
    let sqruff_path = common::sqruff_path();

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

    let mut configured_filter_cmd = Command::new(&sqruff_path);
    configured_filter_cmd
        .current_dir(crate_dir)
        .env("HOME", crate_dir)
        .arg("lint")
        .arg("--format")
        .arg("none")
        .arg("--config")
        .arg(fixture_dir.join(".sqruff"))
        .arg(fixture_dir.join("filter_query.sql"));

    configured_filter_cmd.assert().success();

    let mut disabled_cmd = Command::new(&sqruff_path);
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
