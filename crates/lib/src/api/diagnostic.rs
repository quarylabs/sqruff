use std::ops::Range;

use sqruff_lib_core::errors::SQLBaseError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintDiagnostic {
    pub message: String,
    pub code: Option<String>,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub source_range: Range<usize>,
    pub fixable: bool,
}

impl LintDiagnostic {
    pub(crate) fn from_sql_error(error: &SQLBaseError, source: &str) -> Self {
        let source_range = canonical_source_range(error, source);
        let (line, column) = if error.line_no > 0 && error.line_pos > 0 {
            (error.line_no, error.line_pos)
        } else {
            line_column_for_byte(source, source_range.start)
        };
        let (end_line, end_column) = line_column_for_byte(source, source_range.end);

        Self {
            message: error.desc().to_string(),
            code: error.rule.as_ref().map(|rule| rule.code.to_string()),
            line,
            column,
            end_line,
            end_column,
            source_range,
            fixable: error.fixable,
        }
    }
}

fn canonical_source_range(error: &SQLBaseError, source: &str) -> Range<usize> {
    let mut range = clamp_range(error.source_slice.clone(), source);

    if range.is_empty()
        && error.line_no > 0
        && error.line_pos > 0
        && let Some(start) = byte_for_line_column(source, error.line_no, error.line_pos)
    {
        range = start..next_meaningful_boundary(source, start);
    }

    range
}

fn clamp_range(range: Range<usize>, source: &str) -> Range<usize> {
    let start = previous_char_boundary(source, range.start.min(source.len()));
    let end = previous_char_boundary(source, range.end.min(source.len()));

    if start <= end {
        start..end
    } else {
        start..start
    }
}

fn byte_for_line_column(source: &str, line: usize, column: usize) -> Option<usize> {
    let line_start = line_start_byte(source, line)?;
    let line_end = source[line_start..]
        .find('\n')
        .map_or(source.len(), |offset| line_start + offset);
    let target_chars = column.saturating_sub(1);

    let mut byte = line_end;
    for (index, (offset, _)) in source[line_start..line_end].char_indices().enumerate() {
        if index == target_chars {
            byte = line_start + offset;
            break;
        }
    }

    Some(byte)
}

fn line_start_byte(source: &str, line: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }

    let mut current_line = 1;
    let mut line_start = 0;

    for (idx, byte) in source.bytes().enumerate() {
        if current_line == line {
            return Some(line_start);
        }

        if byte == b'\n' {
            current_line += 1;
            line_start = idx + 1;
        }
    }

    (current_line == line).then_some(line_start)
}

fn next_meaningful_boundary(source: &str, start: usize) -> usize {
    if start >= source.len() {
        return start;
    }

    let start = previous_char_boundary(source, start);
    let mut chars = source[start..].char_indices();
    let Some((_, first)) = chars.next() else {
        return start;
    };

    if first == '\n' {
        return start + first.len_utf8();
    }

    if first.is_alphanumeric() || first == '_' {
        let mut end = start + first.len_utf8();
        for (offset, ch) in chars {
            if ch.is_alphanumeric() || ch == '_' {
                end = start + offset + ch.len_utf8();
            } else {
                break;
            }
        }
        return end;
    }

    start + first.len_utf8()
}

fn line_column_for_byte(source: &str, byte_offset: usize) -> (usize, usize) {
    let byte_offset = previous_char_boundary(source, byte_offset.min(source.len()));
    let mut line = 1;
    let mut line_start = 0;

    for (idx, byte) in source.bytes().enumerate() {
        if idx >= byte_offset {
            break;
        }

        if byte == b'\n' {
            line += 1;
            line_start = idx + 1;
        }
    }

    let column = source[line_start..byte_offset].chars().count() + 1;
    (line, column)
}

fn previous_char_boundary(source: &str, mut offset: usize) -> usize {
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[cfg(test)]
mod tests {
    use sqruff_lib_core::errors::{ErrorStructRule, SQLBaseError};

    use super::*;

    #[test]
    fn from_sql_error_uses_existing_source_range() {
        let error = SQLBaseError {
            description: "bad spacing".into(),
            rule: Some(ErrorStructRule {
                name: "layout.spacing",
                code: "LT01",
            }),
            line_no: 1,
            line_pos: 7,
            source_slice: 6..9,
            fixable: true,
        };

        let diagnostic = LintDiagnostic::from_sql_error(&error, "select   1\n");

        assert_eq!(diagnostic.code.as_deref(), Some("LT01"));
        assert_eq!(diagnostic.source_range, 6..9);
        assert_eq!((diagnostic.line, diagnostic.column), (1, 7));
        assert_eq!((diagnostic.end_line, diagnostic.end_column), (1, 10));
        assert!(diagnostic.fixable);
    }

    #[test]
    fn from_sql_error_expands_empty_range_from_line_column() {
        let error = SQLBaseError {
            description: "unparsable".into(),
            line_no: 2,
            line_pos: 1,
            source_slice: 0..0,
            ..Default::default()
        };

        let diagnostic = LintDiagnostic::from_sql_error(&error, "select 1\nfrom");

        assert_eq!(diagnostic.source_range, 9..13);
        assert_eq!((diagnostic.line, diagnostic.column), (2, 1));
        assert_eq!((diagnostic.end_line, diagnostic.end_column), (2, 5));
    }
}
