use assert_cmd::Command;

mod common;
use common::{manifest_dir, sqruff_path};

#[test]
fn explicit_color_overrides_redirected_output_and_no_color() {
    for (flag, no_color_env, expect_color) in [
        (None, None, false),
        (None, Some("1"), false),
        (Some("--nocolor"), None, false),
        (Some("--color"), None, true),
        (Some("--color"), Some("1"), true),
        (Some("--nocolor"), Some("1"), false),
    ] {
        let mut command = Command::new(sqruff_path());
        command.current_dir(manifest_dir());
        command.env("HOME", manifest_dir());
        if let Some(value) = no_color_env {
            command.env("NO_COLOR", value);
        } else {
            command.env_remove("NO_COLOR");
        }
        if let Some(flag) = flag {
            command.arg(flag);
        }
        command.arg("rules");

        let output = command.assert().success().get_output().stdout.clone();
        assert_eq!(
            output.windows(2).any(|bytes| bytes == b"\x1b["),
            expect_color,
            "flag={flag:?}, NO_COLOR={no_color_env:?}"
        );
    }
}

#[test]
fn forced_color_on_redirected_lint_output() {
    for (flag, expect_color) in [("--color", true), ("--nocolor", false)] {
        let mut command = Command::new(sqruff_path());
        command.current_dir(manifest_dir());
        command.env("HOME", manifest_dir());
        command.env("NO_COLOR", "1");
        command.args([flag, "lint", "--dialect", "ansi", "-"]);
        command.write_stdin("SELECT foo bar FROM tabs");

        let output = command.assert().failure().get_output().stderr.clone();
        assert_eq!(
            output.windows(2).any(|bytes| bytes == b"\x1b["),
            expect_color,
            "flag={flag}"
        );
    }
}
