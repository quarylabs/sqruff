use std::ops::Range;

use super::base::ErasedSegment;
use super::keyword::KeywordSegment;
use super::meta::{Indent, MetaSegment};
use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::init::dialect_selector;
use crate::core::linter::linter::Linter;
use crate::core::parser::lexer::{Lexer, StringOrTemplate};
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs, NewlineSegment,
    NewlineSegmentNewArgs, SymbolSegment, SymbolSegmentNewArgs, WhitespaceSegment,
    WhitespaceSegmentNewArgs,
};
use crate::core::templaters::base::TemplatedFile;
use crate::helpers::ToErasedSegment;

pub fn fresh_ansi_dialect() -> Dialect {
    dialect_selector("ansi").unwrap()
}

pub fn bracket_segments() -> Vec<ErasedSegment> {
    generate_test_segments_func(vec!["bar", " \t ", "(", "foo", "    ", ")", "baar", " \t ", "foo"])
}

pub fn parse_ansi_string(sql: &str) -> ErasedSegment {
    let linter = Linter::new(<_>::default(), None, None);
    linter.parse_string(sql.into(), None, None, None).unwrap().tree.unwrap()
}

pub fn lex(string: &str) -> Vec<ErasedSegment> {
    let config = FluffConfig::new(<_>::default(), None, None);
    let lexer = Lexer::new(&config, None);

    let (segments, errors) = lexer.lex(StringOrTemplate::String(string.into())).unwrap();
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

        let seg = if elem.chars().all(|c| c == ' ' || c == '\t') {
            WhitespaceSegment::create(elem, &position_marker, WhitespaceSegmentNewArgs)
        } else if elem.chars().all(|c| c == '\n') {
            NewlineSegment::create(elem, &position_marker, NewlineSegmentNewArgs {})
        } else if elem == "(" || elem == ")" {
            SymbolSegment::create(
                elem,
                &position_marker,
                SymbolSegmentNewArgs { r#type: "remove me" },
            )
        } else if elem.starts_with("--") {
            CommentSegment::create(
                elem,
                &position_marker,
                CommentSegmentNewArgs { r#type: "inline", trim_start: None },
            )
        } else if elem.starts_with('\"') {
            CodeSegment::create(
                elem,
                &position_marker,
                CodeSegmentNewArgs {
                    code_type: "double_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
            )
        } else if elem.starts_with('\'') {
            CodeSegment::create(
                elem,
                &position_marker,
                CodeSegmentNewArgs {
                    code_type: "single_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
            )
        } else {
            CodeSegment::create(
                elem,
                &position_marker,
                CodeSegmentNewArgs {
                    code_type: "",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
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
    main_list.push(ts.to_erased_segment() as ErasedSegment);
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
            .filter_map(|elem| {
                elem.get_raw().map(|raw| {
                    if matcher_keywords.contains(&raw.as_str()) {
                        KeywordSegment::new(raw, elem.get_position_marker().unwrap().into())
                            .to_erased_segment() as ErasedSegment
                    } else {
                        elem.clone()
                    }
                })
            })
            .collect(),
    }
}
