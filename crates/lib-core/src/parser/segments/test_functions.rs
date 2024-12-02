use std::ops::Range;

use super::base::{ErasedSegment, SegmentBuilder, Tables};
use crate::dialects::base::Dialect;
use crate::dialects::syntax::SyntaxKind;
use crate::helpers::Config;
use crate::parser::lexer::{Lexer, StringOrTemplate};
use crate::parser::markers::PositionMarker;
use crate::templaters::base::TemplatedFile;

pub fn bracket_segments() -> Vec<ErasedSegment> {
    generate_test_segments_func(vec![
        "bar", " \t ", "(", "foo", "    ", ")", "baar", " \t ", "foo",
    ])
}

pub fn lex(dialect: &Dialect, string: &str) -> Vec<ErasedSegment> {
    let lexer = Lexer::new(dialect.lexer_matchers());
    let tables = Tables::default();
    let (segments, errors) = lexer
        .lex(&tables, StringOrTemplate::String(string))
        .unwrap();
    assert_eq!(errors, &[]);
    segments
}

/// Roughly generate test segments.
///
/// This is a factory function so that it works as a fixture,
/// but when actually used, this will return the inner function
/// which is what you actually need.
pub fn generate_test_segments_func(elems: Vec<&str>) -> Vec<ErasedSegment> {
    // Placeholder: assuming TemplatedFile, PositionMarker, and other structures
    // are defined elsewhere in the codebase.
    let raw_file = elems.concat();

    let templated_file: TemplatedFile = raw_file.into();
    let mut idx = 0;
    let mut buff: Vec<ErasedSegment> = Vec::new();

    for elem in elems {
        if elem == "<indent>" {
            buff.push(SegmentBuilder::token(0, "", SyntaxKind::Indent).finish());
            continue;
        } else if elem == "<dedent>" {
            buff.push(SegmentBuilder::token(0, "", SyntaxKind::Dedent).finish());
            continue;
        }

        let position_marker = PositionMarker::new(
            idx..idx + elem.len(),
            idx..idx + elem.len(),
            templated_file.clone(),
            None,
            None,
        );

        let tables = Tables::default();

        let seg = if elem.chars().all(|c| c == ' ' || c == '\t') {
            SegmentBuilder::whitespace(tables.next_id(), elem).config(|this| {
                this.get_mut()
                    .set_position_marker(position_marker.clone().into())
            })
        } else if elem.chars().all(|c| c == '\n') {
            SegmentBuilder::newline(tables.next_id(), elem)
        } else if elem == "(" || elem == ")" {
            SegmentBuilder::token(tables.next_id(), elem, SyntaxKind::RawComparisonOperator)
                .with_position(position_marker)
                .finish()
        } else if elem.starts_with("--") {
            SegmentBuilder::token(0, elem, SyntaxKind::InlineComment)
                .with_position(position_marker)
                .finish()
        } else if elem.starts_with('\"') {
            SegmentBuilder::token(0, elem, SyntaxKind::DoubleQuote)
                .with_position(position_marker)
                .finish()
        } else if elem.starts_with('\'') {
            SegmentBuilder::token(0, elem, SyntaxKind::SingleQuote)
                .with_position(position_marker)
                .finish()
        } else {
            SegmentBuilder::token(0, elem, SyntaxKind::RawComparisonOperator)
                .with_position(position_marker)
                .finish()
        };

        buff.push(seg);
        idx += elem.len();
    }

    buff
}

/// Construct a list of raw segments as a fixture.
pub fn raw_segments() -> Vec<ErasedSegment> {
    generate_test_segments_func(["foobar", ".barfoo"].to_vec())
}

pub fn raw_seg() -> ErasedSegment {
    raw_segments()[0].clone()
}

pub fn test_segments() -> Vec<ErasedSegment> {
    generate_test_segments_func(vec!["bar", " \t ", "foo", "baar", " \t "])
}

pub fn make_result_tuple(
    result_slice: Option<Range<usize>>,
    matcher_keywords: &[&str],
    test_segments: &[ErasedSegment],
) -> Vec<ErasedSegment> {
    // Make a comparison tuple for test matching.
    // No result slice means no match.
    match result_slice {
        None => vec![],
        Some(slice) => test_segments[slice]
            .iter()
            .map(|elem| {
                let raw = elem.raw();
                if matcher_keywords.contains(&raw.as_str()) {
                    SegmentBuilder::keyword(0, raw).config(|this| {
                        this.get_mut()
                            .set_position_marker(Some(elem.get_position_marker().unwrap().clone()))
                    })
                } else {
                    elem.clone()
                }
            })
            .collect(),
    }
}
