use std::ops::Range;

use super::base::ErasedSegment;
use super::meta::{Indent, MetaSegment};
use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::init::dialect_selector;
use crate::core::linter::linter::Linter;
use crate::core::parser::lexer::{Lexer, StringOrTemplate};
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::{CodeSegment, CodeSegmentNewArgs};
use crate::core::templaters::base::TemplatedFile;
use crate::dialects::ansi::Tables;
use crate::dialects::SyntaxKind;
use crate::helpers::{Config, ToErasedSegment};

pub fn fresh_ansi_dialect() -> Dialect {
    dialect_selector("ansi").unwrap()
}

pub fn bracket_segments() -> Vec<ErasedSegment> {
    generate_test_segments_func(vec!["bar", " \t ", "(", "foo", "    ", ")", "baar", " \t ", "foo"])
}

pub fn parse_ansi_string(sql: &str) -> ErasedSegment {
    let tables = Tables::default();
    let linter = Linter::new(<_>::default(), None, None);
    linter.parse_string(&tables, sql, None, None, None).unwrap().tree.unwrap()
}

pub fn lex(config: &FluffConfig, string: &str) -> Vec<ErasedSegment> {
    let lexer = Lexer::new(config, None);
    let tables = Tables::default();
    let (segments, errors) = lexer.lex(&tables, StringOrTemplate::String(string)).unwrap();
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

    let templated_file = TemplatedFile::from_string(raw_file);
    let mut idx = 0;
    let mut buff: Vec<ErasedSegment> = Vec::new();

    for elem in elems {
        if elem == "<indent>" {
            buff.push(Indent::indent().to_erased_segment());
            continue;
        } else if elem == "<dedent>" {
            buff.push(Indent::dedent().to_erased_segment());
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
            CodeSegment::create(
                tables.next_id(),
                elem,
                position_marker.clone().into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::Whitespace },
            )
        } else if elem.chars().all(|c| c == '\n') {
            CodeSegment::create(
                tables.next_id(),
                elem,
                position_marker.clone().into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::Newline },
            )
        } else if elem == "(" || elem == ")" {
            CodeSegment::of(tables.next_id(), elem, SyntaxKind::RawComparisonOperator)
                .config(|this| this.get_mut().set_position_marker(position_marker.clone().into()))
        } else if elem.starts_with("--") {
            CodeSegment::create(
                0,
                elem,
                position_marker.into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::InlineComment },
            )
        } else if elem.starts_with('\"') {
            CodeSegment::create(
                0,
                elem,
                position_marker.clone().into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::DoubleQuote },
            )
        } else if elem.starts_with('\'') {
            CodeSegment::create(
                0,
                elem,
                position_marker.clone().into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::SingleQuote },
            )
        } else {
            CodeSegment::create(
                0,
                elem,
                position_marker.clone().into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::RawComparisonOperator },
            )
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
    let mut main_list = generate_test_segments_func(vec!["bar", " \t ", "foo", "baar", " \t "]);
    let ts = MetaSegment::template(
        main_list.last().unwrap().get_position_marker().unwrap(),
        "{# comment #}",
        "comment",
    );
    main_list.push(ts.to_erased_segment());
    main_list
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
                if matcher_keywords.contains(&&*raw) {
                    CodeSegment::keyword(0, &raw).config(|this| {
                        this.get_mut()
                            .set_position_marker(Some(elem.get_position_marker().unwrap()))
                    })
                } else {
                    elem.clone()
                }
            })
            .collect(),
    }
}
