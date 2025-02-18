use anstyle::Style;
use std::borrow::Cow;
use std::io::IsTerminal;

pub fn should_produce_plain_output(nocolor: bool) -> bool {
    nocolor || !std::io::stdout().is_terminal()
}

pub fn colorize_helper(nocolor: bool, s: &str, style: Style) -> Cow<'_, str> {
    if nocolor {
        s.into()
    } else {
        format!("{style}{s}{style:#}").into()
    }
}

pub fn split_string_on_spaces(s: &str, line_length: usize) -> Vec<&str> {
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
