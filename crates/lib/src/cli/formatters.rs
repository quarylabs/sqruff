use std::borrow::Cow;
use std::io::{IsTerminal, Write};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use anstyle::{AnsiColor, Effects, Style};
use itertools::enumerate;
use sqruff_lib_core::errors::SQLBaseError;

use crate::core::config::FluffConfig;
use crate::core::linter::linted_file::LintedFile;

const LIGHT_GREY: Style = AnsiColor::Black.on_default().effects(Effects::BOLD);

fn split_string_on_spaces(s: &str, line_length: usize) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut line_start = 0;
    let mut last_space = 0;

    for (idx, char) in s.char_indices() {
        if char.is_whitespace() {
            last_space = idx;
        }

        if idx - line_start >= line_length {
            if last_space == line_start {
                lines.push(&s[line_start..idx]);
                line_start = idx + 1;
            } else {
                lines.push(&s[line_start..last_space]);
                line_start = last_space + 1;
            }
            last_space = line_start;
        }
    }

    if line_start < s.len() {
        lines.push(&s[line_start..]);
    }

    lines
}

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
    output_stream: Mutex<Box<dyn Write + Send + Sync>>,
    plain_output: bool,
    filter_empty: bool,
    verbosity: i32,
    output_line_length: usize,
    pub has_fail: AtomicBool,
}

impl OutputStreamFormatter {
    pub fn new(output_stream: Box<dyn Write + Send + Sync>, nocolor: bool) -> Self {
        Self {
            output_stream: output_stream.into(),
            plain_output: Self::should_produce_plain_output(nocolor),
            filter_empty: true,
            verbosity: 0,
            output_line_length: 80,
            has_fail: false.into(),
        }
    }

    fn should_produce_plain_output(nocolor: bool) -> bool {
        nocolor || !std::io::stdout().is_terminal()
    }

    fn dispatch(&self, s: &str) {
        if !self.filter_empty || !s.trim().is_empty() {
            _ = self
                .output_stream
                .lock()
                .unwrap()
                .write(s.as_bytes())
                .unwrap();
        }
    }

    #[allow(dead_code)]
    fn format_config(&self) -> String {
        unimplemented!()
    }

    #[allow(dead_code)]
    fn dispatch_config(&mut self) {
        self.dispatch(&self.format_config())
    }

    #[allow(dead_code)]
    fn dispatch_persist_filename(&self) {}

    #[allow(dead_code)]
    fn format_path(&self) {}

    #[allow(dead_code)]
    fn dispatch_path(&self) {}

    pub fn dispatch_template_header(&self) {}

    pub fn dispatch_parse_header(&self) {}

    #[allow(dead_code)]
    fn dispatch_lint_header(&self) {}

    #[allow(dead_code)]
    fn dispatch_compilation_header(&self) {}

    #[allow(dead_code)]
    fn dispatch_processing_header(&self) {}

    #[allow(dead_code)]
    fn dispatch_dialect_warning(&self) {}

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
    pub fn dispatch_file_violations(&self, linted_file: &LintedFile, only_fixable: bool) {
        if self.verbosity < 0 {
            return;
        }

        let s = self.format_file_violations(
            &linted_file.path,
            linted_file.get_violations(only_fixable.then_some(true)),
        );

        self.dispatch(&s);
    }

    fn colorize<'a>(&self, s: &'a str, style: Style) -> Cow<'a, str> {
        Self::colorize_helper(self.plain_output, s, style)
    }

    fn colorize_helper(plain_output: bool, s: &str, style: Style) -> Cow<'_, str> {
        if plain_output {
            s.into()
        } else {
            format!("{style}{s}{style:#}").into()
        }
    }

    #[allow(dead_code)]
    fn cli_table_row(&self) {}

    #[allow(dead_code)]
    fn cli_table(&self) {}

    fn format_filename(&self, filename: &str, success: impl IntoStatus) -> String {
        let status = success.into_status();

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

    pub fn completion_message(&mut self) {
        let message = if self.plain_output {
            "All Finished\n"
        } else {
            "All Finished 📜 🎉\n"
        };
        self.dispatch(message);
    }
}

pub trait IntoStatus {
    fn into_status(self) -> Status;
}

impl IntoStatus for bool {
    fn into_status(self) -> Status {
        if self {
            Status::Pass
        } else {
            Status::Fail
        }
    }
}

impl IntoStatus for (Status, bool) {
    fn into_status(self) -> Status {
        let (if_ok, is_ok) = self;
        if is_ok {
            if_ok
        } else {
            Status::Fail
        }
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

    use anstyle::AnsiColor;
    use fancy_regex::Regex;
    use sqruff_lib_core::dialects::syntax::SyntaxKind;
    use sqruff_lib_core::errors::{ErrorStructRule, SQLLintError};
    use sqruff_lib_core::parser::markers::PositionMarker;
    use sqruff_lib_core::parser::segments::base::SegmentBuilder;
    use tempdir::TempDir;

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

    fn mk_formatter() -> (TempDir, OutputStreamFormatter) {
        let temp = TempDir::new(env!("CARGO_PKG_NAME")).unwrap();
        let file = Box::new(File::create(temp.path().join("out.txt")).unwrap());

        (temp, OutputStreamFormatter::new(file, false))
    }

    #[test]
    fn test_cli_formatters_filename_nocol() {
        let (_temp, formatter) = mk_formatter();
        let actual = formatter.format_filename("blahblah", true);

        assert_eq!(escape_ansi(&actual), "== [blahblah] PASS");
    }

    #[test]
    fn test_cli_formatters_violation() {
        let (_temp, formatter) = mk_formatter();

        let s = SegmentBuilder::token(0, "foobarbar", SyntaxKind::Word)
            .with_position(PositionMarker::new(
                10..19,
                10..19,
                "      \n\n  foobarbar".into(),
                None,
                None,
            ))
            .finish();

        let mut v = SQLLintError::new("DESC", s);

        v.rule = Some(ErrorStructRule {
            name: "some-name",
            code: "DESC",
        });

        let f = formatter.format_violation(v, 90);

        assert_eq!(escape_ansi(&f), "L:   3 | P:   3 | DESC | DESC [some-name]");
    }

    #[test]
    fn test_cli_helpers_colorize() {
        let (_temp, mut formatter) = mk_formatter();
        // Force color output for this test.
        formatter.plain_output = false;

        let actual = formatter.colorize("foo", AnsiColor::Red.on_default());
        assert_eq!(actual, "\u{1b}[31mfoo\u{1b}[0m");
    }
}
