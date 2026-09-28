use std::fs;

use assert_cmd::Command;

mod common;
use common::sqruff_path;

#[test]
fn large_file_skip_fail_controls_lint_and_fix_exit_codes() {
    let temp = tempfile::tempdir().unwrap();
    let sql = temp.path().join("big.sql");
    fs::write(&sql, "SELECT 1;\n").unwrap();

    for limit in ["byte", "char"] {
        for command in ["lint", "fix"] {
            for (large_file_skip_fail, expected_code) in [(false, 0), (true, 1)] {
                let config = temp
                    .path()
                    .join(format!("{command}-{limit}-{large_file_skip_fail}.cfg"));
                let (byte_limit, char_limit) = if limit == "byte" { (5, 0) } else { (0, 5) };
                fs::write(
                    &config,
                    format!(
                        "[sqruff]\ndialect = ansi\nlarge_file_skip_byte_limit = {byte_limit}\nlarge_file_skip_char_limit = {char_limit}\nlarge_file_skip_fail = {large_file_skip_fail}\n"
                    ),
                )
                .unwrap();

                Command::new(sqruff_path())
                    .current_dir(temp.path())
                    .env("HOME", temp.path())
                    .arg(command)
                    .arg("--config")
                    .arg(&config)
                    .arg(&sql)
                    .assert()
                    .code(expected_code);
            }
        }
    }
}
