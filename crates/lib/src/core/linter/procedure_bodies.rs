//! Lint quoted Snowflake SQL procedure bodies as independent SQL files.
//!
//! Keep the wrapper's UdfBody token opaque. Source fixes on that token let the
//! normal patch machinery combine body edits with edits to the CREATE statement
//! without reinterpreting JavaScript, Python, or ordinary quoted literals as SQL.

use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::errors::{SQLBaseError, SQLFluffUserError};
use sqruff_lib_core::parser::markers::PositionMarker;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder, fix::SourceFix};
use sqruff_lib_core::templaters::TemplatedFile;
use sqruff_lib_core::value::Value;

use super::common::RenderedFile;
use super::core::{InheritedIgnore, Linter};
use crate::core::rules::{LintResult, noqa::IgnoreMask};

struct BodyLint<'a> {
    violations: Vec<SQLBaseError>,
    parse_errors: bool,
    ignore_mask: Option<&'a IgnoreMask>,
    inherited: Option<&'a InheritedIgnore<'a>>,
}

impl Linter {
    pub(super) fn lint_sql_procedure_bodies(
        &self,
        tree: &ErasedSegment,
        templated_file: &TemplatedFile,
        fix: bool,
        ignore_mask: Option<&IgnoreMask>,
        inherited: Option<&InheritedIgnore<'_>>,
    ) -> Result<(ErasedSegment, Vec<SQLBaseError>, bool), SQLFluffUserError> {
        if self.config().dialect_kind() != DialectKind::Snowflake {
            return Ok((tree.clone(), Vec::new(), false));
        }
        let mut state = BodyLint {
            violations: Vec::new(),
            parse_errors: false,
            ignore_mask,
            inherited,
        };
        let tree = self.lint_procedure_children(tree, templated_file, fix, &mut state)?;
        Ok((tree, state.violations, state.parse_errors))
    }

    fn lint_procedure_children(
        &self,
        tree: &ErasedSegment,
        templated_file: &TemplatedFile,
        fix: bool,
        state: &mut BodyLint<'_>,
    ) -> Result<ErasedSegment, SQLFluffUserError> {
        if !tree.is_type(SyntaxKind::CreateProcedureStatement)
            && !tree
                .descendant_type_set()
                .contains(SyntaxKind::CreateProcedureStatement)
        {
            return Ok(tree.clone());
        }
        let sql_procedure =
            tree.is_type(SyntaxKind::CreateProcedureStatement) && is_sql_procedure(tree);
        let children = tree
            .segments()
            .iter()
            .map(|child| {
                if sql_procedure && child.is_type(SyntaxKind::UdfBody) {
                    self.lint_procedure_body(child, templated_file, fix, state)
                } else {
                    self.lint_procedure_children(child, templated_file, fix, state)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tree.new(children))
    }

    fn lint_procedure_body(
        &self,
        body: &ErasedSegment,
        templated_file: &TemplatedFile,
        fix: bool,
        state: &mut BodyLint<'_>,
    ) -> Result<ErasedSegment, SQLFluffUserError> {
        let Some(marker) = body.get_position_marker() else {
            return Ok(body.clone());
        };
        // Never replace rendered template expressions with their rendered SQL.
        if !marker.is_literal() || marker.source_str() != body.raw().as_str() {
            return Ok(body.clone());
        }
        let Some(decoded) = QuotedBody::decode(body.raw()) else {
            return Ok(body.clone());
        };
        if decoded.sql.trim().is_empty() {
            return Ok(body.clone());
        }

        let mut config = self.config().clone();
        // File boundary rules apply to the surrounding file, not to the spaces
        // and newlines separating the SQL body from its string delimiters.
        let mut denylist = config
            .get("rule_denylist", "core")
            .as_array()
            .unwrap_or_default()
            .to_vec();
        denylist.extend([Value::String("LT12".into()), Value::String("LT13".into())]);
        config
            .raw
            .get_mut("core")
            .unwrap()
            .as_map_mut()
            .unwrap()
            .insert("rule_denylist".into(), Value::Array(denylist));
        // The containing file has already been templated. Do not render again.
        let linter = Linter::new(
            config,
            None,
            Some(&crate::templaters::RAW_TEMPLATER),
            self.include_parse_errors(),
        )
        .map_err(SQLFluffUserError::new)?;
        let mapped_ignore =
            |anchor: &ErasedSegment, rule: &crate::core::rules::ErasedRule, mark_used: bool| {
                let Some(local) = anchor.get_position_marker() else {
                    return false;
                };
                let start = decoded.source_offsets[local.source_slice.start.min(decoded.sql.len())];
                let end = decoded.source_offsets[local.source_slice.end.min(decoded.sql.len())];
                let mapped = SegmentBuilder::token(anchor.id(), anchor.raw(), anchor.get_type())
                    .with_position(PositionMarker::new(
                        marker.source_slice.start + start..marker.source_slice.start + end,
                        marker.templated_slice.start + start..marker.templated_slice.start + end,
                        templated_file.clone(),
                        None,
                        None,
                    ))
                    .finish();
                state.ignore_mask.is_some_and(|mask| {
                    mask.is_masked(
                        &LintResult::new(Some(mapped.clone()), Vec::new(), None, None),
                        Some(rule),
                        mark_used,
                    )
                }) || state
                    .inherited
                    .is_some_and(|is_ignored| is_ignored(&mapped, rule, mark_used))
            };
        let tables = sqruff_lib_core::parser::segments::Tables::default();
        // Parse the already-rendered body directly. Normalising decoded CRLF
        // would alter string values and invalidate the raw offset mapping.
        let parsed = linter.parse_rendered(
            &tables,
            RenderedFile {
                templated_file: decoded.sql.as_str().into(),
                alternate_templated_files: Vec::new(),
                templater_violations: Vec::new(),
                filename: templated_file.name().to_owned(),
                source_str: decoded.sql.clone(),
            },
        );
        let linted = linter.lint_parsed_with_ignore(&tables, parsed, fix, Some(&mapped_ignore))?;
        let invalid = linted.has_parse_or_templating_errors();
        state.parse_errors |= invalid;
        for violation in linted.violations() {
            let mut violation = violation.clone();
            let start = byte_position(&decoded.sql, violation.line_no, violation.line_pos);
            let end = violation.source_slice.end.max(start).min(decoded.sql.len());
            violation.source_slice = marker.source_slice.start + decoded.source_offsets[start]
                ..marker.source_slice.start + decoded.source_offsets[end];
            (violation.line_no, violation.line_pos) =
                templated_file.get_line_pos_of_char_pos(violation.source_slice.start, true);
            state.violations.push(violation);
        }
        if !fix || invalid || !linted.has_fixes() {
            return Ok(body.clone());
        }
        let fixed_sql = linted.fix_string();
        if !decoded.single_quoted && fixed_sql.contains("$$") {
            return Ok(body.clone());
        }
        let fixed_raw = decoded.encode(&fixed_sql);
        if fixed_raw == body.raw().as_str() {
            return Ok(body.clone());
        }
        Ok(
            SegmentBuilder::token(body.id(), body.raw(), SyntaxKind::UdfBody)
                .with_position(marker.clone())
                .with_source_fixes(vec![SourceFix::new(
                    fixed_raw.into(),
                    marker.source_slice.clone(),
                    marker.templated_slice.clone(),
                )])
                .finish(),
        )
    }
}

// LANGUAGE and its value are direct code children; parameter names and string
// contents cannot masquerade as a LANGUAGE clause. Snowflake defaults to SQL.
fn is_sql_procedure(procedure: &ErasedSegment) -> bool {
    let mut children = procedure
        .segments()
        .iter()
        .filter(|segment| segment.is_code());
    while let Some(child) = children.next() {
        if child.is_type(SyntaxKind::Keyword) && child.raw().eq_ignore_ascii_case("LANGUAGE") {
            return children
                .next()
                .is_some_and(|language| language.raw().eq_ignore_ascii_case("SQL"));
        }
    }
    true
}

/// A decoded string and a mapping from decoded byte boundaries to raw offsets.
struct QuotedBody {
    sql: String,
    source_offsets: Vec<usize>,
    single_quoted: bool,
}

impl QuotedBody {
    fn decode(raw: &str) -> Option<Self> {
        if let Some(sql) = raw
            .strip_prefix("$$")
            .and_then(|raw| raw.strip_suffix("$$"))
        {
            return Some(Self {
                sql: sql.to_owned(),
                source_offsets: (2..=sql.len() + 2).collect(),
                single_quoted: false,
            });
        }
        let contents = raw.strip_prefix('\'')?.strip_suffix('\'')?;
        let mut sql = String::new();
        let mut source_offsets = vec![1];
        let mut cursor = 0;
        while cursor < contents.len() {
            let start = cursor;
            let character = contents[cursor..].chars().next()?;
            cursor += character.len_utf8();
            let character = match character {
                '\'' if contents[cursor..].starts_with('\'') => {
                    cursor += 1;
                    '\''
                }
                '\\' => decode_escape(contents, &mut cursor)?,
                character => character,
            };
            sql.push(character);
            // Diagnostics land on character boundaries. Interior UTF-8 bytes
            // map to the start of their original character/escape sequence.
            source_offsets.extend(std::iter::repeat_n(start + 1, character.len_utf8() - 1));
            source_offsets.push(cursor + 1);
        }
        Some(Self {
            sql,
            source_offsets,
            single_quoted: true,
        })
    }

    fn encode(&self, sql: &str) -> String {
        if self.single_quoted {
            let mut encoded = String::from("'");
            for character in sql.chars() {
                match character {
                    '\\' => encoded.push_str("\\\\"),
                    '\'' => encoded.push_str("''"),
                    '\0' => encoded.push_str("\\0"),
                    '\r' => encoded.push_str("\\r"),
                    '\u{0008}' => encoded.push_str("\\b"),
                    '\u{000c}' => encoded.push_str("\\f"),
                    character => encoded.push(character),
                }
            }
            encoded.push('\'');
            encoded
        } else {
            format!("$${sql}$$")
        }
    }
}

fn decode_escape(contents: &str, cursor: &mut usize) -> Option<char> {
    let character = contents[*cursor..].chars().next()?;
    *cursor += character.len_utf8();
    Some(match character {
        'b' => '\u{0008}',
        'f' => '\u{000c}',
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'x' | 'u' => {
            let digits = if character == 'x' { 2 } else { 4 };
            let end = *cursor + digits;
            let value = u32::from_str_radix(contents.get(*cursor..end)?, 16).ok()?;
            *cursor = end;
            char::from_u32(value)?
        }
        '0'..='7' => {
            let start = *cursor - 1;
            while *cursor < contents.len()
                && *cursor - start < 3
                && matches!(contents.as_bytes()[*cursor], b'0'..=b'7')
            {
                *cursor += 1;
            }
            char::from_u32(u32::from_str_radix(&contents[start..*cursor], 8).ok()?)?
        }
        // Snowflake ignores the backslash in unknown escape sequences.
        character => character,
    })
}

fn byte_position(sql: &str, line: usize, column: usize) -> usize {
    let start = sql
        .match_indices('\n')
        .take(line.saturating_sub(1))
        .last()
        .map_or(0, |(index, _)| index + 1);
    start
        + sql[start..]
            .char_indices()
            .nth(column.saturating_sub(1))
            .map_or(sql.len() - start, |(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::QuotedBody;

    #[test]
    fn snowflake_single_quote_escapes_decode_and_roundtrip() {
        let raw = r#"'a''b\'c\"d\\e\b\f\n\r\t\0\041\x21\u26c4\z'"#;
        let body = QuotedBody::decode(raw).unwrap();
        assert_eq!(body.sql, "a'b'c\"d\\e\u{0008}\u{000c}\n\r\t\0!!⛄z");
        let encoded = body.encode(&body.sql);
        assert_eq!(QuotedBody::decode(&encoded).unwrap().sql, body.sql);
        assert_eq!(body.source_offsets.len(), body.sql.len() + 1);
    }

    #[test]
    fn dollar_quoted_contents_are_literal() {
        let body = QuotedBody::decode(r"$$RETURN '\n\u26c4';$$").unwrap();
        assert_eq!(body.sql, r"RETURN '\n\u26c4';");
        assert_eq!(body.encode(&body.sql), r"$$RETURN '\n\u26c4';$$");
    }
}
