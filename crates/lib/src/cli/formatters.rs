use std::borrow::Cow;
use std::io::Write;

use colored::{Color, Colorize};

use crate::core::config::FluffConfig;
use crate::core::errors::SQLBaseError;
use crate::core::linter::linted_file::LintedFile;

pub trait Formatter {
    fn dispatch_template_header(
        &self,
        f_name: String,
        linter_config: FluffConfig,
        file_config: FluffConfig,
    );

    fn dispatch_parse_header(&self, f_name: String);
}

pub struct OutputStreamFormatter {
    output_stream: Box<dyn Write>,
    plain_output: bool,
    filter_empty: bool,
    verbosity: i32,
    output_line_length: usize,
}

impl OutputStreamFormatter {
    pub fn new(output_stream: Box<dyn Write>) -> Self {
        colored::control::set_override(true);

        Self {
            output_stream,
            plain_output: false,
            filter_empty: true,
            verbosity: 0,
            output_line_length: 80,
        }
    }

    fn should_produce_plain_output(&self, nocolor: bool) -> bool {
        nocolor || todo!()
    }

    fn dispatch(&mut self, s: &str) {
        if !self.filter_empty || !s.trim().is_empty() {
            self.output_stream.write(s.as_bytes()).unwrap();
        }
    }

    fn format_config(&self) -> String {
        unimplemented!()
    }

    fn dispatch_config(&mut self) {
        self.dispatch(&self.format_config())
    }

    fn dispatch_persist_filename(&self) {}

    fn format_path(&self) {}

    fn dispatch_path(&self) {}

    fn dispatch_template_header(&self) {}

    fn dispatch_parse_header(&self) {}

    fn dispatch_lint_header(&self) {}

    fn dispatch_compilation_header(&self) {}

    fn dispatch_processing_header(&self) {}

    fn dispatch_dialect_warning(&self) {}

    fn format_file_violations(&mut self, fname: &str, mut violations: Vec<SQLBaseError>) -> String {
        let mut text_buffer = String::new();

        let fails =
            violations.iter().filter(|violation| !violation.ignore && !violation.warning).count();
        let warns = violations.iter().filter(|violation| violation.warning).count();
        let show = fails + warns > 0;

        if self.verbosity > 0 || show {
            let text = self.format_filename(fname, fails == 0);
            text_buffer.push_str(&text);
            text_buffer.push('\n');
        }

        if show {
            violations.sort_by(|a, b| {
                a.line_no.cmp(&b.line_no).then_with(|| a.line_pos.cmp(&b.line_pos))
            });

            for violation in violations {
                let text = self.format_violation(violation, self.output_line_length);
                text_buffer.push_str(&text);
                text_buffer.push('\n');
            }
        }

        text_buffer
    }

    fn dispatch_file_violations(
        &mut self,
        fname: &str,
        linted_file: LintedFile,
        only_fixable: bool,
        warn_unused_ignores: bool,
    ) {
        if self.verbosity < 0 {
            return;
        }

        let s = self.format_file_violations(
            fname,
            linted_file.get_violations(only_fixable.then_some(true)),
        );
        self.dispatch(&s);
    }

    fn colorize<'a>(&self, s: &'a str, color: Color) -> Cow<'a, str> {
        Self::colorize_helper(self.plain_output, s, color)
    }

    fn colorize_helper(plain_output: bool, s: &str, color: Color) -> Cow<'_, str> {
        if plain_output { s.into() } else { s.color(color).to_string().into() }
    }

    fn cli_table_row(&self) {}

    fn cli_table(&self) {}

    fn format_filename(&self, filename: &str, success: impl IntoStatus) -> String {
        let status = success.into_status();

        let color = match status {
            Status::Pass | Status::Fixed => Color::Green,
            Status::Fail | Status::Error => Color::Red,
        };

        let filename = self.colorize(filename, Color::BrightGreen);
        let status = self.colorize(status.as_str(), color);

        format!("== [{filename}] {status}")
    }

    fn format_violation(&self, violation: SQLBaseError, max_line_length: usize) -> String {
        unimplemented!()
    }
}

pub trait IntoStatus {
    fn into_status(self) -> Status;
}

impl IntoStatus for bool {
    fn into_status(self) -> Status {
        if self { Status::Pass } else { Status::Fail }
    }
}

impl IntoStatus for (Status, bool) {
    fn into_status(self) -> Status {
        let (if_ok, is_ok) = self;
        if is_ok { if_ok } else { Status::Fail }
    }
}

#[derive(Clone, Copy)]
pub enum Status {
    Pass,
    Fixed,
    Fail,
    Error,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fixed => "FIXED",
            Status::Fail => "FAIL",
            Status::Error => "ERROR",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use colored::Color;
    use fancy_regex::Regex;

    use super::OutputStreamFormatter;
    use crate::cli::formatters::Status;
    use crate::helpers::Boxed;

    fn escape_ansi(line: &str) -> String {
        let ansi_escape = Regex::new("\x1B\\[[0-9]+(?:;[0-9]+)?m").unwrap();
        ansi_escape.replace_all(line, "").into_owned()
    }

    #[test]
    fn test__cli__formatters__filename_nocol() {
        let temp = tempdir::TempDir::new(env!("CARGO_PKG_NAME")).unwrap();
        let file = File::create(temp.path().join("out.txt")).unwrap().boxed();

        let formatter = OutputStreamFormatter::new(file);
        let actual = formatter.format_filename("blahblah", true);

        assert_eq!(escape_ansi(&actual), "== [blahblah] PASS");
    }

    #[test]
    #[ignore = "WIP"]
    fn test__cli__formatters__violation() {}

    #[test]
    fn test__cli__helpers__colorize() {
        let temp = tempdir::TempDir::new(env!("CARGO_PKG_NAME")).unwrap();
        let file = File::create(temp.path().join("out.txt")).unwrap().boxed();

        let mut formatter = OutputStreamFormatter::new(file);
        // Force color output for this test.
        formatter.plain_output = false;

        let actual = formatter.colorize("foo", Color::Red);
        assert_eq!(actual, "\u{1b}[31mfoo\u{1b}[0m");
    }

    #[test]
    fn test__cli__helpers__cli_table() {}

    #[test]
    fn test__cli__fix_no_corrupt_file_contents() {}
}
