//! Dialect-selected SQL inside quoted tokens.
//!
//! A quoted node retains its original spelling while exposing a parsed SQL file
//! to rules. Fixes edit that file normally; the enclosing node encodes the result
//! when producing source patches. No second linter or template rendering is used.

use smol_str::SmolStr;

use crate::dialects::syntax::SyntaxKind;
use crate::errors::SQLParseError;
use crate::parser::Parser;
use crate::parser::lexer::Lexer;
use crate::parser::markers::PositionMarker;
use crate::parser::segments::{ErasedSegment, SegmentBuilder, Tables};
use crate::templaters::{RawFileSlice, TemplateSliceKind, TemplatedFile, TemplatedFileSlice};

/// A dialect's declaration of a quoted SQL body in a parsed statement.
#[derive(Clone, Debug)]
pub struct QuotedBodyParser {
    parent_kind: SyntaxKind,
    body_kind: SyntaxKind,
    select: fn(&ErasedSegment) -> bool,
    backslash_escapes: bool,
}

impl QuotedBodyParser {
    pub fn new(
        parent_kind: SyntaxKind,
        body_kind: SyntaxKind,
        select: fn(&ErasedSegment) -> bool,
    ) -> Self {
        Self {
            parent_kind,
            body_kind,
            select,
            backslash_escapes: false,
        }
    }

    /// Enable backslash escapes in addition to doubled single quotes.
    pub fn backslash_escapes(mut self) -> Self {
        self.backslash_escapes = true;
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct QuotedSql {
    original: SmolStr,
    decoded: QuotedBody,
}

impl QuotedSql {
    pub(crate) fn raw(&self, file: &ErasedSegment) -> SmolStr {
        if file.raw() == self.decoded.sql.as_str() {
            self.original.clone()
        } else {
            self.decoded.encode(file.raw()).into()
        }
    }

    pub(crate) fn can_encode(&self, file: &ErasedSegment) -> bool {
        self.decoded.single_quoted || !file.raw().contains("$$")
    }
}

impl Parser<'_> {
    pub(crate) fn parse_quoted_bodies(
        &self,
        tables: &Tables,
        tree: &ErasedSegment,
    ) -> Result<ErasedSegment, SQLParseError> {
        if self.dialect().quoted_body_parsers().is_empty() {
            return Ok(tree.clone());
        }
        let count = tree.recursive_crawl_all(false).len();
        let mut remaining = self.max_parse_nodes().saturating_sub(count);
        self.parse_quoted_children(tables, tree, &mut remaining)
    }

    fn parse_quoted_children(
        &self,
        tables: &Tables,
        tree: &ErasedSegment,
        remaining: &mut usize,
    ) -> Result<ErasedSegment, SQLParseError> {
        if tree.quoted_file().is_some()
            || !self.dialect().quoted_body_parsers().iter().any(|parser| {
                tree.is_type(parser.parent_kind)
                    || tree.descendant_type_set().contains(parser.parent_kind)
            })
        {
            return Ok(tree.clone());
        }
        let body_parser = self
            .dialect()
            .quoted_body_parsers()
            .iter()
            .find(|parser| tree.is_type(parser.parent_kind) && (parser.select)(tree));
        let children = tree
            .segments()
            .iter()
            .map(|child| {
                if let Some(parser) = body_parser.filter(|parser| child.is_type(parser.body_kind)) {
                    self.parse_quoted_body(tables, child, parser, remaining)
                } else {
                    self.parse_quoted_children(tables, child, remaining)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tree.new(children))
    }

    fn parse_quoted_body(
        &self,
        tables: &Tables,
        token: &ErasedSegment,
        body_parser: &QuotedBodyParser,
        remaining: &mut usize,
    ) -> Result<ErasedSegment, SQLParseError> {
        let Some(marker) = token.get_position_marker() else {
            return Ok(token.clone());
        };
        // A whole-token rewrite must never bake a rendered template into source.
        if !marker.is_literal() || marker.source_str() != token.raw().as_str() {
            return Ok(token.clone());
        }
        let Some(decoded) = QuotedBody::decode(token.raw(), body_parser.backslash_escapes) else {
            return Ok(token.clone());
        };
        if decoded.sql.trim().is_empty() {
            return Ok(token.clone());
        }
        if self.max_parse_depth() == 1 {
            return Err(SQLParseError {
                description: "Maximum parse depth exceeded in quoted SQL body.".into(),
                segment: Some(token.clone()),
            });
        }
        let (segments, lex_errors) = Lexer::from(self.dialect()).lex(tables, decoded.sql.as_str());
        if self.max_parse_nodes() > 0 && segments.len().saturating_add(2) > *remaining {
            return Err(SQLParseError {
                description: format!(
                    "Maximum parse node count exceeded (limit {}).",
                    self.max_parse_nodes()
                ),
                segment: Some(token.clone()),
            });
        }
        // Keep the decoded SQL's working coordinates, and map source coordinates
        // back to the original file before constructing any parent markers.
        let source = &marker.templated_file.source_str;
        let local_file = TemplatedFile::new(
            source.clone(),
            marker.templated_file.name().into(),
            Some(decoded.sql.clone()),
            Some(decoded.slices(marker.source_slice.start)),
            Some(vec![RawFileSlice::new_typed(
                source.clone(),
                TemplateSliceKind::Literal,
                0,
                None,
                None,
            )]),
        )
        .expect("Quoted SQL source mapping is contiguous");
        let segments = segments
            .into_iter()
            .map(|mut segment| {
                let local = segment.get_position_marker().unwrap();
                let start =
                    marker.source_slice.start + decoded.source_offsets[local.source_slice.start];
                let end =
                    marker.source_slice.start + decoded.source_offsets[local.source_slice.end];
                let mapped = PositionMarker::new(
                    start..end,
                    local.templated_slice.clone(),
                    local_file.clone(),
                    Some(local.working_line_no),
                    Some(local.working_line_pos),
                );
                segment.make_mut().set_position_marker(Some(mapped));
                segment
            })
            .collect::<Vec<_>>();
        let parser = Parser::new_with_limits(
            self.dialect(),
            self.indentation_config(),
            self.max_parse_depth()
                .saturating_sub(usize::from(self.max_parse_depth() > 0)),
            if self.max_parse_nodes() > 0 {
                *remaining
            } else {
                0
            },
        );
        let file = if lex_errors.is_empty() {
            parser
                .parse(tables, &segments)?
                .expect("Lexer always emits EOF")
        } else {
            SegmentBuilder::node(
                tables.next_id(),
                SyntaxKind::File,
                self.dialect().name,
                vec![
                    SegmentBuilder::node(
                        tables.next_id(),
                        SyntaxKind::Unparsable,
                        self.dialect().name,
                        segments,
                    )
                    .position_from_segments()
                    .finish(),
                ],
            )
            .position_from_segments()
            .finish()
        };
        if self.max_parse_nodes() > 0 {
            *remaining = remaining.saturating_sub(file.recursive_crawl_all(false).len());
        }
        Ok(SegmentBuilder::node(
            token.id(),
            token.get_type(),
            self.dialect().name,
            vec![file],
        )
        .with_position(marker.clone())
        .with_quoted_sql(QuotedSql {
            original: token.raw().clone(),
            decoded,
        })
        .finish())
    }
}

/// A decoded string and a mapping from decoded byte boundaries to raw offsets.
#[derive(Debug, Clone)]
pub(crate) struct QuotedBody {
    sql: String,
    source_offsets: Vec<usize>,
    single_quoted: bool,
    backslash_escapes: bool,
}

impl QuotedBody {
    fn decode(raw: &str, backslash_escapes: bool) -> Option<Self> {
        if let Some(sql) = raw
            .strip_prefix("$$")
            .and_then(|raw| raw.strip_suffix("$$"))
        {
            return Some(Self {
                sql: sql.to_owned(),
                source_offsets: (2..=sql.len() + 2).collect(),
                single_quoted: false,
                backslash_escapes,
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
                '\\' if backslash_escapes => decode_escape(contents, &mut cursor)?,
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
            backslash_escapes,
        })
    }

    /// Map decoded text back to its spelling in the containing source file.
    fn slices(&self, source_start: usize) -> Vec<TemplatedFileSlice> {
        let mut slices: Vec<TemplatedFileSlice> = Vec::new();
        for (start, character) in self.sql.char_indices() {
            let end = start + character.len_utf8();
            let source =
                source_start + self.source_offsets[start]..source_start + self.source_offsets[end];
            let kind = if source.len() == end - start {
                TemplateSliceKind::Literal
            } else {
                TemplateSliceKind::Escaped
            };
            if let Some(last) = slices.last_mut()
                && kind == TemplateSliceKind::Literal
                && last.slice_type == kind
            {
                last.source_slice.end = source.end;
                last.templated_slice.end = end;
            } else {
                slices.push(TemplatedFileSlice::new_typed(kind, source, start..end));
            }
        }
        slices
    }

    fn encode(&self, sql: &str) -> String {
        if self.single_quoted {
            let mut encoded = String::from("'");
            for character in sql.chars() {
                match character {
                    '\\' if self.backslash_escapes => encoded.push_str("\\\\"),
                    '\'' => encoded.push_str("''"),
                    '\0' if self.backslash_escapes => encoded.push_str("\\0"),
                    '\r' if self.backslash_escapes => encoded.push_str("\\r"),
                    '\u{0008}' if self.backslash_escapes => encoded.push_str("\\b"),
                    '\u{000c}' if self.backslash_escapes => encoded.push_str("\\f"),
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
        // Unknown backslash escapes retain the escaped character.
        character => character,
    })
}

#[cfg(test)]
mod tests {
    use super::QuotedBody;

    #[test]
    fn snowflake_single_quote_escapes_decode_and_roundtrip() {
        let raw = r#"'a''b\'c\"d\\e\b\f\n\r\t\0\041\x21\u26c4\z'"#;
        let body = QuotedBody::decode(raw, true).unwrap();
        assert_eq!(body.sql, "a'b'c\"d\\e\u{0008}\u{000c}\n\r\t\0!!⛄z");
        let encoded = body.encode(&body.sql);
        assert_eq!(QuotedBody::decode(&encoded, true).unwrap().sql, body.sql);
        assert_eq!(body.source_offsets.len(), body.sql.len() + 1);
    }

    #[test]
    fn standard_single_quotes_preserve_backslashes_and_control_characters() {
        let body = QuotedBody::decode("'a\\b\r''c'", false).unwrap();
        assert_eq!(body.sql, "a\\b\r'c");
        assert_eq!(
            QuotedBody::decode(&body.encode(&body.sql), false)
                .unwrap()
                .sql,
            body.sql
        );
    }

    #[test]
    fn dollar_quoted_contents_are_literal() {
        let body = QuotedBody::decode(r"$$RETURN '\n\u26c4';$$", true).unwrap();
        assert_eq!(body.sql, r"RETURN '\n\u26c4';");
        assert_eq!(body.encode(&body.sql), r"$$RETURN '\n\u26c4';$$");
    }
}
