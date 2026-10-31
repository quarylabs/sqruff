use std::fs;

use assert_cmd::Command;

mod common;
use common::sqruff_path;

fn rules_output(width: i64) -> String {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".sqruff"),
        format!("[sqruff]\noutput_line_length = {width}\n"),
    )
    .unwrap();
    let assertion = Command::new(sqruff_path())
        .current_dir(temp.path())
        .env("HOME", temp.path())
        .env("USERPROFILE", temp.path())
        .env("XDG_CONFIG_HOME", temp.path())
        .env("LOCALAPPDATA", temp.path())
        .args(["rules", "--nocolor"])
        .assert()
        .success();
    String::from_utf8(assertion.get_output().stdout.clone()).unwrap()
}

#[test]
fn rules_listing_respects_configured_output_line_length() {
    let description = concat!(
        "CV01: [convention.not_equal] Consistent usage of ``!=`` or ``<>`` ",
        "for \"not equal to\" operator."
    );
    for width in [60, 120, 999] {
        let output = rules_output(width);
        let lines = output.lines().map(str::trim_end).collect::<Vec<_>>();
        assert!(
            lines
                .iter()
                .all(|line| line.chars().count() <= width as usize),
            "rules listing exceeds configured width {width}: {output}"
        );
        assert_eq!(
            lines.contains(&description),
            width as usize >= description.chars().count(),
            "unexpected description wrapping for width {width}"
        );
    }
}

#[test]
fn rules_listing_handles_widths_smaller_than_rule_labels() {
    for width in [-1, 0, 1, 6, 7] {
        let output = rules_output(width);
        assert!(output.contains("CV01:"), "width {width}: {output}");
    }
}
