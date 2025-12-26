pub(crate) mod github_annotation_native_formatter;
pub(crate) mod json;
pub(crate) mod json_types;
pub(crate) mod rules;
pub(crate) mod utils;

use std::borrow::Cow;
use std::io::{Stderr, Write};

use anstyle::{AnsiColor, Effects, Style};
use sqruff_lib::Formatter;
use sqruff_lib::core::linter::linted_file::LintedFile;
use sqruff_lib_core::errors::SQLBaseError;

use crate::formatters::utils::{
    colorize_helper, should_produce_plain_output, split_string_on_spaces,
};

const LIGHT_GREY: Style = AnsiColor::Black.on_default().effects(Effects::BOLD);

pub(crate) struct OutputStreamFormatter {
    output_stream: Option<Stderr>,
    plain_output: bool,
    filter_empty: bool,
    verbosity: i32,
    output_line_length: usize,
}

impl Formatter for OutputStreamFormatter {
    fn dispatch_file_violations(&self, linted_file: &LintedFile) {
        if self.verbosity < 0 {
            return;
        }

        let s = self.format_file_violations(linted_file.path(), linted_file.violations());

        self.dispatch(&s);
    }

    fn completion_message(&self, count: usize) {
        self.dispatch(&format!("The linter processed {count} file(s).\n"));
        self.dispatch(if self.plain_output {
            "All Finished\n"
        } else {
            "All Finished 📜 🎉\n"
        });
    }
}

impl OutputStreamFormatter {
    pub(crate) fn new(output_stream: Option<Stderr>, nocolor: bool, verbosity: i32) -> Self {
        Self {
            output_stream,
            plain_output: should_produce_plain_output(nocolor),
            filter_empty: true,
            verbosity,
            output_line_length: 80,
        }
    }

    fn dispatch(&self, s: &str) {
        if (!self.filter_empty || !s.trim().is_empty())
            && let Some(output_stream) = &self.output_stream
        {
            let mut output_stream = output_stream.lock();
            output_stream
                .write_all(s.as_bytes())
                .and_then(|_| output_stream.flush())
                .unwrap_or_else(|e| panic!("failed to emit error: {e}"));
        }
    }

    fn format_file_violations(&self, fname: &str, violations: &[SQLBaseError]) -> String {
        let mut text_buffer = String::new();

        let show = !violations.is_empty();

        if self.verbosity > 0 || show {
            let text = self.format_filename(fname, !show);
            text_buffer.push_str(&text);
            text_buffer.push('\n');
        }

        if show {
            for violation in violations {
                let text = self.format_violation(violation, self.output_line_length);
                text_buffer.push_str(&text);
                text_buffer.push('\n');
            }
        }

        text_buffer
    }

    fn colorize<'a>(&self, s: &'a str, style: Style) -> Cow<'a, str> {
        colorize_helper(self.plain_output, s, style)
    }

    fn format_filename(&self, filename: &str, success: bool) -> String {
        let status = if success { Status::Pass } else { Status::Fail };

        let color = match status {
            Status::Pass => AnsiColor::Green,
            Status::Fail => AnsiColor::Red,
        }
        .on_default();

        let filename = self.colorize(filename, LIGHT_GREY);
        let status = self.colorize(status.as_str(), color);

        format!("== [{filename}] {status}")
    }

    fn format_violation(&self, violation: &SQLBaseError, max_line_length: usize) -> String {
        let mut desc = violation.desc().to_string();

        let line_elem = format!("{:4}", violation.line_no);
        let pos_elem = format!("{:4}", violation.line_pos);

        if let Some(rule) = &violation.rule {
            let text = self.colorize(rule.name, LIGHT_GREY);
            let text = format!(" [{text}]");
            desc.push_str(&text);
        }

        let split_desc = split_string_on_spaces(&desc, max_line_length - 25);
        let mut section_color = AnsiColor::Blue.on_default();

        let mut out_buff = String::new();
        for (idx, line) in split_desc.into_iter().enumerate() {
            if idx == 0 {
                let rule_code = format!("{:>4}", violation.rule_code());

                if rule_code.contains("PRS") {
                    section_color = AnsiColor::Red.on_default();
                }

                let section = format!("L:{line_elem} | P:{pos_elem} | {rule_code} | ");
                let section = self.colorize(&section, section_color);
                out_buff.push_str(&section);
            } else {
                out_buff.push_str(&format!(
                    "\n{}{}",
                    " ".repeat(23),
                    self.colorize("| ", section_color),
                ));
            }
            out_buff.push_str(line);
        }

        out_buff
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Status {
    Pass,

    Fail,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
        }
    }
}

#[cfg(test)]
mod tests {
    use anstyle::AnsiColor;
    use fancy_regex::Regex;
    use sqruff_lib_core::errors::{ErrorStructRule, SQLBaseError};

    use crate::formatters::{OutputStreamFormatter, utils::split_string_on_spaces};

    #[test]
    fn test_short_string() {
        assert_eq!(split_string_on_spaces("abc", 100), vec!["abc"]);
    }

    #[test]
    fn test_split_with_line_length() {
        assert_eq!(
            split_string_on_spaces("abc def ghi", 7),
            vec!["abc def", "ghi"]
        );
    }

    #[test]
    fn test_preserve_multi_space() {
        assert_eq!(
            split_string_on_spaces("a '   ' b c d e f", 11),
            vec!["a '   ' b c", "d e f"]
        );
    }

    fn escape_ansi(line: &str) -> String {
        let ansi_escape = Regex::new("\x1B\\[[0-9]+(?:;[0-9]+)?m").unwrap();
        ansi_escape.replace_all(line, "").into_owned()
    }

    fn mk_formatter() -> OutputStreamFormatter {
        OutputStreamFormatter::new(None, false, 0)
    }

    #[test]
    fn test_cli_formatters_filename_nocol() {
        let formatter = mk_formatter();
        let actual = formatter.format_filename("blahblah", true);

        assert_eq!(escape_ansi(&actual), "== [blahblah] PASS");
    }

    #[test]
    fn test_cli_formatters_violation() {
        let formatter = mk_formatter();

        let v = SQLBaseError {
            fixable: false,
            line_no: 3,
            line_pos: 3,
            description: "DESC".into(),
            rule: Some(ErrorStructRule {
                name: "some-name",
                code: "DESC",
            }),
            source_slice: 0..0,
        };

        let f = formatter.format_violation(&v, 90);

        assert_eq!(escape_ansi(&f), "L:   3 | P:   3 | DESC | DESC [some-name]");
    }

    #[test]
    fn test_cli_helpers_colorize() {
        let mut formatter = mk_formatter();
        // Force color output for this test.
        formatter.plain_output = false;

        let actual = formatter.colorize("foo", AnsiColor::Red.on_default());
        assert_eq!(actual, "\u{1b}[31mfoo\u{1b}[0m");
    }
}
