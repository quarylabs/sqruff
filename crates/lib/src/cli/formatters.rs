use super::utils::*;
use std::borrow::Cow;
use std::io::{Stderr, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize};

use anstyle::{AnsiColor, Effects, Style};
use itertools::enumerate;
use sqruff_lib_core::errors::SQLBaseError;

use crate::core::config::FluffConfig;
use crate::core::linter::linted_file::LintedFile;

const LIGHT_GREY: Style = AnsiColor::Black.on_default().effects(Effects::BOLD);

pub trait Formatter: Send + Sync {
    fn dispatch_template_header(
        &self,
        f_name: String,
        linter_config: FluffConfig,
        file_config: FluffConfig,
    );

    fn dispatch_parse_header(&self, f_name: String);

    fn dispatch_file_violations(&self, linted_file: &LintedFile, only_fixable: bool);

    fn has_fail(&self) -> bool;

    fn completion_message(&self);
}

pub struct OutputStreamFormatter {
    output_stream: Option<Stderr>,
    plain_output: bool,
    filter_empty: bool,
    verbosity: i32,
    output_line_length: usize,
    pub has_fail: AtomicBool,
    files_dispatched: AtomicUsize,
}

impl Formatter for OutputStreamFormatter {
    fn dispatch_file_violations(&self, linted_file: &LintedFile, only_fixable: bool) {
        if self.verbosity < 0 {
            return;
        }

        let s = self.format_file_violations(
            &linted_file.path,
            linted_file.get_violations(only_fixable.then_some(true)),
        );

        self.dispatch(&s);
        self.files_dispatched
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn has_fail(&self) -> bool {
        self.has_fail.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn completion_message(&self) {
        let count = self
            .files_dispatched
            .load(std::sync::atomic::Ordering::SeqCst);
        let message = format!("The linter processed {count} file(s).\n");
        self.dispatch(&message);

        let message = if self.plain_output {
            "All Finished\n"
        } else {
            "All Finished 📜 🎉\n"
        };
        self.dispatch(message);
    }
    fn dispatch_template_header(
        &self,
        _f_name: String,
        _linter_config: FluffConfig,
        _file_config: FluffConfig,
    ) {
    }

    fn dispatch_parse_header(&self, _f_name: String) {}
}

impl OutputStreamFormatter {
    pub fn new(output_stream: Option<Stderr>, nocolor: bool, verbosity: i32) -> Self {
        Self {
            output_stream,
            plain_output: should_produce_plain_output(nocolor),
            filter_empty: true,
            verbosity,
            output_line_length: 80,
            has_fail: false.into(),
            files_dispatched: 0.into(),
        }
    }

    fn dispatch(&self, s: &str) {
        if !self.filter_empty || !s.trim().is_empty() {
            if let Some(output_stream) = &self.output_stream {
                _ = output_stream.lock().write(s.as_bytes()).unwrap();
            }
        }
    }

    fn format_file_violations(&self, fname: &str, mut violations: Vec<SQLBaseError>) -> String {
        let mut text_buffer = String::new();

        let fails = violations
            .iter()
            .filter(|violation| !violation.ignore && !violation.warning)
            .count();
        let warns = violations
            .iter()
            .filter(|violation| violation.warning)
            .count();
        let show = fails + warns > 0;

        if self.verbosity > 0 || show {
            let text = self.format_filename(fname, fails == 0);
            text_buffer.push_str(&text);
            text_buffer.push('\n');
        }

        if show {
            violations.sort_by(|a, b| {
                a.line_no
                    .cmp(&b.line_no)
                    .then_with(|| a.line_pos.cmp(&b.line_pos))
            });

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
            Status::Pass | Status::Fixed => AnsiColor::Green,
            Status::Fail | Status::Error => {
                self.has_fail
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                AnsiColor::Red
            }
        }
        .on_default();

        let filename = self.colorize(filename, LIGHT_GREY);
        let status = self.colorize(status.as_str(), color);

        format!("== [{filename}] {status}")
    }

    fn format_violation(
        &self,
        violation: impl Into<SQLBaseError>,
        max_line_length: usize,
    ) -> String {
        let violation: SQLBaseError = violation.into();
        let desc = violation.desc();

        let severity = if violation.ignore {
            "IGNORE: "
        } else if violation.warning {
            "WARNING: "
        } else {
            ""
        };

        let line_elem = format!("{:4}", violation.line_no);
        let pos_elem = format!("{:4}", violation.line_pos);

        let mut desc = format!("{severity}{desc}");

        if let Some(rule) = &violation.rule {
            let text = self.colorize(rule.name, LIGHT_GREY);
            let text = format!(" [{text}]");
            desc.push_str(&text);
        }

        let split_desc = split_string_on_spaces(&desc, max_line_length - 25);
        let mut section_color = if violation.ignore || violation.warning {
            LIGHT_GREY
        } else {
            AnsiColor::Blue.on_default()
        };

        let mut out_buff = String::new();
        for (idx, line) in enumerate(split_desc) {
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
    use anstyle::AnsiColor;
    use fancy_regex::Regex;
    use sqruff_lib_core::dialects::syntax::SyntaxKind;
    use sqruff_lib_core::errors::{ErrorStructRule, SQLLintError};
    use sqruff_lib_core::parser::markers::PositionMarker;
    use sqruff_lib_core::parser::segments::base::SegmentBuilder;

    use super::OutputStreamFormatter;
    use crate::cli::formatters::split_string_on_spaces;

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

        let s = SegmentBuilder::token(0, "foobarbar", SyntaxKind::Word)
            .with_position(PositionMarker::new(
                10..19,
                10..19,
                "      \n\n  foobarbar".into(),
                None,
                None,
            ))
            .finish();

        let mut v = SQLLintError::new("DESC", s, false, vec![]);

        v.rule = Some(ErrorStructRule {
            name: "some-name",
            code: "DESC",
        });

        let f = formatter.format_violation(v, 90);

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
