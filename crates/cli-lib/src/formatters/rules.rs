use super::utils::*;

use anstyle::{AnsiColor, Style};
use sqruff_lib::{core::rules::ErasedRule, rules::rules};
use std::borrow::Cow;
use std::ops::Range;

const BLUE: Style = AnsiColor::Blue.on_default();
const YELLOW: Style = AnsiColor::Yellow.on_default();

#[derive(Debug)]
pub(crate) struct RulesFormatter {
    rules: Vec<ErasedRule>,
    plain_output: bool,
    output_line_length: usize,
}

impl Default for RulesFormatter {
    fn default() -> Self {
        Self {
            rules: rules(),
            plain_output: false,
            output_line_length: 80,
        }
    }
}

impl RulesFormatter {
    pub(crate) fn new(nocolor: Option<bool>, output_line_length: i32) -> Self {
        Self {
            rules: rules(),
            plain_output: should_produce_plain_output(nocolor),
            output_line_length: usize::try_from(output_line_length).unwrap_or(0),
        }
    }

    fn colorize<'a>(&self, s: &'a str, style: Style) -> Cow<'a, str> {
        colorize_helper(self.plain_output, s, style)
    }

    fn format_groups(&self, rule: &ErasedRule) -> String {
        rule.groups()
            .iter()
            .map(|group| group.as_ref())
            .collect::<Vec<&str>>()
            .join(", ")
    }

    fn format_rule(&self, rule: &ErasedRule) -> String {
        let description = format!("[{}] {}", rule.name(), rule.description());
        let groups = format!("groups: {}", self.format_groups(rule));
        let indent = " ".repeat(rule.code().chars().count() + 2);
        let mut lines = Vec::new();
        for (value, colored, style) in [
            (&description, 1..1 + rule.name().len(), BLUE),
            (&groups, "groups: ".len()..groups.len(), YELLOW),
        ] {
            for chunk in wrap_field(rule.code(), value, self.output_line_length) {
                let prefix = if lines.is_empty() {
                    format!("{}: ", self.colorize(rule.code(), BLUE))
                } else {
                    indent.clone()
                };
                lines.push(format!(
                    "{prefix}{}",
                    self.colorize_chunk(value, chunk, colored.clone(), style)
                ));
            }
        }
        lines.join("\n")
    }

    fn colorize_chunk(
        &self,
        value: &str,
        chunk: Range<usize>,
        colored: Range<usize>,
        style: Style,
    ) -> String {
        let start = chunk.start.max(colored.start);
        let end = chunk.end.min(colored.end);
        if start >= end {
            return value[chunk].to_owned();
        }
        format!(
            "{}{}{}",
            &value[chunk.start..start],
            self.colorize(&value[start..end], style),
            &value[end..chunk.end]
        )
    }

    pub(crate) fn rules_info(&self) {
        println!("==== sqruff - rules ====");
        for rule in self.rules.clone() {
            let rule_info = self.format_rule(&rule);
            println!("{rule_info}");
        }
    }
}

/// Wrap values before applying ANSI colors, keeping at least one value column.
fn wrap_field(label: &str, value: &str, width: usize) -> Vec<Range<usize>> {
    let value_width = width.saturating_sub(label.chars().count() + 2).max(1);
    let mut chunks = Vec::new();
    let mut offset = 0;
    for paragraph in value.split_inclusive('\n') {
        let line = paragraph.trim_end_matches('\n');
        let mut start = 0;
        while start < line.len() {
            let remaining = &line[start..];
            let trimmed = remaining.trim_start();
            start += remaining.len() - trimmed.len();
            if trimmed.is_empty() {
                break;
            }
            let mut end = start
                + trimmed
                    .char_indices()
                    .nth(value_width)
                    .map_or(trimmed.len(), |(index, _)| index);
            if end < line.len() && !line[end..].starts_with(char::is_whitespace) {
                end = line[start..end]
                    .rfind(char::is_whitespace)
                    .map_or(end, |index| start + index);
            }
            let text = line[start..end].trim_end();
            chunks.push(offset + start..offset + start + text.len());
            start = end;
        }
        offset += paragraph.len();
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::{RulesFormatter, wrap_field};

    #[test]
    fn wrap_field_keeps_a_value_column_for_small_widths() {
        for width in [-1, 0, 1, 6, 7] {
            let formatter = RulesFormatter::new(Some(true), width);
            let chunks = wrap_field("CV01", "abc", formatter.output_line_length);
            let values = chunks
                .into_iter()
                .map(|range| &"abc"[range])
                .collect::<Vec<_>>();
            assert_eq!(values, ["a", "b", "c"]);
        }
    }

    #[test]
    fn wrap_field_preserves_unicode_and_breaks_long_words() {
        let value = "abc é😀xy\nlast";
        let chunks = wrap_field("CV01", value, 9);
        let values = chunks
            .into_iter()
            .map(|range| &value[range])
            .collect::<Vec<_>>();
        assert_eq!(values, ["abc", "é😀x", "y", "las", "t"]);
    }
}
