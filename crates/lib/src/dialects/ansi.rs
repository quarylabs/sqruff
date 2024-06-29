use std::borrow::Cow;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, OnceLock};

use ahash::AHashSet;
use itertools::{chain, Itertools};
use smol_str::SmolStr;
use uuid::Uuid;

use super::ansi_keywords::{ANSI_RESERVED_KEYWORDS, ANSI_UNRESERVED_KEYWORDS};
use super::SyntaxKind;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::{AliasInfo, ColumnAliasInfo};
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::grammar::anyof::{one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::{Anything, Nothing, Ref};
use crate::core::parser::grammar::conditional::Conditional;
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::{Matcher, Pattern};
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use crate::core::parser::segments::base::{
    pos_marker, CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs,
    ErasedSegment, IdentifierSegment, NewlineSegment, NewlineSegmentNewArgs, Segment,
    SymbolSegment, SymbolSegmentNewArgs, UnparsableSegment, WhitespaceSegment,
    WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::bracketed::BracketedSegment;
use crate::core::parser::segments::common::{ComparisonOperatorSegment, LiteralSegment};
use crate::core::parser::segments::fix::SourceFix;
use crate::core::parser::segments::generator::SegmentGenerator;
use crate::core::parser::segments::meta::MetaSegment;
use crate::core::parser::types::ParseMode;
use crate::helpers::{Config, ToErasedSegment, ToMatchable};

#[macro_export]
macro_rules! vec_of_erased {
    ($($elem:expr),* $(,)?) => {{
        vec![$(Arc::new($elem)),*]
    }};
}

trait BoxedE {
    fn boxed(self) -> Arc<Self>;
}

impl<T> BoxedE for T {
    fn boxed(self) -> Arc<Self>
    where
        Self: Sized,
    {
        Arc::new(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    kind: SyntaxKind,
    segments: Vec<ErasedSegment>,
    uuid: Uuid,
    position_marker: Option<PositionMarker>,
    raw: OnceLock<String>,
    source_fixes: Vec<SourceFix>,
    descendant_type_set: OnceLock<AHashSet<&'static str>>,
}

impl Node {
    fn new(kind: SyntaxKind, segments: Vec<ErasedSegment>) -> Self {
        let position_marker = pos_marker(&segments);
        Self {
            kind,
            segments,
            uuid: Uuid::new_v4(),
            position_marker: position_marker.into(),
            raw: OnceLock::new(),
            source_fixes: Vec::new(),
            descendant_type_set: OnceLock::new(),
        }
    }
}

impl Segment for Node {
    fn new(&self, segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self {
            kind: self.kind,
            uuid: self.uuid,
            segments,
            position_marker: self.position_marker.clone(),
            raw: OnceLock::new(),
            source_fixes: Vec::new(),
            descendant_type_set: OnceLock::new(),
        }
        .to_erased_segment()
    }

    fn descendant_type_set(&self) -> AHashSet<&'static str> {
        self.descendant_type_set
            .get_or_init(|| {
                let mut result_set = AHashSet::new();

                for seg in self.segments() {
                    result_set.extend(seg.descendant_type_set().union(&seg.class_types()));
                }

                result_set
            })
            .clone()
    }

    fn edit(&self, raw: Option<String>, source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        let mut cloned = self.clone();
        if let Some((a, b)) = cloned.raw.get_mut().zip(raw) {
            *a = b;
        };
        cloned.source_fixes = source_fixes.unwrap_or_default();
        cloned.to_erased_segment()
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        self.source_fixes.clone()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.get_or_init(|| self.segments().iter().map(|segment| segment.raw()).join("")).into()
    }

    fn get_type(&self) -> &'static str {
        self.kind.as_str()
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_marker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_marker = position_marker;
    }

    fn segments(&self) -> &[ErasedSegment] {
        &self.segments
    }

    fn set_segments(&mut self, segments: Vec<ErasedSegment>) {
        self.segments = segments;
    }

    fn get_uuid(&self) -> Uuid {
        self.uuid
    }

    fn class_types(&self) -> AHashSet<&'static str> {
        match self.kind {
            SyntaxKind::ColumnReference => ["object_reference", self.get_type()].into(),
            SyntaxKind::WildcardIdentifier => ["wildcard_identifier", "object_reference"].into(),
            _ => [self.get_type()].into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeMatcher {
    node_kind: SyntaxKind,
    pub(crate) match_grammar: Arc<dyn Matchable>,
}

impl NodeMatcher {
    pub fn new(node_kind: SyntaxKind, match_grammar: Arc<dyn Matchable>) -> Self {
        Self { node_kind, match_grammar }
    }
}

impl PartialEq for NodeMatcher {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl Matchable for NodeMatcher {
    fn get_type(&self) -> &'static str {
        self.node_kind.as_str()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let Some(match_grammar) = self.match_grammar() else {
            unimplemented!("{} has no match function implemented", std::any::type_name::<Self>())
        };

        if segments.len() == 1 && segments[0].get_type() == self.get_type() {
            return Ok(MatchResult::from_matched(segments.to_vec()));
        } else if segments.len() > 1 && segments[0].get_type() == self.get_type() {
            let (first_segment, remaining_segments) =
                segments.split_first().expect("segments should not be empty");
            return Ok(MatchResult {
                matched_segments: vec![first_segment.clone()],
                unmatched_segments: remaining_segments.to_vec(),
            });
        }

        let match_result = match_grammar.match_segments(segments, parse_context)?;
        if match_result.has_match() {
            Ok(MatchResult {
                matched_segments: vec![
                    Node::new(self.node_kind, match_result.matched_segments).to_erased_segment(),
                ],
                unmatched_segments: match_result.unmatched_segments,
            })
        } else {
            Ok(MatchResult::from_unmatched(segments.to_vec()))
        }
    }

    fn match_grammar(&self) -> Option<Arc<dyn Matchable>> {
        self.match_grammar.clone().into()
    }
}

pub fn ansi_dialect() -> Dialect {
    raw_dialect().config(|this| this.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut ansi_dialect = Dialect::new("FileSegment");

    ansi_dialect.set_lexer_matchers(lexer_matchers());

    // Set the bare functions
    ansi_dialect.sets_mut("bare_functions").extend([
        "current_timestamp",
        "current_time",
        "current_date",
    ]);

    // Set the datetime units
    ansi_dialect.sets_mut("datetime_units").extend([
        "DAY",
        "DAYOFYEAR",
        "HOUR",
        "MILLISECOND",
        "MINUTE",
        "MONTH",
        "QUARTER",
        "SECOND",
        "WEEK",
        "WEEKDAY",
        "YEAR",
    ]);

    ansi_dialect.sets_mut("date_part_function_name").extend(["DATEADD"]);

    // Set Keywords
    ansi_dialect
        .update_keywords_set_from_multiline_string("unreserved_keywords", ANSI_UNRESERVED_KEYWORDS);
    ansi_dialect
        .update_keywords_set_from_multiline_string("reserved_keywords", ANSI_RESERVED_KEYWORDS);

    // Bracket pairs (a set of tuples).
    // (name, startref, endref, persists)
    // NOTE: The `persists` value controls whether this type
    // of bracket is persisted during matching to speed up other
    // parts of the matching process. Round brackets are the most
    // common and match the largest areas and so are sufficient.
    ansi_dialect.update_bracket_sets(
        "bracket_pairs",
        vec![
            ("round", "StartBracketSegment", "EndBracketSegment", true),
            ("square", "StartSquareBracketSegment", "EndSquareBracketSegment", false),
            ("curly", "StartCurlyBracketSegment", "EndCurlyBracketSegment", false),
        ],
    );

    // Set the value table functions. These are functions that, if they appear as
    // an item in "FROM", are treated as returning a COLUMN, not a TABLE.
    // Apparently, among dialects supported by SQLFluff, only BigQuery has this
    // concept, but this set is defined in the ANSI dialect because:
    // - It impacts core linter rules (see AL04 and several other rules that
    //   subclass from it) and how they interpret the contents of table_expressions
    // - At least one other database (DB2) has the same value table function,
    //   UNNEST(), as BigQuery. DB2 is not currently supported by SQLFluff.
    ansi_dialect.sets_mut("value_table_functions");

    let symbol_factory = |segment: &dyn Segment| {
        SymbolSegment::create(
            &segment.raw(),
            segment.get_position_marker(),
            SymbolSegmentNewArgs { r#type: "remove me" },
        )
    };

    ansi_dialect.add([
        // Real segments
        ("DelimiterGrammar".into(), Ref::new("SemicolonSegment").to_matchable().into()),
        (
            "SemicolonSegment".into(),
            StringParser::new(
                ";",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "statement_terminator" },
                    )
                },
                Some("statement_terminator".into()),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "ColonSegment".into(),
            StringParser::new(":", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "SliceSegment".into(),
            StringParser::new(":", symbol_factory, "slice".to_owned().into(), false, None)
                .to_matchable()
                .into(),
        ),
        // NOTE: The purpose of the colon_delimiter is that it has different layout rules.
        // It assumes no whitespace on either side.
        (
            "ColonDelimiterSegment".into(),
            StringParser::new(":", symbol_factory, "slice".to_owned().into(), false, None)
                .to_matchable()
                .into(),
        ),
        (
            "StartBracketSegment".into(),
            StringParser::new(
                "(",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker().unwrap().into(),
                        SymbolSegmentNewArgs { r#type: "start_bracket" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EndBracketSegment".into(),
            StringParser::new(
                ")",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker().unwrap().into(),
                        SymbolSegmentNewArgs { r#type: "end_bracket" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "StartSquareBracketSegment".into(),
            StringParser::new(
                "[",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker().unwrap().into(),
                        SymbolSegmentNewArgs { r#type: "start_square_bracket" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EndSquareBracketSegment".into(),
            StringParser::new(
                "]",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "end_square_bracket" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "StartCurlyBracketSegment".into(),
            StringParser::new(
                "{",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "start_curly_bracket" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EndCurlyBracketSegment".into(),
            StringParser::new(
                "}",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "end_curly_bracket" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "CommaSegment".into(),
            StringParser::new(
                ",",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "comma" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DotSegment".into(),
            StringParser::new(
                ".",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "dot" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "StarSegment".into(),
            StringParser::new(
                "*",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "star" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "TildeSegment".into(),
            StringParser::new(
                "~",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "tilde" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "ParameterSegment".into(),
            StringParser::new("?", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "CastOperatorSegment".into(),
            StringParser::new(
                "::",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "casting_operator" },
                    )
                },
                Some("casting_operator".into()),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "PlusSegment".into(),
            StringParser::new(
                "+",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "MinusSegment".into(),
            StringParser::new(
                "-",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "PositiveSegment".into(),
            StringParser::new(
                "+",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "sign_indicator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "NegativeSegment".into(),
            StringParser::new(
                "-",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "sign_indicator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DivideSegment".into(),
            StringParser::new(
                "/",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "MultiplySegment".into(),
            StringParser::new(
                "*",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "ModuloSegment".into(),
            StringParser::new(
                "%",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "SlashSegment".into(),
            StringParser::new("/", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "AmpersandSegment".into(),
            StringParser::new("&", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "PipeSegment".into(),
            StringParser::new(
                "|",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "pipe" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "BitwiseXorSegment".into(),
            StringParser::new(
                "^",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "LikeOperatorSegment".into(),
            TypedParser::new(
                "like_operator",
                |it| {
                    ComparisonOperatorSegment::create(&it.raw(), &it.get_position_marker().unwrap())
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "RawNotSegment".into(),
            StringParser::new("!", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "RawEqualsSegment".into(),
            StringParser::new(
                "=",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "raw_comparison_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "RawGreaterThanSegment".into(),
            StringParser::new(
                ">",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "raw_comparison_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "RawLessThanSegment".into(),
            StringParser::new(
                "<",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "raw_comparison_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            // The following functions can be called without parentheses per ANSI specification
            "BareFunctionSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(
                    dialect.sets("bare_functions").into_iter().map(Into::into).collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs { code_type: "bare_function", ..Default::default() },
                        )
                    },
                    None,
                    false,
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        // The strange regex here it to make sure we don't accidentally match numeric
        // literals. We also use a regex to explicitly exclude disallowed keywords.
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                // Generate the anti template from the set of reserved keywords
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({})$", pattern);

                RegexParser::new(
                    "[A-Z0-9_]*[A-Z][A-Z0-9_]*",
                    |segment| {
                        IdentifierSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "naked_identifier",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    anti_template.into(),
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        (
            "ParameterNameSegment".into(),
            SegmentGenerator::new(|_dialect| {
                let pattern = r#"\"?[A-Z][A-Z0-9_]*\"?"#;

                RegexParser::new(
                    pattern,
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs::default(),
                        )
                    },
                    None,
                    false,
                    None,
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        (
            "FunctionNameIdentifierSegment".into(),
            TypedParser::new(
                "word",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "function_name_identifier" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        // Maybe data types should be more restrictive?
        (
            "DatatypeIdentifierSegment".into(),
            SegmentGenerator::new(|_| {
                // Generate the anti template from the set of reserved keywords
                // TODO - this is a stopgap until we implement explicit data types
                let anti_template = format!("^({})$", "NOT");

                one_of(vec![
                    RegexParser::new(
                        "[A-Z_][A-Z0-9_]*",
                        |segment| {
                            CodeSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                CodeSegmentNewArgs::default(),
                            )
                        },
                        None,
                        false,
                        anti_template.into(),
                        None,
                    )
                    .boxed(),
                    Ref::new("SingleIdentifierGrammar")
                        .exclude(Ref::new("NakedIdentifierSegment"))
                        .boxed(),
                ])
                .boxed()
            })
            .into(),
        ),
        // Ansi Intervals
        (
            "DatetimeUnitSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(
                    dialect.sets("datetime_units").into_iter().map(Into::into).collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs { code_type: "date_part", ..Default::default() },
                        )
                    },
                    None,
                    false,
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        (
            "DatePartFunctionName".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(
                    dialect
                        .sets("date_part_function_name")
                        .into_iter()
                        .map(Into::into)
                        .collect::<Vec<_>>(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs::default(),
                        )
                    },
                    None,
                    false,
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        (
            "QuotedIdentifierSegment".into(),
            TypedParser::new(
                "double_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "quoted_identifier" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            TypedParser::new("single_quote", symbol_factory, None, false, None)
                .to_matchable()
                .into(),
        ),
        (
            "SingleQuotedIdentifierSegment".into(),
            TypedParser::new(
                "single_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "quoted_identifier" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "NumericLiteralSegment".into(),
            TypedParser::new(
                "numeric_literal",
                |seg| {
                    LiteralSegment {
                        raw: seg.raw().into(),
                        position_maker: seg.get_position_marker().unwrap(),
                        uuid: seg.get_uuid(),
                    }
                    .to_erased_segment()
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        // NullSegment is defined separately to the keyword, so we can give it a different
        // type
        (
            "NullLiteralSegment".into(),
            StringParser::new(
                "null",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "null_literal" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "NanLiteralSegment".into(),
            StringParser::new(
                "nan",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "null_literal" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "TrueSegment".into(),
            StringParser::new(
                "true",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "boolean_literal" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "FalseSegment".into(),
            StringParser::new(
                "false",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "boolean_literal" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        // We use a GRAMMAR here not a Segment. Otherwise, we get an unnecessary layer
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment")
            ])
            .config(|this| this.terminators = vec_of_erased![Ref::new("DotSegment")])
            .to_matchable()
            .into(),
        ),
        (
            "BooleanLiteralGrammar".into(),
            one_of(vec_of_erased![Ref::new("TrueSegment"), Ref::new("FalseSegment")])
                .to_matchable()
                .into(),
        ),
        // We specifically define a group of arithmetic operators to make it easier to
        // override this if some dialects have different available operators
        (
            "ArithmeticBinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("PlusSegment"),
                Ref::new("MinusSegment"),
                Ref::new("DivideSegment"),
                Ref::new("MultiplySegment"),
                Ref::new("ModuloSegment"),
                Ref::new("BitwiseAndSegment"),
                Ref::new("BitwiseOrSegment"),
                Ref::new("BitwiseXorSegment"),
                Ref::new("BitwiseLShiftSegment"),
                Ref::new("BitwiseRShiftSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SignedSegmentGrammar".into(),
            one_of(vec_of_erased![Ref::new("PositiveSegment"), Ref::new("NegativeSegment")])
                .to_matchable()
                .into(),
        ),
        (
            "StringBinaryOperatorGrammar".into(),
            one_of(vec![Ref::new("ConcatSegment").boxed()]).to_matchable().into(),
        ),
        (
            "BooleanBinaryOperatorGrammar".into(),
            one_of(vec![
                Ref::new("AndOperatorGrammar").boxed(),
                Ref::new("OrOperatorGrammar").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ComparisonOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("EqualsSegment"),
                Ref::new("GreaterThanSegment"),
                Ref::new("LessThanSegment"),
                Ref::new("GreaterThanOrEqualToSegment"),
                Ref::new("LessThanOrEqualToSegment"),
                Ref::new("NotEqualToSegment"),
                Ref::new("LikeOperatorSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("FROM")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    Ref::keyword("NOT"),
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("FROM")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        // hookpoint for other dialects
        // e.g. EXASOL str to date cast with DATE '2021-01-01'
        // Give it a different type as needs to be single quotes and
        // should not be changed by rules (e.g. rule CV10)
        (
            "DateTimeLiteralGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATE"),
                    Ref::keyword("TIME"),
                    Ref::keyword("TIMESTAMP"),
                    Ref::keyword("INTERVAL")
                ]),
                TypedParser::new(
                    "single_quote",
                    |seg| LiteralSegment::create(&seg.raw(), &seg.get_position_marker().unwrap()),
                    None,
                    false,
                    None
                )
            ])
            .to_matchable()
            .into(),
        ),
        // Hookpoint for other dialects
        // e.g. INTO is optional in BIGQUERY
        (
            "MergeIntoLiteralGrammar".into(),
            Sequence::new(vec![Ref::keyword("MERGE").boxed(), Ref::keyword("INTO").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "LiteralGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("QuotedLiteralSegment"),
                Ref::new("NumericLiteralSegment"),
                Ref::new("BooleanLiteralGrammar"),
                Ref::new("QualifiedNumericLiteralSegment"),
                // NB: Null is included in the literals, because it is a keyword which
                // can otherwise be easily mistaken for an identifier.
                Ref::new("NullLiteralSegment"),
                Ref::new("DateTimeLiteralGrammar"),
                Ref::new("ArrayLiteralSegment"),
                Ref::new("TypedArrayLiteralSegment"),
                Ref::new("ObjectLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AndOperatorGrammar".into(),
            StringParser::new(
                "AND",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "OrOperatorGrammar".into(),
            StringParser::new(
                "OR",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "NotOperatorGrammar".into(),
            StringParser::new(
                "NOT",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "keyword" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            // This is a placeholder for other dialects.
            "PreTableFunctionKeywordsGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "BinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("ArithmeticBinaryOperatorGrammar"),
                Ref::new("StringBinaryOperatorGrammar"),
                Ref::new("BooleanBinaryOperatorGrammar"),
                Ref::new("ComparisonOperatorGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        // This pattern is used in a lot of places.
        // Defined here to avoid repetition.
        (
            "BracketedColumnReferenceListGrammar".into(),
            Bracketed::new(vec![
                Delimited::new(vec![Ref::new("ColumnReferenceSegment").boxed()]).boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrReplaceGrammar".into(),
            Sequence::new(vec![Ref::keyword("OR").boxed(), Ref::keyword("REPLACE").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "TemporaryTransientGrammar".into(),
            one_of(vec![Ref::keyword("TRANSIENT").boxed(), Ref::new("TemporaryGrammar").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "TemporaryGrammar".into(),
            one_of(vec![Ref::keyword("TEMP").boxed(), Ref::keyword("TEMPORARY").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "IfExistsGrammar".into(),
            Sequence::new(vec![Ref::keyword("IF").boxed(), Ref::keyword("EXISTS").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "IfNotExistsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::keyword("NOT"),
                Ref::keyword("EXISTS")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LikeGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIKE"),
                Ref::keyword("RLIKE"),
                Ref::keyword("ILIKE")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "UnionGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("UNION"),
                one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")])
                    .config(|this| this.optional())
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IsClauseGrammar".into(),
            one_of(vec![
                Ref::new("NullLiteralSegment").boxed(),
                Ref::new("NanLiteralSegment").boxed(),
                Ref::new("BooleanLiteralGrammar").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "InOperatorGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("NOT").optional(),
                Ref::keyword("IN"),
                one_of(vec_of_erased![
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("Expression_A_Grammar"),]),
                        Ref::new("SelectableGrammar"),
                    ])])
                    .config(|this| this.parse_mode(ParseMode::Greedy)),
                    Ref::new("FunctionSegment"), // E.g. UNNEST()
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("FROM"),
                Ref::keyword("WHERE"),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Ref::keyword("LIMIT"),
                Ref::keyword("OVERLAPS"),
                Ref::new("SetOperatorSegment"),
                Ref::keyword("FETCH"),
            ])
            .to_matchable()
            .into(),
        ),
        // Define these as grammars to allow child dialects to enable them (since they are
        // non-standard keywords)
        ("IsNullGrammar".into(), Nothing::new().to_matchable().into()),
        ("NotNullGrammar".into(), Nothing::new().to_matchable().into()),
        ("CollateGrammar".into(), Nothing::new().to_matchable().into()),
        (
            "FromClauseTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("WHERE").boxed(),
                Ref::keyword("LIMIT").boxed(),
                Sequence::new(vec![Ref::keyword("GROUP").boxed(), Ref::keyword("BY").boxed()])
                    .boxed(),
                Sequence::new(vec![Ref::keyword("ORDER").boxed(), Ref::keyword("BY").boxed()])
                    .boxed(),
                Ref::keyword("HAVING").boxed(),
                Ref::keyword("QUALIFY").boxed(),
                Ref::keyword("WINDOW").boxed(),
                Ref::new("SetOperatorSegment").boxed(),
                Ref::new("WithNoSchemaBindingClauseSegment").boxed(),
                Ref::new("WithDataClauseSegment").boxed(),
                Ref::keyword("FETCH").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("LIMIT").boxed(),
                Sequence::new(vec![Ref::keyword("GROUP").boxed(), Ref::keyword("BY").boxed()])
                    .boxed(),
                Sequence::new(vec![Ref::keyword("ORDER").boxed(), Ref::keyword("BY").boxed()])
                    .boxed(),
                Ref::keyword("HAVING").boxed(),
                Ref::keyword("QUALIFY").boxed(),
                Ref::keyword("WINDOW").boxed(),
                Ref::keyword("OVERLAPS").boxed(),
                Ref::keyword("FETCH").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseTerminatorGrammar".into(),
            one_of(vec![
                Sequence::new(vec![Ref::keyword("ORDER").boxed(), Ref::keyword("BY").boxed()])
                    .boxed(),
                Ref::keyword("LIMIT").boxed(),
                Ref::keyword("HAVING").boxed(),
                Ref::keyword("QUALIFY").boxed(),
                Ref::keyword("WINDOW").boxed(),
                Ref::keyword("FETCH").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseTerminatorGrammar".into(),
            one_of(vec![
                Sequence::new(vec![Ref::keyword("ORDER").boxed(), Ref::keyword("BY").boxed()])
                    .boxed(),
                Ref::keyword("LIMIT").boxed(),
                Ref::keyword("QUALIFY").boxed(),
                Ref::keyword("WINDOW").boxed(),
                Ref::keyword("FETCH").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrderByClauseTerminators".into(),
            one_of(vec![
                Ref::keyword("LIMIT").boxed(),
                Ref::keyword("HAVING").boxed(),
                Ref::keyword("QUALIFY").boxed(),
                Ref::keyword("WINDOW").boxed(),
                Ref::new("FrameClauseUnitGrammar").boxed(),
                Ref::keyword("SEPARATOR").boxed(),
                Ref::keyword("FETCH").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PrimaryKeyGrammar".into(),
            Sequence::new(vec![Ref::keyword("PRIMARY").boxed(), Ref::keyword("KEY").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "ForeignKeyGrammar".into(),
            Sequence::new(vec![Ref::keyword("FOREIGN").boxed(), Ref::keyword("KEY").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "UniqueKeyGrammar".into(),
            Sequence::new(vec![Ref::keyword("UNIQUE").boxed()]).to_matchable().into(),
        ),
        // Odd syntax, but prevents eager parameters being confused for data types
        (
            "FunctionParameterGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("ParameterNameSegment").optional().boxed(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ANY").boxed(),
                            Ref::keyword("TYPE").boxed(),
                        ])
                        .boxed(),
                        Ref::new("DatatypeSegment").boxed(),
                    ])
                    .boxed(),
                ])
                .boxed(),
                one_of(vec![
                    Sequence::new(vec![Ref::keyword("ANY").boxed(), Ref::keyword("TYPE").boxed()])
                        .boxed(),
                    Ref::new("DatatypeSegment").boxed(),
                ])
                .boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AutoIncrementGrammar".into(),
            Sequence::new(vec![Ref::keyword("AUTO_INCREMENT").boxed()]).to_matchable().into(),
        ),
        // Base Expression element is the right thing to reference for everything
        // which functions as an expression, but could include literals.
        (
            "BaseExpressionElementGrammar".into(),
            one_of(vec![
                Ref::new("LiteralGrammar").boxed(),
                Ref::new("BareFunctionSegment").boxed(),
                Ref::new("IntervalExpressionSegment").boxed(),
                Ref::new("FunctionSegment").boxed(),
                Ref::new("ColumnReferenceSegment").boxed(),
                Ref::new("ExpressionSegment").boxed(),
                Sequence::new(vec![
                    Ref::new("DatatypeSegment").boxed(),
                    Ref::new("LiteralGrammar").boxed(),
                ])
                .boxed(),
            ])
            .config(|this| {
                // These terminators allow better performance by giving a signal
                // of a likely complete match if they come after a match. For
                // example "123," only needs to match against the LiteralGrammar
                // and because a comma follows, never be matched against
                // ExpressionSegment or FunctionSegment, which are both much
                // more complicated.

                this.terminators = vec_of_erased![
                    Ref::new("CommaSegment"),
                    Ref::keyword("AS"),
                    // TODO: We can almost certainly add a few more here.
                ];
            })
            .to_matchable()
            .into(),
        ),
        (
            "FilterClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("FILTER").boxed(),
                Bracketed::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("WHERE").boxed(),
                        Ref::new("ExpressionSegment").boxed(),
                    ])
                    .boxed(),
                ])
                .boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IgnoreRespectNullsGrammar".into(),
            Sequence::new(vec![
                one_of(vec![Ref::keyword("IGNORE").boxed(), Ref::keyword("RESPECT").boxed()])
                    .boxed(),
                Ref::keyword("NULLS").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FrameClauseUnitGrammar".into(),
            one_of(vec![Ref::keyword("ROWS").boxed(), Ref::keyword("RANGE").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "JoinTypeKeywordsGrammar".into(),
            one_of(vec![
                Ref::keyword("CROSS").boxed(),
                Ref::keyword("INNER").boxed(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FULL").boxed(),
                        Ref::keyword("LEFT").boxed(),
                        Ref::keyword("RIGHT").boxed(),
                    ])
                    .boxed(),
                    Ref::keyword("OUTER").optional().boxed(),
                ])
                .boxed(),
            ])
            .config(|this| this.optional())
            .to_matchable()
            .into(),
        ),
        (
            // It's as a sequence to allow to parametrize that in Postgres dialect with LATERAL
            "JoinKeywordsGrammar".into(),
            Sequence::new(vec![Ref::keyword("JOIN").boxed()]).to_matchable().into(),
        ),
        (
            // NATURAL joins are not supported in all dialects (e.g. not in Bigquery
            // or T-SQL). So define here to allow override with Nothing() for those.
            "NaturalJoinKeywordsGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("NATURAL").boxed(),
                one_of(vec![
                    // Note: NATURAL joins do not support CROSS joins
                    Ref::keyword("INNER").boxed(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("LEFT").boxed(),
                            Ref::keyword("RIGHT").boxed(),
                            Ref::keyword("FULL").boxed(),
                        ])
                        .boxed(),
                        Ref::keyword("OUTER").optional().boxed(),
                    ])
                    .config(|this| this.optional())
                    .boxed(),
                ])
                .config(|this| this.optional())
                .boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        // This can be overwritten by dialects
        ("ExtendedNaturalJoinKeywordsGrammar".into(), Nothing::new().to_matchable().into()),
        ("NestedJoinGrammar".into(), Nothing::new().to_matchable().into()),
        (
            "ReferentialActionGrammar".into(),
            one_of(vec![
                Ref::keyword("RESTRICT").boxed(),
                Ref::keyword("CASCADE").boxed(),
                Sequence::new(vec![Ref::keyword("SET").boxed(), Ref::keyword("NULL").boxed()])
                    .boxed(),
                Sequence::new(vec![Ref::keyword("NO").boxed(), Ref::keyword("ACTION").boxed()])
                    .boxed(),
                Sequence::new(vec![Ref::keyword("SET").boxed(), Ref::keyword("DEFAULT").boxed()])
                    .boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DropBehaviorGrammar".into(),
            one_of(vec![Ref::keyword("RESTRICT").boxed(), Ref::keyword("CASCADE").boxed()])
                .config(|this| this.optional())
                .to_matchable()
                .into(),
        ),
        (
            "ColumnConstraintDefaultGrammar".into(),
            one_of(vec![
                Ref::new("ShorthandCastSegment").boxed(),
                Ref::new("LiteralGrammar").boxed(),
                Ref::new("FunctionSegment").boxed(),
                Ref::new("BareFunctionSegment").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ReferenceDefinitionGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("REFERENCES"),
                Ref::new("TableReferenceSegment"),
                // Foreign columns making up FOREIGN KEY constraint
                Ref::new("BracketedColumnReferenceListGrammar").optional(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MATCH"),
                    one_of(vec_of_erased![
                        Ref::keyword("FULL"),
                        Ref::keyword("PARTIAL"),
                        Ref::keyword("SIMPLE")
                    ])
                ])
                .config(|this| this.optional()),
                AnyNumberOf::new(vec_of_erased![
                    // ON DELETE clause, e.g. ON DELETE NO ACTION
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ON"),
                        Ref::keyword("DELETE"),
                        Ref::new("ReferentialActionGrammar")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ON"),
                        Ref::keyword("UPDATE"),
                        Ref::new("ReferentialActionGrammar")
                    ])
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TrimParametersGrammar".into(),
            one_of(vec![
                Ref::keyword("BOTH").boxed(),
                Ref::keyword("LEADING").boxed(),
                Ref::keyword("TRAILING").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DefaultValuesGrammar".into(),
            Sequence::new(vec![Ref::keyword("DEFAULT").boxed(), Ref::keyword("VALUES").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "ObjectReferenceDelimiterGrammar".into(),
            one_of(vec![
                Ref::new("DotSegment").boxed(),
                // NOTE: The double dot syntax allows for default values.
                Sequence::new(vec![Ref::new("DotSegment").boxed(), Ref::new("DotSegment").boxed()])
                    .boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ObjectReferenceTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("ON").boxed(),
                Ref::keyword("AS").boxed(),
                Ref::keyword("USING").boxed(),
                Ref::new("CommaSegment").boxed(),
                Ref::new("CastOperatorSegment").boxed(),
                Ref::new("StartSquareBracketSegment").boxed(),
                Ref::new("StartBracketSegment").boxed(),
                Ref::new("BinaryOperatorGrammar").boxed(),
                Ref::new("ColonSegment").boxed(),
                Ref::new("DelimiterGrammar").boxed(),
                Ref::new("JoinLikeClauseGrammar").boxed(),
                Bracketed::new(vec![]).boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AlterTableOptionsGrammar".into(),
            one_of(vec_of_erased![
                // Table options
                Sequence::new(vec_of_erased![
                    Ref::new("ParameterNameSegment"),
                    Ref::new("EqualsSegment").optional(),
                    one_of(vec_of_erased![
                        Ref::new("LiteralGrammar"),
                        Ref::new("NakedIdentifierSegment")
                    ])
                ]),
                // Add things
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("ADD"), Ref::keyword("MODIFY")]),
                    Ref::keyword("COLUMN").optional(),
                    Ref::new("ColumnDefinitionSegment"),
                    one_of(vec_of_erased![Sequence::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::keyword("FIRST"),
                        Ref::keyword("AFTER"),
                        Ref::new("ColumnReferenceSegment"),
                        // Bracketed Version of the same
                        Ref::new("BracketedColumnReferenceListGrammar")
                    ])])])
                    .config(|this| this.optional())
                ]),
                // Rename
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    one_of(vec_of_erased![Ref::keyword("AS"), Ref::keyword("TO")])
                        .config(|this| this.optional()),
                    Ref::new("TableReferenceSegment")
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    ansi_dialect.add([
        (
            "FileSegment".into(),
            NodeMatcher::new(
                SyntaxKind::File,
                Delimited::new(vec![Ref::new("StatementSegment").boxed()])
                    .config(|this| {
                        this.allow_trailing();
                        this.delimiter(
                            AnyNumberOf::new(vec![Ref::new("DelimiterGrammar").boxed()])
                                .config(|config| config.min_times(1)),
                        );
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ColumnReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ColumnReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar")))
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ObjectReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ObjectReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::Expression,
                Ref::new("Expression_A_Grammar").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WildcardIdentifierSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WildcardIdentifier,
                Sequence::new(vec![
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").boxed(),
                            Ref::new("ObjectReferenceDelimiterGrammar").boxed(),
                        ])
                        .boxed(),
                    ])
                    .boxed(),
                    Ref::new("StarSegment").boxed(),
                ])
                .allow_gaps(false)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "NamedWindowExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::NamedWindowExpression,
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("AS"),
                    one_of(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Bracketed::new(vec_of_erased![Ref::new("WindowSpecificationSegment")])
                            .config(|this| this.parse_mode(ParseMode::Greedy)),
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FunctionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::Function,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("DatePartFunctionNameSegment"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Ref::new("DatetimeUnitSegment"),
                            Ref::new("FunctionContentsGrammar").optional()
                        ])])
                        .config(|this| this.parse_mode(ParseMode::Greedy))
                    ])]),
                    Sequence::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("FunctionNameSegment").exclude(one_of(vec_of_erased![
                                Ref::new("DatePartFunctionNameSegment"),
                                Ref::new("ValuesClauseSegment")
                            ])),
                            Bracketed::new(vec_of_erased![
                                Ref::new("FunctionContentsGrammar").optional()
                            ])
                            .config(|this| this.parse_mode(ParseMode::Greedy))
                        ]),
                        Ref::new("PostFunctionGrammar").optional()
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::HavingClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("HAVING"),
                    MetaSegment::implicit_indent(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "PathSegment".into(),
            NodeMatcher::new(
                SyntaxKind::PathSegment,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("SlashSegment"),
                        Delimited::new(vec_of_erased![TypedParser::new(
                            "word",
                            |segment: &dyn Segment| {
                                CodeSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    CodeSegmentNewArgs {
                                        code_type: "path_segment",
                                        ..Default::default()
                                    },
                                )
                            },
                            None,
                            false,
                            None,
                        )])
                        .config(|this| {
                            this.allow_gaps = false;
                            this.delimiter(Ref::new("SlashSegment"));
                        }),
                    ]),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "LimitClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::LimitClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIMIT"),
                    MetaSegment::indent(),
                    optionally_bracketed(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("ALL"),
                    ])]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("OFFSET"),
                            one_of(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                Ref::new("ExpressionSegment"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::new("CommaSegment"),
                            Ref::new("NumericLiteralSegment"),
                        ]),
                    ])
                    .config(|this| this.optional()),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CubeRollupClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CubeRollupClause,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("CubeFunctionNameSegment"),
                        Ref::new("RollupFunctionNameSegment"),
                    ]),
                    Bracketed::new(vec_of_erased![Ref::new("GroupingExpressionList")]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "RollupFunctionNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RollupFunctionName,
                StringParser::new(
                    "ROLLUP",
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs::default(),
                        )
                    },
                    None,
                    false,
                    None,
                )
                .boxed(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CubeFunctionNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CubeFunctionName,
                StringParser::new(
                    "CUBE",
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs::default(),
                        )
                    },
                    None,
                    false,
                    None,
                )
                .boxed(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "GroupingSetsClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::GroupingSetsClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("GROUPING"),
                    Ref::keyword("SETS"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Ref::new("CubeRollupClauseSegment"),
                        Ref::new("GroupingExpressionList"),
                    ])]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "GroupingExpressionList".into(),
            NodeMatcher::new(
                SyntaxKind::GroupingExpressionList,
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("ExpressionSegment"),
                            Bracketed::new(vec_of_erased![]),
                        ]),
                        Ref::new("GroupByClauseTerminatorGrammar"),
                    ]),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SetClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SetClause,
                Sequence::new(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("LiteralGrammar"),
                        Ref::new("BareFunctionSegment"),
                        Ref::new("FunctionSegment"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("ExpressionSegment"),
                        Ref::new("ValuesClauseSegment"),
                        Ref::keyword("DEFAULT"),
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FetchClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FetchClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("FETCH"),
                    one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("NEXT")]),
                    Ref::new("NumericLiteralSegment").optional(),
                    one_of(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("ROWS")]),
                    Ref::keyword("ONLY"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FunctionDefinitionGrammar".into(),
            NodeMatcher::new(
                SyntaxKind::FunctionDefinition,
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    Ref::new("QuotedLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LANGUAGE"),
                        Ref::new("NakedIdentifierSegment")
                    ])
                    .config(|this| this.optional()),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AlterSequenceOptionsSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AlterSequenceOptionsSegment,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INCREMENT"),
                        Ref::keyword("BY"),
                        Ref::new("NumericLiteralSegment")
                    ]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MINVALUE"),
                            Ref::new("NumericLiteralSegment")
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MINVALUE")])
                    ]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MAXVALUE"),
                            Ref::new("NumericLiteralSegment")
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MAXVALUE")])
                    ]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CACHE"),
                            Ref::new("NumericLiteralSegment")
                        ]),
                        Ref::keyword("NOCACHE")
                    ]),
                    one_of(vec_of_erased![Ref::keyword("CYCLE"), Ref::keyword("NOCYCLE")]),
                    one_of(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("NOORDER")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "RoleReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RoleReference,
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TablespaceReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TablespaceReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ExtensionReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ExtensionReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TagReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TagReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ColumnDefinitionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ColumnDefinition,
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"), // Column name
                    Ref::new("DatatypeSegment"),         // Column type,
                    Bracketed::new(vec_of_erased![Anything::new()]).config(|this| this.optional()),
                    AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")])
                        .config(|this| this.optional())
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ColumnConstraintSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ColumnConstraintSegment,
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONSTRAINT"),
                        Ref::new("ObjectReferenceSegment"), // Constraint name
                    ])
                    .config(|this| this.optional()),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NOT").optional(),
                            Ref::keyword("NULL"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CHECK"),
                            Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::new("ColumnConstraintDefaultGrammar"),
                        ]),
                        Ref::new("PrimaryKeyGrammar"),
                        Ref::new("UniqueKeyGrammar"), // UNIQUE
                        Ref::new("AutoIncrementGrammar"),
                        Ref::new("ReferenceDefinitionGrammar"), /* REFERENCES reftable [ (
                                                                 * refcolumn) ] */
                        Ref::new("CommentClauseSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLLATE"),
                            Ref::new("CollationReferenceSegment"),
                        ]), // COLLATE
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CommentClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CommentClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::new("QuotedLiteralSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TableEndClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::TableEndClause, Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "MergeMatchSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeMatch,
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("MergeMatchedClauseSegment"),
                    Ref::new("MergeNotMatchedClauseSegment")
                ])
                .config(|this| this.min_times(1))
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeMatchedClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeWhenMatchedClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHEN"),
                    Ref::keyword("MATCHED"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AND"),
                        Ref::new("ExpressionSegment")
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("THEN"),
                    MetaSegment::indent(),
                    one_of(vec_of_erased![
                        Ref::new("MergeUpdateClauseSegment"),
                        Ref::new("MergeDeleteClauseSegment")
                    ]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeNotMatchedClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeWhenNotMatchedClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHEN"),
                    Ref::keyword("NOT"),
                    Ref::keyword("MATCHED"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AND"),
                        Ref::new("ExpressionSegment")
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("THEN"),
                    MetaSegment::indent(),
                    Ref::new("MergeInsertClauseSegment"),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeInsertClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeInsertClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    MetaSegment::indent(),
                    Ref::new("BracketedColumnReferenceListGrammar").optional(),
                    MetaSegment::dedent(),
                    Ref::new("ValuesClauseSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeUpdateClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeUpdateClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("UPDATE"),
                    MetaSegment::indent(),
                    Ref::new("SetClauseListSegment"),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeDeleteClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeDeleteClause, Ref::keyword("DELETE").to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "SetClauseListSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SetClauseList,
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    MetaSegment::indent(),
                    Ref::new("SetClauseSegment"),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("CommaSegment"),
                        Ref::new("SetClauseSegment"),
                    ]),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TableReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TableReference,
                Ref::new("ObjectReferenceSegment").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SchemaReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SchemaReference,
                Ref::new("ObjectReferenceSegment").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SingleIdentifierListSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SingleIdentifierList,
                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                    .config(|this| this.optional())
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::GroupByClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("GROUP"),
                    Ref::keyword("BY"),
                    one_of(vec_of_erased![
                        Ref::new("CubeRollupClauseSegment"),
                        Sequence::new(vec_of_erased![
                            MetaSegment::indent(),
                            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                Ref::new("NumericLiteralSegment"),
                                Ref::new("ExpressionSegment"),
                            ])])
                            .config(|this| {
                                this.terminators =
                                    vec![Ref::new("GroupByClauseTerminatorGrammar").boxed()];
                            }),
                            MetaSegment::dedent()
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FrameClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FrameClause,
                Sequence::new(vec_of_erased![
                    Ref::new("FrameClauseUnitGrammar"),
                    one_of(vec_of_erased![
                        frame_extent(),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("BETWEEN"),
                            frame_extent(),
                            Ref::keyword("AND"),
                            frame_extent(),
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WithCompoundStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WithCompoundStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("RECURSIVE").optional(),
                    Delimited::new(vec_of_erased![Ref::new("CTEDefinitionSegment")]).config(
                        |this| {
                            this.terminators = vec_of_erased![Ref::keyword("SELECT")];
                            this.allow_trailing();
                        }
                    ),
                    one_of(vec_of_erased![
                        Ref::new("NonWithSelectableGrammar"),
                        Ref::new("NonWithNonSelectableGrammar")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CTEDefinitionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CommonTableExpression,
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("CTEColumnList").optional(),
                    Ref::keyword("AS").optional(),
                    Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CTEColumnList".into(),
            NodeMatcher::new(
                SyntaxKind::CTEColumnList,
                Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SequenceReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SequenceReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TriggerReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TriggerReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TableConstraintSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TableConstraint,
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONSTRAINT"),
                        Ref::new("ObjectReferenceSegment")
                    ])
                    .config(|this| this.optional()),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("UNIQUE"),
                            Ref::new("BracketedColumnReferenceListGrammar")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::new("PrimaryKeyGrammar"),
                            Ref::new("BracketedColumnReferenceListGrammar")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::new("ForeignKeyGrammar"),
                            Ref::new("BracketedColumnReferenceListGrammar"),
                            Ref::new("ReferenceDefinitionGrammar")
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "JoinOnConditionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::JoinOnCondition,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Conditional::new(MetaSegment::implicit_indent()).indented_on_contents(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    Conditional::new(MetaSegment::dedent()).indented_on_contents()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DatabaseReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DatabaseReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "IndexReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::IndexReference,
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CollationReferenceSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CollationReference,
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]).config(
                        |this| {
                            this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                            this.terminators =
                                vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                            this.allow_gaps = false;
                        }
                    ),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "OverClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::OverClause,
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::new("IgnoreRespectNullsGrammar").optional(),
                    Ref::keyword("OVER"),
                    one_of(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Bracketed::new(vec_of_erased![
                            Ref::new("WindowSpecificationSegment").optional()
                        ])
                        .config(|this| this.parse_mode(ParseMode::Greedy))
                    ]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "NamedWindowSegment".into(),
            NodeMatcher::new(
                SyntaxKind::NamedWindow,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WINDOW"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Ref::new("NamedWindowExpressionSegment")]),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WindowSpecificationSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WindowSpecification,
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar")
                        .optional()
                        .exclude(Ref::keyword("PARTITION")),
                    Ref::new("PartitionClauseSegment").optional(),
                    Ref::new("OrderByClauseSegment").optional(),
                    Ref::new("FrameClauseSegment").optional()
                ])
                .config(|this| this.optional())
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "PartitionClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::PartitionByClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARTITION"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    optionally_bracketed(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ExpressionSegment"
                    )])]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "JoinClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::JoinClause,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("JoinTypeKeywordsGrammar").optional(),
                        Ref::new("JoinKeywordsGrammar"),
                        MetaSegment::indent(),
                        Ref::new("FromExpressionElementSegment"),
                        AnyNumberOf::new(vec_of_erased![Ref::new("NestedJoinGrammar")]),
                        MetaSegment::dedent(),
                        Sequence::new(vec_of_erased![
                            Conditional::new(MetaSegment::indent()).indented_using_on(),
                            one_of(vec_of_erased![
                                Ref::new("JoinOnConditionSegment"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("USING"),
                                    MetaSegment::indent(),
                                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                        Ref::new("SingleIdentifierGrammar")
                                    ])])
                                    .config(|this| this.parse_mode = ParseMode::Greedy),
                                    MetaSegment::dedent(),
                                ])
                            ]),
                            Conditional::new(MetaSegment::dedent()).indented_using_on(),
                        ])
                        .config(|this| this.optional())
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::new("NaturalJoinKeywordsGrammar"),
                        Ref::new("JoinKeywordsGrammar"),
                        MetaSegment::indent(),
                        Ref::new("FromExpressionElementSegment"),
                        MetaSegment::dedent(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::new("ExtendedNaturalJoinKeywordsGrammar"),
                        MetaSegment::indent(),
                        Ref::new("FromExpressionElementSegment"),
                        MetaSegment::dedent(),
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropTriggerStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropTriggerStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("TRIGGER"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TriggerReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SampleExpression,
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESAMPLE"),
                    one_of(vec_of_erased![Ref::keyword("BERNOULLI"), Ref::keyword("SYSTEM")]),
                    Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("REPEATABLE"),
                        Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TableExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TableExpression,
                one_of(vec_of_erased![
                    Ref::new("ValuesClauseSegment"),
                    Ref::new("BareFunctionSegment"),
                    Ref::new("FunctionSegment"),
                    Ref::new("TableReferenceSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
                    Bracketed::new(vec_of_erased![Ref::new("MergeStatementSegment")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropTriggerStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropTriggerStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("TRIGGER"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TriggerReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SampleExpression,
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESAMPLE"),
                    one_of(vec_of_erased![Ref::keyword("BERNOULLI"), Ref::keyword("SYSTEM")]),
                    Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("REPEATABLE"),
                        Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TableExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TableExpression,
                one_of(vec_of_erased![
                    Ref::new("ValuesClauseSegment"),
                    Ref::new("BareFunctionSegment"),
                    Ref::new("FunctionSegment"),
                    Ref::new("TableReferenceSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
                    Bracketed::new(vec_of_erased![Ref::new("MergeStatementSegment")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateTriggerStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateTriggerStatement,
                Sequence::new(vec![
                    Ref::keyword("CREATE").boxed(),
                    Ref::keyword("TRIGGER").boxed(),
                    Ref::new("TriggerReferenceSegment").boxed(),
                    one_of(vec![
                        Ref::keyword("BEFORE").boxed(),
                        Ref::keyword("AFTER").boxed(),
                        Sequence::new(vec![
                            Ref::keyword("INSTEAD").boxed(),
                            Ref::keyword("OF").boxed(),
                        ])
                        .boxed(),
                    ])
                    .config(|this| this.optional())
                    .boxed(),
                    Delimited::new(vec![
                        Ref::keyword("INSERT").boxed(),
                        Ref::keyword("DELETE").boxed(),
                        Sequence::new(vec![
                            Ref::keyword("UPDATE").boxed(),
                            Ref::keyword("OF").boxed(),
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").boxed()])
                                //.with_terminators(vec!["OR", "ON"])
                                .boxed(),
                        ])
                        .boxed(),
                    ])
                    .config(|this| {
                        this.delimiter(Ref::keyword("OR"));
                        // .with_terminators(vec!["ON"]);
                    })
                    .boxed(),
                    Ref::keyword("ON").boxed(),
                    Ref::new("TableReferenceSegment").boxed(),
                    AnyNumberOf::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("REFERENCING"),
                            Ref::keyword("OLD"),
                            Ref::keyword("ROW"),
                            Ref::keyword("AS"),
                            Ref::new("ParameterNameSegment"),
                            Ref::keyword("NEW"),
                            Ref::keyword("ROW"),
                            Ref::keyword("AS"),
                            Ref::new("ParameterNameSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::new("TableReferenceSegment"),
                        ]),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("NOT"),
                                Ref::keyword("DEFERRABLE"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DEFERRABLE").optional(),
                                one_of(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("INITIALLY"),
                                        Ref::keyword("IMMEDIATE"),
                                    ]),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("INITIALLY"),
                                        Ref::keyword("DEFERRED"),
                                    ]),
                                ]),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FOR"),
                            Ref::keyword("EACH").optional(),
                            one_of(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("STATEMENT"),]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WHEN"),
                            Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                        ]),
                    ])
                    .boxed(),
                    Sequence::new(vec![
                        Ref::keyword("EXECUTE").boxed(),
                        Ref::keyword("PROCEDURE").boxed(),
                        Ref::new("FunctionNameIdentifierSegment").boxed(),
                        Bracketed::new(vec![
                            Ref::new("FunctionContentsGrammar").optional().boxed(),
                        ])
                        .boxed(),
                    ])
                    .config(|this| this.optional())
                    .boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropModelStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropModelStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("MODEL"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DescribeStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DescribeStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DESCRIBE"),
                    Ref::new("NakedIdentifierSegment"),
                    Ref::new("ObjectReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "UseStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::UseStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("USE"),
                    Ref::new("DatabaseReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ExplainStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ExplainStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXPLAIN"),
                    one_of(vec_of_erased![
                        Ref::new("SelectableGrammar"),
                        Ref::new("InsertStatementSegment"),
                        Ref::new("UpdateStatementSegment"),
                        Ref::new("DeleteStatementSegment")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateSequenceStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateSequenceStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("SEQUENCE"),
                    Ref::new("SequenceReferenceSegment"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("CreateSequenceOptionsSegment")])
                        .config(|this| this.optional())
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateSequenceOptionsSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateSequenceOptionsSegment,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INCREMENT"),
                        Ref::keyword("BY"),
                        Ref::new("NumericLiteralSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("START"),
                        Ref::keyword("WITH").optional(),
                        Ref::new("NumericLiteralSegment")
                    ]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MINVALUE"),
                            Ref::new("NumericLiteralSegment")
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MINVALUE")])
                    ]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MAXVALUE"),
                            Ref::new("NumericLiteralSegment")
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MAXVALUE")])
                    ]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CACHE"),
                            Ref::new("NumericLiteralSegment")
                        ]),
                        Ref::keyword("NOCACHE")
                    ]),
                    one_of(vec_of_erased![Ref::keyword("CYCLE"), Ref::keyword("NOCYCLE")]),
                    one_of(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("NOORDER")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AlterSequenceStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AlterSequenceStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("SEQUENCE"),
                    Ref::new("SequenceReferenceSegment"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("AlterSequenceOptionsSegment")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropSequenceStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropSequenceStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("SEQUENCE"),
                    Ref::new("SequenceReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropCastStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropCastStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("CAST"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("DatatypeSegment"),
                        Ref::keyword("AS"),
                        Ref::new("DatatypeSegment")
                    ]),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateFunctionStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateFunctionStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::new("OrReplaceGrammar").optional(),
                    Ref::new("TemporaryGrammar").optional(),
                    Ref::keyword("FUNCTION"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("FunctionNameSegment"),
                    Ref::new("FunctionParameterListGrammar"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RETURNS"),
                        Ref::new("DatatypeSegment")
                    ])
                    .config(|this| this.optional()),
                    Ref::new("FunctionDefinitionGrammar")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropFunctionStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropFunctionStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("FUNCTION"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("FunctionNameSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateModelStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateModelStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::new("OrReplaceGrammar").optional(),
                    Ref::keyword("MODEL"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OPTIONS"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ParameterNameSegment"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::new("LiteralGrammar"), // Single value
                                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                        Ref::new("QuotedLiteralSegment")
                                    ])])
                                    .config(|this| {
                                        this.bracket_type("square");
                                        this.optional();
                                    })
                                ])
                            ])
                        ])])
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("AS"),
                    Ref::new("SelectableGrammar")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateViewStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateViewStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::new("OrReplaceGrammar").optional(),
                    Ref::keyword("VIEW"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("BracketedColumnReferenceListGrammar").optional(),
                    Ref::keyword("AS"),
                    optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]),
                    Ref::new("WithNoSchemaBindingClauseSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DeleteStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DELETE"),
                    Ref::new("FromClauseSegment"),
                    Ref::new("WhereClauseSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "UpdateStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::UpdateStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("UPDATE"),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("AliasExpressionSegment").exclude(Ref::keyword("SET")).optional(),
                    Ref::new("SetClauseListSegment"),
                    Ref::new("FromClauseSegment").optional(),
                    Ref::new("WhereClauseSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateCastStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateCastStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("CAST"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("DatatypeSegment"),
                        Ref::keyword("AS"),
                        Ref::new("DatatypeSegment")
                    ]),
                    Ref::keyword("WITH"),
                    Ref::keyword("SPECIFIC").optional(),
                    one_of(vec_of_erased![
                        Ref::keyword("ROUTINE"),
                        Ref::keyword("FUNCTION"),
                        Ref::keyword("PROCEDURE"),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("INSTANCE"),
                                Ref::keyword("STATIC"),
                                Ref::keyword("CONSTRUCTOR")
                            ])
                            .config(|this| this.optional()),
                            Ref::keyword("METHOD")
                        ])
                    ]),
                    Ref::new("FunctionNameSegment"),
                    Ref::new("FunctionParameterListGrammar").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOR"),
                        Ref::new("ObjectReferenceSegment")
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::keyword("ASSIGNMENT")])
                        .config(|this| this.optional())
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateRoleStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateRoleStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("ROLE"),
                    Ref::new("RoleReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropRoleStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropRoleStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("ROLE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AlterTableStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AlterTableStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Delimited::new(vec_of_erased![Ref::new("AlterTableOptionsGrammar")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SetSchemaStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SetSchemaStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("SchemaReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropSchemaStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropSchemaStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SchemaReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropTypeStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropTypeStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("TYPE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateDatabaseStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateDatabaseStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("DATABASE"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("DatabaseReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropDatabaseStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropDatabaseStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("DATABASE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("DatabaseReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FunctionParameterListGrammar".into(),
            NodeMatcher::new(
                SyntaxKind::FunctionParameterList,
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("FunctionParameterGrammar")])
                        .config(|this| this.optional())
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateIndexStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateIndexStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::new("OrReplaceGrammar").optional(),
                    Ref::keyword("UNIQUE").optional(),
                    Ref::keyword("INDEX"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("IndexReferenceSegment"),
                    Ref::keyword("ON"),
                    Ref::new("TableReferenceSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "IndexColumnDefinitionSegment"
                    )])])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropIndexStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropIndexStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("INDEX"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("IndexReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateTableStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateTableStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::new("OrReplaceGrammar").optional(),
                    Ref::new("TemporaryTransientGrammar").optional(),
                    Ref::keyword("TABLE"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    one_of(vec_of_erased![
                        // Columns and comment syntax
                        Sequence::new(vec_of_erased![
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                                vec_of_erased![
                                    Ref::new("TableConstraintSegment"),
                                    Ref::new("ColumnDefinitionSegment")
                                ]
                            )])]),
                            Ref::new("CommentClauseSegment").optional()
                        ]),
                        // Create AS syntax:
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AS"),
                            optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
                        ]),
                        // Create LIKE syntax
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LIKE"),
                            Ref::new("TableReferenceSegment")
                        ])
                    ]),
                    Ref::new("TableEndClauseSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AccessStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AccessStatement,
                {
                    let global_permissions = one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CREATE"),
                            one_of(vec_of_erased![
                                Ref::keyword("ROLE"),
                                Ref::keyword("USER"),
                                Ref::keyword("WAREHOUSE"),
                                Ref::keyword("DATABASE"),
                                Ref::keyword("INTEGRATION"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("APPLY"),
                            Ref::keyword("MASKING"),
                            Ref::keyword("POLICY"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("EXECUTE"),
                            Ref::keyword("TASK")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MANAGE"),
                            Ref::keyword("GRANTS")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MONITOR"),
                            one_of(vec_of_erased![
                                Ref::keyword("EXECUTION"),
                                Ref::keyword("USAGE")
                            ]),
                        ]),
                    ]);

                    let schema_object_types = one_of(vec_of_erased![
                        Ref::keyword("TABLE"),
                        Ref::keyword("VIEW"),
                        Ref::keyword("STAGE"),
                        Ref::keyword("FUNCTION"),
                        Ref::keyword("PROCEDURE"),
                        Ref::keyword("ROUTINE"),
                        Ref::keyword("SEQUENCE"),
                        Ref::keyword("STREAM"),
                        Ref::keyword("TASK"),
                    ]);

                    let permissions = Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CREATE"),
                                one_of(vec_of_erased![
                                    Ref::keyword("SCHEMA"),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("MASKING"),
                                        Ref::keyword("POLICY"),
                                    ]),
                                    Ref::keyword("PIPE"),
                                    schema_object_types.clone(),
                                ]),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("IMPORTED"),
                                Ref::keyword("PRIVILEGES")
                            ]),
                            Ref::keyword("APPLY"),
                            Ref::keyword("CONNECT"),
                            Ref::keyword("CREATE"),
                            Ref::keyword("DELETE"),
                            Ref::keyword("EXECUTE"),
                            Ref::keyword("INSERT"),
                            Ref::keyword("MODIFY"),
                            Ref::keyword("MONITOR"),
                            Ref::keyword("OPERATE"),
                            Ref::keyword("OWNERSHIP"),
                            Ref::keyword("READ"),
                            Ref::keyword("REFERENCE_USAGE"),
                            Ref::keyword("REFERENCES"),
                            Ref::keyword("SELECT"),
                            Ref::keyword("TEMP"),
                            Ref::keyword("TEMPORARY"),
                            Ref::keyword("TRIGGER"),
                            Ref::keyword("TRUNCATE"),
                            Ref::keyword("UPDATE"),
                            Ref::keyword("USAGE"),
                            Ref::keyword("USE_ANY_ROLE"),
                            Ref::keyword("WRITE"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ALL"),
                                Ref::keyword("PRIVILEGES").optional(),
                            ]),
                        ]),
                        Ref::new("BracketedColumnReferenceListGrammar").optional(),
                    ]);

                    let objects = one_of(vec_of_erased![
                        Ref::keyword("ACCOUNT"),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("RESOURCE"),
                                    Ref::keyword("MONITOR"),
                                ]),
                                Ref::keyword("WAREHOUSE"),
                                Ref::keyword("DATABASE"),
                                Ref::keyword("DOMAIN"),
                                Ref::keyword("INTEGRATION"),
                                Ref::keyword("LANGUAGE"),
                                Ref::keyword("SCHEMA"),
                                Ref::keyword("ROLE"),
                                Ref::keyword("TABLESPACE"),
                                Ref::keyword("TYPE"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("FOREIGN"),
                                    one_of(vec_of_erased![
                                        Ref::keyword("SERVER"),
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("DATA"),
                                            Ref::keyword("WRAPPER"),
                                        ]),
                                    ]),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ALL"),
                                    Ref::keyword("SCHEMAS"),
                                    Ref::keyword("IN"),
                                    Ref::keyword("DATABASE"),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("FUTURE"),
                                    Ref::keyword("SCHEMAS"),
                                    Ref::keyword("IN"),
                                    Ref::keyword("DATABASE"),
                                ]),
                                schema_object_types.clone(),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ALL"),
                                    one_of(vec_of_erased![
                                        Ref::keyword("TABLES"),
                                        Ref::keyword("VIEWS"),
                                        Ref::keyword("STAGES"),
                                        Ref::keyword("FUNCTIONS"),
                                        Ref::keyword("PROCEDURES"),
                                        Ref::keyword("ROUTINES"),
                                        Ref::keyword("SEQUENCES"),
                                        Ref::keyword("STREAMS"),
                                        Ref::keyword("TASKS"),
                                    ]),
                                    Ref::keyword("IN"),
                                    Ref::keyword("SCHEMA"),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("FUTURE"),
                                    Ref::keyword("IN"),
                                    one_of(vec_of_erased![
                                        Ref::keyword("DATABASE"),
                                        Ref::keyword("SCHEMA")
                                    ]),
                                ]),
                            ])
                            .config(|this| this.optional()),
                            Delimited::new(vec_of_erased![
                                Ref::new("ObjectReferenceSegment"),
                                Sequence::new(vec_of_erased![
                                    Ref::new("FunctionNameSegment"),
                                    Ref::new("FunctionParameterListGrammar").optional(),
                                ]),
                            ])
                            .config(|this| this.terminators =
                                vec_of_erased![Ref::keyword("TO"), Ref::keyword("FROM")]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LARGE"),
                            Ref::keyword("OBJECT"),
                            Ref::new("NumericLiteralSegment"),
                        ]),
                    ]);

                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("GRANT"),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                                        global_permissions.clone(),
                                        permissions.clone()
                                    ])])
                                    .config(|this| this.terminators =
                                        vec_of_erased![Ref::keyword("ON")]),
                                    Ref::keyword("ON"),
                                    objects.clone()
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ROLE"),
                                    Ref::new("ObjectReferenceSegment")
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("OWNERSHIP"),
                                    Ref::keyword("ON"),
                                    Ref::keyword("USER"),
                                    Ref::new("ObjectReferenceSegment"),
                                ]),
                                Ref::new("ObjectReferenceSegment")
                            ]),
                            Ref::keyword("TO"),
                            one_of(vec_of_erased![
                                Ref::keyword("GROUP"),
                                Ref::keyword("USER"),
                                Ref::keyword("ROLE"),
                                Ref::keyword("SHARE")
                            ])
                            .config(|this| this.optional()),
                            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                                Ref::new("RoleReferenceSegment"),
                                Ref::new("FunctionSegment"),
                                Ref::keyword("PUBLIC")
                            ])]),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("WITH"),
                                    Ref::keyword("GRANT"),
                                    Ref::keyword("OPTION"),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("WITH"),
                                    Ref::keyword("ADMIN"),
                                    Ref::keyword("OPTION"),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("COPY"),
                                    Ref::keyword("CURRENT"),
                                    Ref::keyword("GRANTS"),
                                ])
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("GRANTED"),
                                Ref::keyword("BY"),
                                one_of(vec_of_erased![
                                    Ref::keyword("CURRENT_USER"),
                                    Ref::keyword("SESSION_USER"),
                                    Ref::new("ObjectReferenceSegment")
                                ])
                            ])
                            .config(|this| this.optional())
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("REVOKE"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("GRANT"),
                                Ref::keyword("OPTION"),
                                Ref::keyword("FOR")
                            ])
                            .config(|this| this.optional()),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Delimited::new(vec_of_erased![
                                        one_of(vec_of_erased![global_permissions, permissions])
                                            .config(|this| this.terminators =
                                                vec_of_erased![Ref::keyword("ON")])
                                    ]),
                                    Ref::keyword("ON"),
                                    objects
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ROLE"),
                                    Ref::new("ObjectReferenceSegment")
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("OWNERSHIP"),
                                    Ref::keyword("ON"),
                                    Ref::keyword("USER"),
                                    Ref::new("ObjectReferenceSegment"),
                                ]),
                                Ref::new("ObjectReferenceSegment"),
                            ]),
                            Ref::keyword("FROM"),
                            one_of(vec_of_erased![
                                Ref::keyword("GROUP"),
                                Ref::keyword("USER"),
                                Ref::keyword("ROLE"),
                                Ref::keyword("SHARE")
                            ])
                            .config(|this| this.optional()),
                            Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment")]),
                            Ref::new("DropBehaviorGrammar").optional()
                        ])
                    ])
                }
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::InsertStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("OVERWRITE").optional(),
                    Ref::keyword("INTO"),
                    Ref::new("TableReferenceSegment"),
                    one_of(vec_of_erased![
                        Ref::new("SelectableGrammar"),
                        Sequence::new(vec_of_erased![
                            Ref::new("BracketedColumnReferenceListGrammar"),
                            Ref::new("SelectableGrammar")
                        ]),
                        Ref::new("DefaultValuesGrammar")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TransactionStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TransactionStatement,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("START"),
                        Ref::keyword("BEGIN"),
                        Ref::keyword("COMMIT"),
                        Ref::keyword("ROLLBACK"),
                        Ref::keyword("END")
                    ]),
                    one_of(vec_of_erased![Ref::keyword("TRANSACTION"), Ref::keyword("WORK")])
                        .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NAME"),
                        Ref::new("SingleIdentifierGrammar")
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AND"),
                        Ref::keyword("NO").optional(),
                        Ref::keyword("CHAIN")
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropTableStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropTableStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::new("TemporaryGrammar").optional(),
                    Ref::keyword("TABLE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment")]),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropViewStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropViewStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("VIEW"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateUserStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateUserStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("USER"),
                    Ref::new("RoleReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropUserStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropUserStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("USER"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("RoleReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "NotEqualToSegment".into(),
            NodeMatcher::new(
                SyntaxKind::NotEqualTo,
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("RawNotSegment").boxed(),
                        Ref::new("RawEqualsSegment").boxed(),
                    ])
                    .allow_gaps(false)
                    .boxed(),
                    Sequence::new(vec![
                        Ref::new("RawLessThanSegment").boxed(),
                        Ref::new("RawGreaterThanSegment").boxed(),
                    ])
                    .allow_gaps(false)
                    .boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ConcatSegment".into(),
            NodeMatcher::new(
                SyntaxKind::Concat,
                Sequence::new(vec![
                    Ref::new("PipeSegment").boxed(),
                    Ref::new("PipeSegment").boxed(),
                ])
                .allow_gaps(false)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ArrayExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::ArrayExpression, Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "LocalAliasSegment".into(),
            NodeMatcher::new(SyntaxKind::LocalAlias, Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "MergeStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeStatement,
                Sequence::new(vec![
                    Ref::new("MergeIntoLiteralGrammar").boxed(),
                    MetaSegment::indent().boxed(),
                    one_of(vec![
                        Ref::new("TableReferenceSegment").boxed(),
                        Ref::new("AliasedTableReferenceGrammar").boxed(),
                    ])
                    .boxed(),
                    MetaSegment::dedent().boxed(),
                    Ref::keyword("USING").boxed(),
                    MetaSegment::indent().boxed(),
                    one_of(vec![
                        Ref::new("TableReferenceSegment").boxed(),
                        Ref::new("AliasedTableReferenceGrammar").boxed(),
                        Sequence::new(vec![
                            Bracketed::new(vec![Ref::new("SelectableGrammar").boxed()]).boxed(),
                            Ref::new("AliasExpressionSegment").optional().boxed(),
                        ])
                        .boxed(),
                    ])
                    .boxed(),
                    MetaSegment::dedent().boxed(),
                    Conditional::new(MetaSegment::indent()).indented_using_on().boxed(),
                    Ref::new("JoinOnConditionSegment").boxed(),
                    Conditional::new(MetaSegment::dedent()).indented_using_on().boxed(),
                    Ref::new("MergeMatchSegment").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "IndexColumnDefinitionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::IndexColumnDefinition,
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").boxed(), // Column name
                    one_of(vec![Ref::keyword("ASC").boxed(), Ref::keyword("DESC").boxed()])
                        .config(|this| this.optional())
                        .boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "BitwiseAndSegment".into(),
            NodeMatcher::new(SyntaxKind::BitwiseAnd, Ref::new("AmpersandSegment").to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "BitwiseOrSegment".into(),
            NodeMatcher::new(SyntaxKind::BitwiseOr, Ref::new("PipeSegment").to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "BitwiseLShiftSegment".into(),
            NodeMatcher::new(
                SyntaxKind::BitwiseLShift,
                Sequence::new(vec![
                    Ref::new("RawLessThanSegment").boxed(),
                    Ref::new("RawLessThanSegment").boxed(),
                ])
                .allow_gaps(false)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "BitwiseRShiftSegment".into(),
            NodeMatcher::new(
                SyntaxKind::BitwiseRShift,
                Sequence::new(vec![
                    Ref::new("RawGreaterThanSegment").boxed(),
                    Ref::new("RawGreaterThanSegment").boxed(),
                ])
                .allow_gaps(false)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "LessThanSegment".into(),
            NodeMatcher::new(SyntaxKind::LessThan, Ref::new("RawLessThanSegment").to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "GreaterThanOrEqualToSegment".into(),
            NodeMatcher::new(
                SyntaxKind::GreaterThanOrEqualTo,
                Sequence::new(vec![
                    Ref::new("RawGreaterThanSegment").boxed(),
                    Ref::new("RawEqualsSegment").boxed(),
                ])
                .allow_gaps(false)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "LessThanOrEqualToSegment".into(),
            NodeMatcher::new(
                SyntaxKind::LessThanOrEqualTo,
                Sequence::new(vec![
                    Ref::new("RawLessThanSegment").boxed(),
                    Ref::new("RawEqualsSegment").boxed(),
                ])
                .allow_gaps(false)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "EqualsSegment".into(),
            NodeMatcher::new(SyntaxKind::Equals, Ref::new("RawEqualsSegment").to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "GreaterThanSegment".into(),
            NodeMatcher::new(
                SyntaxKind::GreaterThan,
                Ref::new("RawGreaterThanSegment").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "QualifiedNumericLiteralSegment".into(),
            NodeMatcher::new(
                SyntaxKind::QualifiedNumericLiteral,
                Sequence::new(vec![
                    Ref::new("SignedSegmentGrammar").boxed(),
                    Ref::new("NumericLiteralSegment").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AggregateOrderByClause".into(),
            NodeMatcher::new(
                SyntaxKind::AggregateOrderByClause,
                Ref::new("OrderByClauseSegment").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FunctionNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FunctionName,
                Sequence::new(vec_of_erased![
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("DotSegment")
                    ])])
                    .config(|this| this.terminators = vec_of_erased![Ref::new("BracketedSegment")]),
                    one_of(vec_of_erased![
                        Ref::new("FunctionNameIdentifierSegment"),
                        Ref::new("QuotedIdentifierSegment")
                    ])
                ])
                .terminators(vec_of_erased![Ref::new("BracketedSegment")])
                .allow_gaps(false)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CaseExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CaseExpression,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CASE"),
                        MetaSegment::implicit_indent(),
                        AnyNumberOf::new(vec_of_erased![Ref::new("WhenClauseSegment")],).config(
                            |this| {
                                this.terminators =
                                    vec_of_erased![Ref::keyword("ELSE"), Ref::keyword("END")];
                            }
                        ),
                        Ref::new("ElseClauseSegment").optional(),
                        MetaSegment::dedent(),
                        Ref::keyword("END"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CASE"),
                        Ref::new("ExpressionSegment"),
                        MetaSegment::implicit_indent(),
                        AnyNumberOf::new(vec_of_erased![Ref::new("WhenClauseSegment")],).config(
                            |this| {
                                this.terminators =
                                    vec_of_erased![Ref::keyword("ELSE"), Ref::keyword("END")];
                            }
                        ),
                        Ref::new("ElseClauseSegment").optional(),
                        MetaSegment::dedent(),
                        Ref::keyword("END"),
                    ]),
                ])
                .config(|this| {
                    this.terminators =
                        vec_of_erased![Ref::new("CommaSegment"), Ref::new("BinaryOperatorGrammar")]
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WhenClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WhenClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHEN"),
                    Sequence::new(vec_of_erased![
                        MetaSegment::implicit_indent(),
                        Ref::new("ExpressionSegment"),
                        MetaSegment::dedent(),
                    ]),
                    Conditional::new(MetaSegment::indent()).indented_then(),
                    Ref::keyword("THEN"),
                    Conditional::new(MetaSegment::implicit_indent()).indented_then_contents(),
                    Ref::new("ExpressionSegment"),
                    Conditional::new(MetaSegment::dedent()).indented_then_contents(),
                    Conditional::new(MetaSegment::dedent()).indented_then(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ElseClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ElseClause,
                Sequence::new(vec![
                    Ref::keyword("ELSE").boxed(),
                    Ref::new("ExpressionSegment").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WhereClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHERE"),
                    MetaSegment::implicit_indent(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SetOperatorSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SetOperator,
                one_of(vec_of_erased![
                    Ref::new("UnionGrammar"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("INTERSECT"), Ref::keyword("EXCEPT")]),
                        Ref::keyword("ALL").optional(),
                    ]),
                    Ref::keyword("MINUS"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ValuesClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ValuesClause,
                Sequence::new(vec![
                    one_of(vec![Ref::keyword("VALUE").boxed(), Ref::keyword("VALUES").boxed()])
                        .boxed(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("ROW").optional().boxed(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::keyword("DEFAULT").boxed(),
                                    Ref::new("LiteralGrammar").boxed(),
                                    Ref::new("ExpressionSegment").boxed(),
                                ])
                                .boxed(),
                            ])
                            .config(|this| this.parse_mode(ParseMode::Greedy))
                            .boxed(),
                        ])
                        .boxed(),
                    ])
                    .boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "EmptyStructLiteralSegment".into(),
            NodeMatcher::new(
                SyntaxKind::EmptyStructLiteral,
                Sequence::new(vec![
                    Ref::new("StructTypeSegment").boxed(),
                    Ref::new("EmptyStructLiteralBracketsSegment").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ObjectLiteralSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ObjectLiteral,
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ObjectLiteralElementSegment").boxed()])
                        .config(|this| {
                            this.optional();
                        })
                        .boxed(),
                ])
                .config(|this| {
                    this.bracket_type("curly");
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ObjectLiteralElementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ObjectLiteralElement,
                Sequence::new(vec![
                    Ref::new("QuotedLiteralSegment").boxed(),
                    Ref::new("ColonSegment").boxed(),
                    Ref::new("BaseExpressionElementGrammar").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TimeZoneGrammar".into(),
            NodeMatcher::new(
                SyntaxKind::TimeZoneGrammar,
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("AT").boxed(),
                        Ref::keyword("TIME").boxed(),
                        Ref::keyword("ZONE").boxed(),
                        Ref::new("ExpressionSegment").boxed(),
                    ])
                    .boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "BracketedArguments".into(),
            NodeMatcher::new(
                SyntaxKind::BracketedArguments,
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("LiteralGrammar").boxed()])
                        .config(|this| {
                            this.optional();
                        })
                        .boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DataType,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("TIME"), Ref::keyword("TIMESTAMP")]),
                        Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")])
                            .config(|this| this.optional()),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("WITHOUT")]),
                            Ref::keyword("TIME"),
                            Ref::keyword("ZONE"),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DOUBLE"),
                        Ref::keyword("PRECISION")
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![
                                    Ref::keyword("CHARACTER"),
                                    Ref::keyword("BINARY")
                                ]),
                                one_of(vec_of_erased![
                                    Ref::keyword("VARYING"),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("LARGE"),
                                        Ref::keyword("OBJECT"),
                                    ]),
                                ]),
                            ]),
                            Sequence::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::new("SingleIdentifierGrammar"),
                                    Ref::new("DotSegment"),
                                ])
                                .config(|this| this.optional()),
                                Ref::new("DatatypeIdentifierSegment"),
                            ]),
                        ]),
                        Ref::new("BracketedArguments").optional(),
                        one_of(vec_of_erased![
                            Ref::keyword("UNSIGNED"),
                            Ref::new("CharCharacterSetGrammar"),
                        ])
                        .config(|config| config.optional()),
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AliasExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AliasExpression,
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::keyword("AS").optional(),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
                                .config(|this| this.optional())
                        ]),
                        Ref::new("SingleQuotedIdentifierSegment")
                    ]),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ShorthandCastSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ShorthandCast,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("Expression_D_Grammar"),
                        Ref::new("CaseExpressionSegment")
                    ]),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("CastOperatorSegment"),
                        Ref::new("DatatypeSegment"),
                        Ref::new("TimeZoneGrammar").optional()
                    ]),])
                    .config(|this| this.min_times(1)),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ArrayAccessorSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ArrayAccessor,
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").boxed(),
                            Ref::new("ExpressionSegment").boxed(),
                        ])
                        .boxed(),
                    ])
                    .config(|this| this.delimiter(Ref::new("SliceSegment")))
                    .boxed(),
                ])
                .config(|this| {
                    this.bracket_type("square");
                    this.parse_mode(ParseMode::Greedy);
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ArrayLiteralSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ArrayLiteral,
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("BaseExpressionElementGrammar")])
                        .config(|this| {
                            this.delimiter(Ref::new("CommaSegment"));
                            this.optional();
                        }),
                ])
                .config(|this| {
                    this.bracket_type("square");
                    this.parse_mode(ParseMode::Greedy);
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TypedArrayLiteralSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TypedArrayLiteral,
                Sequence::new(vec![
                    Ref::new("ArrayTypeSegment").boxed(),
                    Ref::new("ArrayLiteralSegment").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "StructTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::StructType, Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "StructLiteralSegment".into(),
            NodeMatcher::new(
                SyntaxKind::StructLiteral,
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("BaseExpressionElementGrammar"),
                        Ref::new("AliasExpressionSegment").optional(),
                    ]
                )])])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TypedStructLiteralSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TypedStructLiteral,
                Sequence::new(vec![
                    Ref::new("StructTypeSegment").boxed(),
                    Ref::new("StructLiteralSegment").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "IntervalExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::IntervalExpression,
                Sequence::new(vec![
                    Ref::keyword("INTERVAL").boxed(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("NumericLiteralSegment").boxed(),
                            one_of(vec![
                                Ref::new("QuotedLiteralSegment").boxed(),
                                Ref::new("DatetimeUnitSegment").boxed(),
                            ])
                            .boxed(),
                        ])
                        .boxed(),
                        Ref::new("QuotedLiteralSegment").boxed(),
                    ])
                    .boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ArrayTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::ArrayType, Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "SizedArrayTypeSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SizedArrayType,
                Sequence::new(vec![
                    Ref::new("ArrayTypeSegment").boxed(),
                    Ref::new("ArrayAccessorSegment").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "UnorderedSelectStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SelectStatement,
                Sequence::new(vec_of_erased![
                    Ref::new("SelectClauseSegment"),
                    MetaSegment::dedent(),
                    Ref::new("FromClauseSegment").optional(),
                    Ref::new("WhereClauseSegment").optional(),
                    Ref::new("GroupByClauseSegment").optional(),
                    Ref::new("HavingClauseSegment").optional(),
                    Ref::new("OverlapsClauseSegment").optional(),
                    Ref::new("NamedWindowSegment").optional()
                ])
                .terminators(vec_of_erased![
                    Ref::new("SetOperatorSegment"),
                    Ref::new("WithNoSchemaBindingClauseSegment"),
                    Ref::new("WithDataClauseSegment"),
                    Ref::new("OrderByClauseSegment"),
                    Ref::new("LimitClauseSegment")
                ])
                .config(|this| {
                    this.parse_mode(ParseMode::GreedyOnceStarted);
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "OverlapsClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::OverlapsClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("OVERLAPS"),
                    one_of(vec_of_erased![
                        Bracketed::new(vec_of_erased![
                            Ref::new("DateTimeLiteralGrammar"),
                            Ref::new("CommaSegment"),
                            Ref::new("DateTimeLiteralGrammar"),
                        ]),
                        Ref::new("ColumnReferenceSegment"),
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        ("SelectClauseSegment".into(), {
            NodeMatcher::new(SyntaxKind::SelectClause, select_clause_segment())
                .to_matchable()
                .into()
        }),
        (
            "StatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, statement_segment()).to_matchable().into(),
        ),
        (
            "WithNoSchemaBindingClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WithNoSchemaBindingClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("NO"),
                    Ref::keyword("SCHEMA"),
                    Ref::keyword("BINDING"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WithDataClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WithDataClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Sequence::new(vec_of_erased![Ref::keyword("NO")])
                        .config(|this| this.optional()),
                    Ref::keyword("DATA"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SetExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SetExpression,
                Sequence::new(vec_of_erased![
                    Ref::new("NonSetSelectableGrammar"),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("SetOperatorSegment"),
                        Ref::new("NonSetSelectableGrammar"),
                    ])])
                    .config(|this| this.min_times(1)),
                    Ref::new("OrderByClauseSegment").optional(),
                    Ref::new("LimitClauseSegment").optional(),
                    Ref::new("NamedWindowSegment").optional(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FromClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FromClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("FROM"),
                    Delimited::new(vec_of_erased![Ref::new("FromExpressionSegment")]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "EmptyStructLiteralBracketsSegment".into(),
            NodeMatcher::new(
                SyntaxKind::EmptyStructLiteralBrackets,
                Bracketed::new(vec![]).to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WildcardExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::WildcardExpression, wildcard_expression_segment())
                .to_matchable()
                .into(),
        ),
        (
            "OrderByClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::OrderByClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ORDER"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("ExpressionSegment"),
                        ]),
                        one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC"),])
                            .config(|this| this.optional()),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NULLS"),
                            one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST"),]),
                        ])
                        .config(|this| this.optional()),
                    ])])
                    .config(|this| this.terminators =
                        vec_of_erased![Ref::keyword("LIMIT"), Ref::new("FrameClauseUnitGrammar")]),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TruncateStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TruncateStatement,
                Sequence::new(vec![
                    Ref::keyword("TRUNCATE").boxed(),
                    Ref::keyword("TABLE").optional().boxed(),
                    Ref::new("TableReferenceSegment").boxed(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FromExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FromExpression,
                optionally_bracketed(vec_of_erased![Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    one_of(vec_of_erased![
                        Ref::new("FromExpressionElementSegment"),
                        Bracketed::new(vec_of_erased![Ref::new("FromExpressionSegment")])
                    ])
                    .config(|this| this.terminators = vec_of_erased![
                        Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                        Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY")]),
                    ]),
                    MetaSegment::dedent(),
                    Conditional::new(MetaSegment::indent()).indented_joins(),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("JoinClauseSegment"),
                            Ref::new("JoinLikeClauseGrammar")
                        ])
                        .config(|this| {
                            this.optional();
                            this.terminators = vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ORDER"),
                                    Ref::keyword("BY")
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("GROUP"),
                                    Ref::keyword("BY")
                                ]),
                            ];
                        })
                    ])]),
                    Conditional::new(MetaSegment::dedent()).indented_joins(),
                ])])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DatePartFunctionNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FunctionName,
                Ref::new("DatePartFunctionName").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FromExpressionElementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FromExpressionElement,
                Sequence::new(vec_of_erased![
                    Ref::new("PreTableFunctionKeywordsGrammar").optional(),
                    optionally_bracketed(vec_of_erased![Ref::new("TableExpressionSegment")]),
                    Ref::new("AliasExpressionSegment")
                        .exclude(one_of(vec_of_erased![
                            Ref::new("FromClauseTerminatorGrammar"),
                            Ref::new("SamplingExpressionSegment"),
                            Ref::new("JoinLikeClauseGrammar")
                        ]))
                        .optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("OFFSET"),
                        Ref::new("AliasExpressionSegment")
                    ])
                    .config(|this| this.optional()),
                    Ref::new("SamplingExpressionSegment").optional(),
                    Ref::new("PostTableExpressionGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, select_statement()).to_matchable().into(),
        ),
        (
            "CreateSchemaStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateSchemaStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("SchemaReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseModifierSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SelectClauseModifier,
                one_of(vec![Ref::keyword("DISTINCT").boxed(), Ref::keyword("ALL").boxed()])
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseElementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClauseElement, select_clause_element())
                .to_matchable()
                .into(),
        ),
    ]);

    // hookpoint
    ansi_dialect.add([("CharCharacterSetGrammar".into(), Nothing::new().to_matchable().into())]);

    // This is a hook point to allow subclassing for other dialects
    ansi_dialect.add([(
        "AliasedTableReferenceGrammar".into(),
        Sequence::new(vec![
            Ref::new("TableReferenceSegment").boxed(),
            Ref::new("AliasExpressionSegment").boxed(),
        ])
        .to_matchable()
        .into(),
    )]);

    ansi_dialect.add([
        // FunctionContentsExpressionGrammar intended as a hook to override in other dialects.
        (
            "FunctionContentsExpressionGrammar".into(),
            Ref::new("ExpressionSegment").to_matchable().into(),
        ),
        (
            "FunctionContentsGrammar".into(),
            AnyNumberOf::new(vec![
                Ref::new("ExpressionSegment").boxed(),
                // A Cast-like function
                Sequence::new(vec![
                    Ref::new("ExpressionSegment").boxed(),
                    Ref::keyword("AS").boxed(),
                    Ref::new("DatatypeSegment").boxed(),
                ])
                .boxed(),
                // Trim function
                Sequence::new(vec![
                    Ref::new("TrimParametersGrammar").boxed(),
                    Ref::new("ExpressionSegment").optional().exclude(Ref::keyword("FROM")).boxed(),
                    Ref::keyword("FROM").boxed(),
                    Ref::new("ExpressionSegment").boxed(),
                ])
                .boxed(),
                // An extract-like or substring-like function
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("DatetimeUnitSegment").boxed(),
                        Ref::new("ExpressionSegment").boxed(),
                    ])
                    .boxed(),
                    Ref::keyword("FROM").boxed(),
                    Ref::new("ExpressionSegment").boxed(),
                ])
                .boxed(),
                Sequence::new(vec![
                    // Allow an optional distinct keyword here.
                    Ref::keyword("DISTINCT").optional().boxed(),
                    one_of(vec![
                        // For COUNT(*) or similar
                        Ref::new("StarSegment").boxed(),
                        Delimited::new(vec![Ref::new("FunctionContentsExpressionGrammar").boxed()])
                            .boxed(),
                    ])
                    .boxed(),
                ])
                .boxed(),
                Ref::new("AggregateOrderByClause").boxed(), // Used in various functions
                Sequence::new(vec![
                    Ref::keyword("SEPARATOR").boxed(),
                    Ref::new("LiteralGrammar").boxed(),
                ])
                .boxed(),
                // Position-like function
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").boxed(),
                        Ref::new("SingleIdentifierGrammar").boxed(),
                        Ref::new("ColumnReferenceSegment").boxed(),
                    ])
                    .boxed(),
                    Ref::keyword("IN").boxed(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").boxed(),
                        Ref::new("SingleIdentifierGrammar").boxed(),
                        Ref::new("ColumnReferenceSegment").boxed(),
                    ])
                    .boxed(),
                ])
                .boxed(),
                Ref::new("IgnoreRespectNullsGrammar").boxed(),
                Ref::new("IndexColumnDefinitionSegment").boxed(),
                Ref::new("EmptyStructLiteralSegment").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PostFunctionGrammar".into(),
            one_of(vec![
                Ref::new("OverClauseSegment").boxed(),
                Ref::new("FilterClauseGrammar").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Assuming `ansi_dialect` is an instance of a struct representing a SQL dialect
    // and `add_grammar` is a method to add a new grammar rule to the dialect.
    ansi_dialect.add([("JoinLikeClauseGrammar".into(), Nothing::new().to_matchable().into())]);

    ansi_dialect.add([
        (
            // Expression_A_Grammar
            // https://www.cockroachlabs.com/docs/v20.2/sql-grammar.html#a_expr
            // The upstream grammar is defined recursively, which if implemented naively
            // will cause SQLFluff to overflow the stack from recursive function calls.
            // To work around this, the a_expr grammar is reworked a bit into sub-grammars
            // that effectively provide tail recursion.
            "Expression_A_Unary_Operator_Grammar".into(),
            one_of(vec![
                // This grammar corresponds to the unary operator portion of the initial
                // recursive block on the Cockroach Labs a_expr grammar.
                Ref::new("SignedSegmentGrammar")
                    .exclude(Sequence::new(vec![
                        Ref::new("QualifiedNumericLiteralSegment").boxed(),
                    ]))
                    .boxed(),
                Ref::new("TildeSegment").boxed(),
                Ref::new("NotOperatorGrammar").boxed(),
                // Used in CONNECT BY clauses (EXASOL, Snowflake, Postgres...)
                Ref::keyword("PRIOR").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Tail_Recurse_Expression_A_Grammar".into(),
            Sequence::new(vec_of_erased![
                // This should be used instead of a recursive call to Expression_A_Grammar
                // whenever the repeating element in Expression_A_Grammar makes a recursive
                // call to itself at the _end_.
                AnyNumberOf::new(vec_of_erased![Ref::new("Expression_A_Unary_Operator_Grammar")])
                    .config(
                        |this| this.terminators = vec_of_erased![Ref::new("BinaryOperatorGrammar")]
                    ),
                Ref::new("Expression_C_Grammar"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_A_Grammar".into(),
            Sequence::new(vec![
                Ref::new("Tail_Recurse_Expression_A_Grammar").boxed(),
                AnyNumberOf::new(vec![
                    one_of(vec![
                        // Like grammar with NOT and optional ESCAPE
                        Sequence::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("NOT").optional().boxed(),
                                Ref::new("LikeGrammar").boxed(),
                            ])
                            .boxed(),
                            Ref::new("Expression_A_Grammar").boxed(),
                            Sequence::new(vec![
                                Ref::keyword("ESCAPE").boxed(),
                                Ref::new("Tail_Recurse_Expression_A_Grammar").boxed(),
                            ])
                            .config(|this| this.optional())
                            .boxed(),
                        ])
                        .boxed(),
                        // Binary operator grammar
                        Sequence::new(vec![
                            Ref::new("BinaryOperatorGrammar").boxed(),
                            Ref::new("Tail_Recurse_Expression_A_Grammar").boxed(),
                        ])
                        .boxed(),
                        // IN grammar with NOT and brackets
                        Sequence::new(vec![
                            Ref::keyword("NOT").optional().boxed(),
                            Ref::keyword("IN").boxed(),
                            Bracketed::new(vec![
                                one_of(vec![
                                    Delimited::new(vec![Ref::new("Expression_A_Grammar").boxed()])
                                        .boxed(),
                                    Ref::new("SelectableGrammar").boxed(),
                                ])
                                .boxed(),
                            ])
                            .config(|this| this.parse_mode(ParseMode::Greedy))
                            .boxed(),
                        ])
                        .boxed(),
                        // IN grammar with function segment
                        Sequence::new(vec![
                            Ref::keyword("NOT").optional().boxed(),
                            Ref::keyword("IN").boxed(),
                            Ref::new("FunctionSegment").boxed(),
                        ])
                        .boxed(),
                        // IS grammar
                        Sequence::new(vec![
                            Ref::keyword("IS").boxed(),
                            Ref::keyword("NOT").optional().boxed(),
                            Ref::new("IsClauseGrammar").boxed(),
                        ])
                        .boxed(),
                        // IS NULL and NOT NULL grammars
                        Ref::new("IsNullGrammar").boxed(),
                        Ref::new("NotNullGrammar").boxed(),
                        // COLLATE grammar
                        Ref::new("CollateGrammar").boxed(),
                        // BETWEEN grammar
                        Sequence::new(vec![
                            Ref::keyword("NOT").optional().boxed(),
                            Ref::keyword("BETWEEN").boxed(),
                            Ref::new("Expression_B_Grammar").boxed(),
                            Ref::keyword("AND").boxed(),
                            Ref::new("Tail_Recurse_Expression_A_Grammar").boxed(),
                        ])
                        .boxed(),
                        // Additional sequences and grammar rules can be added here
                    ])
                    .boxed(),
                ])
                .boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        // Expression_B_Grammar: Does not directly feed into Expression_A_Grammar
        // but is used for a BETWEEN statement within Expression_A_Grammar.
        // https://www.cockroachlabs.com/docs/v20.2/sql-grammar.htm#b_expr
        // We use a similar trick as seen with Expression_A_Grammar to avoid recursion
        // by using a tail recursion grammar.  See the comments for a_expr to see how
        // that works.
        (
            "Expression_B_Unary_Operator_Grammar".into(),
            one_of(vec![
                Ref::new("SignedSegmentGrammar")
                    .exclude(Sequence::new(vec![
                        Ref::new("QualifiedNumericLiteralSegment").boxed(),
                    ]))
                    .boxed(),
                Ref::new("TildeSegment").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Tail_Recurse_Expression_B_Grammar".into(),
            Sequence::new(vec![
                // Only safe to use if the recursive call is at the END of the repeating
                // element in the main b_expr portion.
                AnyNumberOf::new(vec![Ref::new("Expression_B_Unary_Operator_Grammar").boxed()])
                    .boxed(),
                Ref::new("Expression_C_Grammar").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_B_Grammar".into(),
            Sequence::new(vec![
                // Always start with the tail recursion element
                Ref::new("Tail_Recurse_Expression_B_Grammar").boxed(),
                AnyNumberOf::new(vec![
                    one_of(vec![
                        // Arithmetic, string, or comparison binary operators followed by tail
                        // recursion
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::new("ArithmeticBinaryOperatorGrammar").boxed(),
                                Ref::new("StringBinaryOperatorGrammar").boxed(),
                                Ref::new("ComparisonOperatorGrammar").boxed(),
                            ])
                            .boxed(),
                            Ref::new("Tail_Recurse_Expression_B_Grammar").boxed(),
                        ])
                        .boxed(),
                        // Additional sequences and rules from b_expr can be added here
                    ])
                    .boxed(),
                ])
                .boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_C_Grammar".into(),
            one_of(vec![
                // Sequence for "EXISTS" with a bracketed selectable grammar
                Sequence::new(vec![
                    Ref::keyword("EXISTS").boxed(),
                    Bracketed::new(vec![Ref::new("SelectableGrammar").boxed()]).boxed(),
                ])
                .boxed(),
                // Sequence for Expression_D_Grammar or CaseExpressionSegment
                // followed by any number of TimeZoneGrammar
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("Expression_D_Grammar").boxed(),
                        Ref::new("CaseExpressionSegment").boxed(),
                    ])
                    .boxed(),
                    AnyNumberOf::new(vec![Ref::new("TimeZoneGrammar").boxed()])
                        .config(|this| this.optional())
                        .boxed(),
                ])
                .boxed(),
                Ref::new("ShorthandCastSegment").boxed(),
            ])
            .config(|this| this.terminators = vec_of_erased![Ref::new("CommaSegment")])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_D_Grammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("BareFunctionSegment").boxed(),
                    Ref::new("FunctionSegment").boxed(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Ref::new("ExpressionSegment").boxed(),
                            Ref::new("SelectableGrammar").boxed(),
                            Delimited::new(vec![
                                Ref::new("ColumnReferenceSegment").boxed(),
                                Ref::new("FunctionSegment").boxed(),
                                Ref::new("LiteralGrammar").boxed(),
                                Ref::new("LocalAliasSegment").boxed(),
                            ])
                            .boxed(),
                        ])
                        .boxed(),
                    ])
                    .config(|this| this.parse_mode(ParseMode::Greedy))
                    .boxed(),
                    Ref::new("SelectStatementSegment").boxed(),
                    Ref::new("LiteralGrammar").boxed(),
                    Ref::new("IntervalExpressionSegment").boxed(),
                    Ref::new("TypedStructLiteralSegment").boxed(),
                    Ref::new("ArrayExpressionSegment").boxed(),
                    Ref::new("ColumnReferenceSegment").boxed(),
                    Sequence::new(vec![
                        Ref::new("SingleIdentifierGrammar").boxed(),
                        Ref::new("ObjectReferenceDelimiterGrammar").boxed(),
                        Ref::new("StarSegment").boxed(),
                    ])
                    .boxed(),
                    Sequence::new(vec![
                        Ref::new("StructTypeSegment").boxed(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ExpressionSegment").boxed()]).boxed(),
                        ])
                        .boxed(),
                    ])
                    .boxed(),
                    Sequence::new(vec![
                        Ref::new("DatatypeSegment").boxed(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").boxed(),
                            Ref::new("NumericLiteralSegment").boxed(),
                            Ref::new("BooleanLiteralGrammar").boxed(),
                            Ref::new("NullLiteralSegment").boxed(),
                            Ref::new("DateTimeLiteralGrammar").boxed(),
                        ])
                        .boxed(),
                    ])
                    .boxed(),
                    Ref::new("LocalAliasSegment").boxed(),
                ])
                .config(|this| this.terminators = vec_of_erased![Ref::new("CommaSegment")])
                .boxed(),
                Ref::new("AccessorGrammar").optional().boxed(),
            ])
            .allow_gaps(true)
            .to_matchable()
            .into(),
        ),
        (
            "AccessorGrammar".into(),
            AnyNumberOf::new(vec![Ref::new("ArrayAccessorSegment").boxed()]).to_matchable().into(),
        ),
    ]);

    ansi_dialect.add([
        (
            "SelectableGrammar".into(),
            one_of(vec![
                optionally_bracketed(vec![Ref::new("WithCompoundStatementSegment").boxed()])
                    .boxed(),
                Ref::new("NonWithSelectableGrammar").boxed(),
                Bracketed::new(vec![Ref::new("SelectableGrammar").boxed()]).boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NonWithSelectableGrammar".into(),
            one_of(vec![
                Ref::new("SetExpressionSegment").boxed(),
                optionally_bracketed(vec![Ref::new("SelectStatementSegment").boxed()]).boxed(),
                Ref::new("NonSetSelectableGrammar").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NonWithNonSelectableGrammar".into(),
            one_of(vec![
                Ref::new("UpdateStatementSegment").boxed(),
                Ref::new("InsertStatementSegment").boxed(),
                Ref::new("DeleteStatementSegment").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NonSetSelectableGrammar".into(),
            one_of(vec![
                Ref::new("ValuesClauseSegment").boxed(),
                Ref::new("UnorderedSelectStatementSegment").boxed(),
                Bracketed::new(vec![Ref::new("SelectStatementSegment").boxed()]).boxed(),
                Bracketed::new(vec![Ref::new("NonSetSelectableGrammar").boxed()]).boxed(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // This is a hook point to allow subclassing for other dialects
    ansi_dialect.add([
        ("PostTableExpressionGrammar".into(), Nothing::new().to_matchable().into()),
        (
            "BracketedSegment".into(),
            BracketedSegment::new(vec![], vec![], vec![], true).to_matchable().into(),
        ),
    ]);

    ansi_dialect
}

pub fn select_clause_element() -> Arc<dyn Matchable> {
    one_of(vec_of_erased![
        // *, blah.*, blah.blah.*, etc.
        Ref::new("WildcardExpressionSegment"),
        Sequence::new(vec_of_erased![
            Ref::new("BaseExpressionElementGrammar"),
            Ref::new("AliasExpressionSegment").optional(),
        ]),
    ])
    .to_matchable()
}

fn lexer_matchers() -> Vec<Matcher> {
    vec![
        Matcher::regex("whitespace", r"[^\S\r\n]+", |slice, marker| {
            WhitespaceSegment::create(slice, marker.into(), WhitespaceSegmentNewArgs {})
        }),
        Matcher::regex("inline_comment", r"(--|#)[^\n]*", |slice, marker| {
            CommentSegment::create(
                slice,
                marker.into(),
                CommentSegmentNewArgs {
                    r#type: "inline_comment",
                    trim_start: Some(vec!["--", "#"]),
                },
            )
        }),
        Matcher::regex("block_comment", r"\/\*([^\*]|\*(?!\/))*\*\/", |slice, marker| {
            CommentSegment::create(
                slice,
                marker.into(),
                CommentSegmentNewArgs { r#type: "block_comment", trim_start: None },
            )
        })
        .subdivider(Pattern::regex("newline", r"\r\n|\n", |slice, marker| {
            NewlineSegment::create(slice, marker.into(), NewlineSegmentNewArgs {})
        }))
        .post_subdivide(Pattern::regex("whitespace", r"[^\S\r\n]+", |slice, marker| {
            WhitespaceSegment::create(slice, marker.into(), WhitespaceSegmentNewArgs {})
        })),
        Matcher::regex("single_quote", r"'([^'\\]|\\.|'')*'", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "single_quote", ..Default::default() },
            )
        }),
        Matcher::regex("double_quote", r#""([^"\\]|\\.)*""#, |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "double_quote", ..Default::default() },
            )
        }),
        Matcher::regex("back_quote", r"`[^`]*`", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "back_quote", ..Default::default() },
            )
        }),
        Matcher::regex("dollar_quote", r"\$(\w*)\$[\s\S]*?\$\1\$", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "dollar_quote", ..Default::default() },
            )
        }),
        Matcher::regex(
            "numeric_literal",
            r"(?>\d+\.\d+|\d+\.(?![\.\w])|\.\d+|\d+)(\.?[eE][+-]?\d+)?((?<=\.)|(?=\b))",
            |raw, position_maker| {
                CodeSegment::create(
                    raw,
                    position_maker.into(),
                    CodeSegmentNewArgs { code_type: "numeric_literal", ..Default::default() },
                )
            },
        ),
        Matcher::regex("like_operator", r"!?~~?\*?", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "like_operator", ..Default::default() },
            )
        }),
        Matcher::regex("newline", r"\r\n|\n", |slice, marker| {
            NewlineSegment::create(slice, marker.into(), NewlineSegmentNewArgs {})
        }),
        Matcher::string("casting_operator", "::", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "casting_operator", ..Default::default() },
            )
        }),
        Matcher::string("equals", "=", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "equals", ..Default::default() },
            )
        }),
        Matcher::string("greater_than", ">", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "greater_than", ..Default::default() },
            )
        }),
        Matcher::string("less_than", "<", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "less_than", ..Default::default() },
            )
        }),
        Matcher::string("not", "!", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "not", ..Default::default() },
            )
        }),
        Matcher::string("dot", ".", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "dot", ..Default::default() },
            )
        }),
        Matcher::string("comma", ",", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "comma", ..Default::default() },
            )
        }),
        Matcher::string("plus", "+", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "plus", ..Default::default() },
            )
        }),
        Matcher::string("minus", "-", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "minus", ..Default::default() },
            )
        }),
        Matcher::string("divide", "/", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "divide", ..Default::default() },
            )
        }),
        Matcher::string("percent", "%", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "percent", ..Default::default() },
            )
        }),
        Matcher::string("question", "?", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "question", ..Default::default() },
            )
        }),
        Matcher::string("ampersand", "&", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "ampersand", ..Default::default() },
            )
        }),
        Matcher::string("vertical_bar", "|", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "vertical_bar", ..Default::default() },
            )
        }),
        Matcher::string("caret", "^", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "caret", ..Default::default() },
            )
        }),
        Matcher::string("star", "*", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "star", ..Default::default() },
            )
        }),
        Matcher::string("start_bracket", "(", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "start_bracket", ..Default::default() },
            )
        }),
        Matcher::string("end_bracket", ")", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "end_bracket", ..Default::default() },
            )
        }),
        Matcher::string("start_square_bracket", "[", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "start_square_bracket", ..Default::default() },
            )
        }),
        Matcher::string("end_square_bracket", "]", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "end_square_bracket", ..Default::default() },
            )
        }),
        Matcher::string("start_curly_bracket", "{", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "start_curly_bracket", ..Default::default() },
            )
        }),
        Matcher::string("end_curly_bracket", "}", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "end_curly_bracket", ..Default::default() },
            )
        }),
        Matcher::string("colon", ":", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "colon", ..Default::default() },
            )
        }),
        Matcher::string("semicolon", ";", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "semicolon", ..Default::default() },
            )
        }),
        Matcher::regex("word", "[0-9a-zA-Z_]+", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "word", ..Default::default() },
            )
        }),
    ]
}

pub fn frame_extent() -> AnyNumberOf {
    one_of(vec_of_erased![
        Sequence::new(vec_of_erased![Ref::keyword("CURRENT"), Ref::keyword("ROW")]),
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("NumericLiteralSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("INTERVAL"),
                    Ref::new("QuotedLiteralSegment")
                ]),
                Ref::keyword("UNBOUNDED")
            ]),
            one_of(vec_of_erased![Ref::keyword("PRECEDING"), Ref::keyword("FOLLOWING")])
        ])
    ])
}

pub fn explainable_stmt() -> AnyNumberOf {
    one_of(vec_of_erased![
        Ref::new("SelectableGrammar"),
        Ref::new("InsertStatementSegment"),
        Ref::new("UpdateStatementSegment"),
        Ref::new("DeleteStatementSegment")
    ])
}

pub fn get_unordered_select_statement_segment_grammar() -> Arc<dyn Matchable> {
    Sequence::new(vec_of_erased![
        Ref::new("SelectClauseSegment"),
        MetaSegment::dedent(),
        Ref::new("FromClauseSegment").optional(),
        Ref::new("WhereClauseSegment").optional(),
        Ref::new("GroupByClauseSegment").optional(),
        Ref::new("HavingClauseSegment").optional(),
        Ref::new("OverlapsClauseSegment").optional(),
        Ref::new("NamedWindowSegment").optional()
    ])
    .terminators(vec_of_erased![
        Ref::new("SetOperatorSegment"),
        Ref::new("WithNoSchemaBindingClauseSegment"),
        Ref::new("WithDataClauseSegment"),
        Ref::new("OrderByClauseSegment"),
        Ref::new("LimitClauseSegment")
    ])
    .config(|this| {
        this.parse_mode(ParseMode::GreedyOnceStarted);
    })
    .to_matchable()
}

pub fn select_statement() -> Arc<dyn Matchable> {
    get_unordered_select_statement_segment_grammar().copy(
        Some(vec_of_erased![
            Ref::new("OrderByClauseSegment").optional(),
            Ref::new("FetchClauseSegment").optional(),
            Ref::new("LimitClauseSegment").optional(),
            Ref::new("NamedWindowSegment").optional()
        ]),
        None,
        None,
        None,
        vec_of_erased![
            Ref::new("SetOperatorSegment"),
            Ref::new("WithNoSchemaBindingClauseSegment"),
            Ref::new("WithDataClauseSegment")
        ],
        true,
    )
}

pub fn select_clause_segment() -> Arc<dyn Matchable> {
    Sequence::new(vec_of_erased![
        Ref::keyword("SELECT"),
        Ref::new("SelectClauseModifierSegment").optional(),
        MetaSegment::indent(),
        Delimited::new(vec_of_erased![Ref::new("SelectClauseElementSegment")])
            .config(|this| this.allow_trailing())
    ])
    .terminators(vec_of_erased![Ref::new("SelectClauseTerminatorGrammar")])
    .config(|this| {
        this.parse_mode(ParseMode::GreedyOnceStarted);
    })
    .to_matchable()
}

pub fn statement_segment() -> Arc<dyn Matchable> {
    one_of(vec![
        Ref::new("SelectableGrammar").boxed(),
        Ref::new("MergeStatementSegment").boxed(),
        Ref::new("InsertStatementSegment").boxed(),
        Ref::new("TransactionStatementSegment").boxed(),
        Ref::new("DropTableStatementSegment").boxed(),
        Ref::new("DropViewStatementSegment").boxed(),
        Ref::new("CreateUserStatementSegment").boxed(),
        Ref::new("DropUserStatementSegment").boxed(),
        Ref::new("TruncateStatementSegment").boxed(),
        Ref::new("AccessStatementSegment").boxed(),
        Ref::new("CreateTableStatementSegment").boxed(),
        Ref::new("CreateRoleStatementSegment").boxed(),
        Ref::new("DropRoleStatementSegment").boxed(),
        Ref::new("AlterTableStatementSegment").boxed(),
        Ref::new("CreateSchemaStatementSegment").boxed(),
        Ref::new("SetSchemaStatementSegment").boxed(),
        Ref::new("DropSchemaStatementSegment").boxed(),
        Ref::new("DropTypeStatementSegment").boxed(),
        Ref::new("CreateDatabaseStatementSegment").boxed(),
        Ref::new("DropDatabaseStatementSegment").boxed(),
        Ref::new("CreateIndexStatementSegment").boxed(),
        Ref::new("DropIndexStatementSegment").boxed(),
        Ref::new("CreateViewStatementSegment").boxed(),
        Ref::new("DeleteStatementSegment").boxed(),
        Ref::new("UpdateStatementSegment").boxed(),
        Ref::new("CreateCastStatementSegment").boxed(),
        Ref::new("DropCastStatementSegment").boxed(),
        Ref::new("CreateFunctionStatementSegment").boxed(),
        Ref::new("DropFunctionStatementSegment").boxed(),
        Ref::new("CreateModelStatementSegment").boxed(),
        Ref::new("DropModelStatementSegment").boxed(),
        Ref::new("DescribeStatementSegment").boxed(),
        Ref::new("UseStatementSegment").boxed(),
        Ref::new("ExplainStatementSegment").boxed(),
        Ref::new("CreateSequenceStatementSegment").boxed(),
        Ref::new("AlterSequenceStatementSegment").boxed(),
        Ref::new("DropSequenceStatementSegment").boxed(),
        Ref::new("CreateTriggerStatementSegment").boxed(),
        Ref::new("DropTriggerStatementSegment").boxed(),
    ])
    .config(|this| this.terminators = vec_of_erased![Ref::new("DelimiterGrammar")])
    .to_matchable()
}

pub fn wildcard_expression_segment() -> Arc<dyn Matchable> {
    Sequence::new(vec![Ref::new("WildcardIdentifierSegment").boxed()]).to_matchable()
}

/// A segment representing a whole file or script.
/// This is also the default "root" segment of the dialect,
/// and so is usually instantiated directly. It therefore
/// has no match_grammar.
#[derive(Hash, Default, Debug, Clone, PartialEq)]
pub struct FileSegment {
    segments: Vec<ErasedSegment>,
    pos_marker: Option<PositionMarker>,
    uuid: Uuid,
}

impl FileSegment {
    pub fn root_parse(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
        _f_name: Option<String>,
    ) -> Result<ErasedSegment, SQLParseError> {
        // Trim the start
        let start_idx = segments.iter().position(|segment| segment.is_code()).unwrap_or(0);

        // Trim the end
        // Note: The '+ 1' in the end is to include the segment at 'end_idx' in the
        // slice.
        let end_idx =
            segments.iter().rposition(|segment| segment.is_code()).map_or(start_idx, |idx| idx + 1);

        if start_idx == end_idx {
            let mut file =
                FileSegment { segments: segments.to_vec(), uuid: Uuid::new_v4(), pos_marker: None }
                    .to_erased_segment();

            let b = pos_marker(file.segments()).into();
            file.get_mut().set_position_marker(b);

            return Ok(file);
        }

        let final_seg = segments.last().unwrap();
        assert!(final_seg.get_position_marker().is_some());

        let _closing_position = final_seg.get_position_marker().unwrap().templated_slice.end;

        let match_result = {
            // NOTE: Don't call .match() on the segment class itself, but go
            // straight to the match grammar inside.
            let cls = parse_context.dialect().r#ref("FileSegment");
            cls.match_grammar()
                .unwrap()
                .match_segments(&segments[start_idx..end_idx], parse_context)
        }?;

        let has_match = match_result.has_match();
        let MatchResult { matched_segments, unmatched_segments } = match_result;

        let content: Vec<_> = if !has_match {
            vec![UnparsableSegment::new(segments[start_idx..end_idx].to_vec()).to_erased_segment()]
        } else if !unmatched_segments.is_empty() {
            let idx = unmatched_segments
                .iter()
                .position(|item| item.is_code())
                .unwrap_or(unmatched_segments.len());
            let mut result = Vec::new();
            result.extend(matched_segments.clone());
            result.extend(unmatched_segments.iter().take(idx).cloned());
            if idx < unmatched_segments.len() {
                result.push(
                    UnparsableSegment::new(unmatched_segments[idx..].to_vec()).to_erased_segment(),
                );
            }

            result
        } else {
            chain(matched_segments, unmatched_segments).collect()
        };

        let mut result = Vec::new();
        result.extend_from_slice(&segments[..start_idx]);
        result.extend(content);
        result.extend_from_slice(&segments[end_idx..]);

        let mut file = Self { segments: result, uuid: Uuid::new_v4(), pos_marker: None };
        file.set_position_marker(pos_marker(&file.segments).into());

        Ok(file.to_erased_segment())
    }
}

impl Segment for FileSegment {
    fn new(&self, segments: Vec<ErasedSegment>) -> ErasedSegment {
        FileSegment { segments, uuid: self.uuid, pos_marker: self.pos_marker.clone() }
            .to_erased_segment()
    }

    fn get_type(&self) -> &'static str {
        "file"
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.pos_marker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.pos_marker = position_marker;
    }

    fn segments(&self) -> &[ErasedSegment] {
        &self.segments
    }

    fn class_types(&self) -> AHashSet<&'static str> {
        ["file"].into()
    }
}

pub struct FromExpressionElementSegment(pub ErasedSegment);
pub struct FromClauseSegment(pub ErasedSegment);

impl FromClauseSegment {
    pub fn eventual_aliases(&self) -> Vec<(ErasedSegment, AliasInfo)> {
        let mut buff = Vec::new();
        let mut direct_table_children = Vec::new();
        let mut join_clauses = Vec::new();

        for from_expression in self.0.children(&["from_expression"]) {
            direct_table_children.extend(from_expression.children(&["from_expression_element"]));
            join_clauses.extend(from_expression.children(&["join_clause"]));
        }

        for clause in &direct_table_children {
            let tmp;

            let alias = FromExpressionElementSegment(clause.clone()).eventual_alias();

            let table_expr = if direct_table_children.contains(clause) {
                clause
            } else {
                tmp = clause.child(&["from_expression_element"]).unwrap();
                &tmp
            };

            buff.push((table_expr.clone(), alias));
        }

        for clause in join_clauses {
            let aliases = JoinClauseSegment(clause).eventual_aliases();

            if !aliases.is_empty() {
                buff.extend(aliases);
            }
        }

        buff
    }
}

#[derive(Clone)]
pub struct SelectClauseElementSegment(pub ErasedSegment);

impl SelectClauseElementSegment {
    pub fn alias(&self) -> Option<ColumnAliasInfo> {
        let alias_expression_segment =
            self.0.recursive_crawl(&["alias_expression"], true, None, true).first()?.clone();

        let alias_identifier_segment = alias_expression_segment
            .segments()
            .iter()
            .find(|it| matches!(it.get_type(), "naked_identifier" | "identifier"))?;

        let aliased_segment = self
            .0
            .segments()
            .iter()
            .find(|&s| !s.is_whitespace() && !s.is_meta() && s != &alias_expression_segment)
            .unwrap();

        let mut column_reference_segments = Vec::new();
        if aliased_segment.is_type("column_reference") {
            column_reference_segments.push(aliased_segment.clone());
        } else {
            column_reference_segments.extend(aliased_segment.recursive_crawl(
                &["column_reference"],
                true,
                None,
                true,
            ));
        }

        Some(ColumnAliasInfo {
            alias_identifier_name: alias_identifier_segment.raw().into(),
            aliased_segment: aliased_segment.clone(),
            column_reference_segments,
        })
    }
}

impl FromExpressionElementSegment {
    pub fn eventual_alias(&self) -> AliasInfo {
        let mut tbl_expression = self.0.child(&["table_expression"]).or_else(|| {
            self.0
                .child(&["bracketed"])
                .and_then(|bracketed| bracketed.child(&["table_expression"]))
        });

        if let Some(tbl_expression_inner) = &tbl_expression
            && tbl_expression_inner.child(&["object_reference"]).is_none()
        {
            let bracketed = tbl_expression_inner.child(&["bracketed"]);
            if let Some(bracketed) = bracketed {
                tbl_expression = bracketed.child(&["table_expression"]);
            }
        }

        let reference = tbl_expression.and_then(|tbl_expression| {
            tbl_expression.child(&["object_reference", "table_reference"])
        });

        let reference = reference.as_ref().map(|reference| reference.reference());

        let alias_expression = self.0.child(&["alias_expression"]);
        if let Some(alias_expression) = alias_expression {
            let segment = alias_expression.child(&["identifier", "naked_identifier"]);
            if let Some(segment) = segment {
                return AliasInfo {
                    ref_str: segment.raw().into(),
                    segment: segment.into(),
                    aliased: true,
                    from_expression_element: self.0.clone(),
                    alias_expression: alias_expression.into(),
                    object_reference: reference.map(|it| it.clone().0),
                };
            }
        }

        if let Some(reference) = &reference {
            let references = reference.iter_raw_references();
            if !references.is_empty() {
                let penultimate_ref = references.last().unwrap();
                return AliasInfo {
                    ref_str: penultimate_ref.part.clone().into(),
                    segment: penultimate_ref.segments[0].clone().into(),
                    aliased: false,
                    from_expression_element: self.0.clone(),
                    alias_expression: None,
                    object_reference: reference.clone().0.into(),
                };
            }
        }

        AliasInfo {
            ref_str: SmolStr::new_static(""),
            segment: None,
            aliased: false,
            from_expression_element: self.0.clone(),
            alias_expression: None,
            object_reference: reference.map(|it| it.clone().0),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ObjectReferenceLevel {
    Object = 1,
    Table = 2,
    Schema = 3,
}

#[derive(Clone, Debug)]
pub struct ObjectReferencePart {
    pub part: String,
    pub segments: Vec<ErasedSegment>,
}

#[derive(Clone)]
pub struct ObjectReferenceSegment(pub ErasedSegment);

impl ObjectReferenceSegment {
    pub fn is_qualified(&self) -> bool {
        self.iter_raw_references().len() > 1
    }

    pub fn qualification(&self) -> &'static str {
        if self.is_qualified() { "qualified" } else { "unqualified" }
    }

    pub fn extract_possible_references(
        &self,
        level: ObjectReferenceLevel,
        dialect: &str,
    ) -> Vec<ObjectReferencePart> {
        let refs = self.iter_raw_references();

        match dialect {
            "ansi" | "postgres" | "clickhouse" => {
                let level = level as usize;
                if refs.len() >= level && level > 0 {
                    refs.get(refs.len() - level).cloned().into_iter().collect()
                } else {
                    vec![]
                }
            }
            "bigquery" => {
                if level == ObjectReferenceLevel::Schema && refs.len() >= 3 {
                    return vec![refs[0].clone()];
                }

                if level == ObjectReferenceLevel::Table {
                    return refs.into_iter().take(3).collect_vec();
                }

                if level == ObjectReferenceLevel::Object && refs.len() >= 3 {
                    return vec![refs[1].clone(), refs[2].clone()];
                }

                self.extract_possible_references(level, "ansi")
            }
            _ => unimplemented!(),
        }
    }

    pub fn extract_possible_multipart_references(
        &self,
        levels: &[ObjectReferenceLevel],
    ) -> Vec<Vec<ObjectReferencePart>> {
        let refs = self.iter_raw_references();
        let mut sorted_levels = levels.to_vec();
        sorted_levels.sort_unstable();

        if let (Some(&min_level), Some(&max_level)) = (sorted_levels.first(), sorted_levels.last())
        {
            if refs.len() >= max_level as usize {
                let start = refs.len() - max_level as usize;
                let end = refs.len() - min_level as usize + 1;
                if start < end {
                    return vec![refs[start..end].to_vec()];
                }
            }
        }
        vec![]
    }

    pub fn iter_raw_references(&self) -> Vec<ObjectReferencePart> {
        let mut acc = Vec::new();

        for elem in self.0.recursive_crawl(&["identifier", "naked_identifier"], true, None, true) {
            acc.extend(self.iter_reference_parts(elem));
        }

        acc
    }

    fn iter_reference_parts(&self, elem: ErasedSegment) -> Vec<ObjectReferencePart> {
        let mut acc = Vec::new();

        let raw = elem.raw();
        let parts = raw.split('.');

        for part in parts {
            acc.push(ObjectReferencePart { part: part.into(), segments: vec![elem.clone()] });
        }

        acc
    }
}

pub struct JoinClauseSegment(ErasedSegment);

impl JoinClauseSegment {
    fn eventual_aliases(&self) -> Vec<(ErasedSegment, AliasInfo)> {
        let mut buff = Vec::new();

        let from_expression = self.0.child(&["from_expression_element"]).unwrap();
        let alias = FromExpressionElementSegment(from_expression.clone()).eventual_alias();

        buff.push((from_expression.clone(), alias));

        for join_clause in
            self.0.recursive_crawl(&["join_clause"], true, "select_statement".into(), true)
        {
            if join_clause.get_uuid() == join_clause.get_uuid() {
                continue;
            }

            let aliases = JoinClauseSegment(join_clause).eventual_aliases();

            if !aliases.is_empty() {
                buff.extend(aliases);
            }
        }

        buff
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect_file;
    use itertools::Itertools;
    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

    use crate::core::config::FluffConfig;
    use crate::core::linter::linter::Linter;
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::lexer::{Lexer, StringOrTemplate};
    use crate::core::parser::segments::base::ErasedSegment;
    use crate::core::parser::segments::test_functions::{fresh_ansi_dialect, lex};
    use crate::helpers;

    #[test]
    fn test__dialect__ansi__file_lex() {
        // Define the test cases
        let test_cases = vec![
            ("a b", vec!["a", " ", "b", ""]),
            ("b.c", vec!["b", ".", "c", ""]),
            ("abc \n \t def  ;blah", vec!["abc", " ", "\n", " \t ", "def", "  ", ";", "blah", ""]),
        ];

        for (raw, res) in test_cases {
            // Assume FluffConfig and Lexer are defined somewhere in your codebase
            let config = FluffConfig::new(<_>::default(), None, None);

            let lexer = Lexer::new(&config, None);

            // Assume that the lex function returns a Result with tokens
            let tokens_result = lexer.lex(StringOrTemplate::String(raw));

            // Check if lexing was successful, and if not, fail the test
            assert!(tokens_result.is_ok(), "Lexing failed for input: {}", raw);

            let (tokens, errors) = tokens_result.unwrap();
            assert_eq!(errors.len(), 0, "Lexing failed for input: {}", raw);

            // Check if the raw components of the tokens match the expected result
            let raw_list: Vec<_> = tokens.iter().map(|token| token.raw()).collect();
            assert_eq!(raw_list, res, "Mismatch for input: {:?}", raw);

            // Check if the concatenated raw components of the tokens match the original raw
            // string
            let concatenated: String = tokens.iter().map(|token| token.raw()).collect();
            assert_eq!(concatenated, raw, "Concatenation mismatch for input: {}", raw);
        }
    }

    #[test]
    fn test__dialect__ansi_specific_segment_parses() {
        let cases = [
            ("SelectKeywordSegment", "select"),
            ("NakedIdentifierSegment", "online_sales"),
            ("BareFunctionSegment", "current_timestamp"),
            ("FunctionSegment", "current_timestamp()"),
            ("NumericLiteralSegment", "1000.0"),
            ("ExpressionSegment", "online_sales / 1000.0"),
            ("IntervalExpressionSegment", "INTERVAL 1 YEAR"),
            ("ExpressionSegment", "CASE WHEN id = 1 THEN 'nothing' ELSE 'test' END"),
            // Nested Case Expressions
            (
                "ExpressionSegment",
                "CASE WHEN id = 1 THEN CASE WHEN true THEN 'something' ELSE 'nothing' END
            ELSE 'test' END",
            ),
            // Casting expressions
            ("ExpressionSegment", "CAST(ROUND(online_sales / 1000.0) AS varchar)"),
            // Like expressions
            ("ExpressionSegment", "name NOT LIKE '%y'"),
            // Functions with a space
            ("SelectClauseElementSegment", "MIN (test.id) AS min_test_id"),
            // Interval literals
            (
                "ExpressionSegment",
                "DATE_ADD(CURRENT_DATE('America/New_York'), INTERVAL 1
            year)",
            ),
            // Array accessors
            ("ExpressionSegment", "my_array[1]"),
            ("ExpressionSegment", "my_array[OFFSET(1)]"),
            ("ExpressionSegment", "my_array[5:8]"),
            ("ExpressionSegment", "4 + my_array[OFFSET(1)]"),
            ("ExpressionSegment", "bits[OFFSET(0)] + 7"),
            (
                "SelectClauseElementSegment",
                "(count_18_24 * bits[OFFSET(0)]) / audience_size AS relative_abundance",
            ),
            ("ExpressionSegment", "count_18_24 * bits[OFFSET(0)] + count_25_34"),
            (
                "SelectClauseElementSegment",
                "(count_18_24 * bits[OFFSET(0)] + count_25_34) / audience_size AS \
                 relative_abundance",
            ),
            // Dense math expressions
            ("SelectStatementSegment", "SELECT t.val/t.id FROM test WHERE id*1.0/id > 0.8"),
            ("SelectStatementSegment", "SELECT foo FROM bar INNER JOIN baz"),
            ("SelectClauseElementSegment", "t.val/t.id"),
            // Issue with casting raise as part of PR #177
            ("SelectClauseElementSegment", "CAST(num AS INT64)"),
            // Casting as datatype with arguments
            ("SelectClauseElementSegment", "CAST(num AS numeric(8,4))"),
            // Wildcard field selection
            ("SelectClauseElementSegment", "a.*"),
            ("SelectClauseElementSegment", "a.b.*"),
            ("SelectClauseElementSegment", "a.b.c.*"),
            // Default Element Syntax
            ("SelectClauseElementSegment", "a..c.*"),
            // Negative Elements
            ("SelectClauseElementSegment", "-some_variable"),
            ("SelectClauseElementSegment", "- some_variable"),
            // Complex Functions
            (
                "ExpressionSegment",
                "concat(left(uaid, 2), '|', right(concat('0000000', SPLIT_PART(uaid, '|', 4)),
            10), '|', '00000000')",
            ),
            // Notnull and Isnull
            ("ExpressionSegment", "c is null"),
            ("ExpressionSegment", "c is not null"),
            ("SelectClauseElementSegment", "c is null as c_isnull"),
            ("SelectClauseElementSegment", "c is not null as c_notnull"),
            // Shorthand casting
            ("ExpressionSegment", "NULL::INT"),
            ("SelectClauseElementSegment", "NULL::INT AS user_id"),
            ("TruncateStatementSegment", "TRUNCATE TABLE test"),
            ("TruncateStatementSegment", "TRUNCATE test"),
            ("FunctionNameSegment", "cte_1.foo"),
            ("SelectStatementSegment", "select * from my_cte cross join other_cte"),
        ];

        let dialect = fresh_ansi_dialect();
        let config = FluffConfig::default();

        for (segment_ref, sql_string) in cases {
            let mut ctx = ParseContext::new(&dialect, <_>::default());

            let segment = dialect.r#ref(segment_ref);
            let mut segments = lex(&config, sql_string);

            if segments.last().unwrap().get_type() == "end_of_file" {
                segments.pop();
            }

            let mut match_result = segment.match_segments(&segments, &mut ctx).unwrap();

            assert_eq!(match_result.len(), 1, "failed {segment_ref}, {sql_string}");

            let parsed = match_result.matched_segments.pop().unwrap();
            assert_eq!(sql_string, parsed.raw());
        }
    }

    #[test]
    fn test__dialect__ansi_specific_segment_not_match() {
        let cases = [("ObjectReferenceSegment", "\n     ")];

        let dialect = fresh_ansi_dialect();
        let config = FluffConfig::new(<_>::default(), None, None);

        for (segment_ref, sql) in cases {
            let segments = lex(&config, sql);

            let mut parse_cx = ParseContext::from_config(&config);
            let segment = dialect.r#ref(segment_ref);

            let match_result = segment.match_segments(&segments, &mut parse_cx).unwrap();
            assert!(!match_result.has_match());
        }
    }

    #[test]
    fn test__dialect__ansi_specific_segment_not_parse() {
        let tests = vec![
            ("SELECT 1 + (2 ", vec![(1, 12)]),
            // ("SELECT * FROM a ORDER BY 1 UNION SELECT * FROM b", vec![(1, 28)]),
            // (
            //     "SELECT * FROM a LIMIT 1 UNION SELECT * FROM b",
            //     vec![(1, 25)],
            // ),
            // (
            //     "SELECT * FROM a ORDER BY 1 LIMIT 1 UNION SELECT * FROM b",
            //     vec![(1, 36)],
            // ),
        ];

        for (raw, err_locations) in tests {
            let lnt = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None);
            let parsed = lnt.parse_string(raw, None, None, None).unwrap();
            assert!(!parsed.violations.is_empty());

            let locs: Vec<(usize, usize)> =
                parsed.violations.iter().map(|v| (v.line_no, v.line_pos)).collect();
            assert_eq!(locs, err_locations);
        }
    }

    #[test]
    fn test__dialect__ansi_is_whitespace() {
        let lnt = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None);
        let file_content =
            std::fs::read_to_string("test/fixtures/dialects/ansi/select_in_multiline_comment.sql")
                .expect("Unable to read file");

        let parsed = lnt.parse_string(&file_content, None, None, None).unwrap();

        for raw_seg in parsed.tree.unwrap().get_raw_segments() {
            if raw_seg.is_type("whitespace") || raw_seg.is_type("newline") {
                assert!(raw_seg.is_whitespace());
            }
        }
    }

    #[test]
    fn test__dialect__ansi_parse_indented_joins() {
        let cases = [
            // ("select field_1 from my_table as alias_1", [1, 5, 8, 11, 15, 16, 17].as_slice()),
            (
                "select field_1 from my_table as alias_1 join foo using (field_1)",
                [1, 5, 8, 11, 15, 17, 19, 23, 24, 26, 29, 31, 33, 34, 35].as_slice(),
            ),
        ];
        let lnt = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None);

        for (sql_string, meta_loc) in cases {
            let parsed = lnt.parse_string(sql_string, None, None, None).unwrap();
            let tree = parsed.tree.unwrap();

            let res_meta_locs = tree
                .get_raw_segments()
                .into_iter()
                .enumerate()
                .filter_map(|(idx, raw_seg)| raw_seg.is_meta().then_some(idx))
                .collect_vec();

            assert_eq!(res_meta_locs, meta_loc);
        }
    }

    fn parse_sql(linter: &Linter, sql: &str) -> ErasedSegment {
        let parsed = linter.parse_string(sql, None, None, None).unwrap();
        parsed.tree.unwrap()
    }

    #[test]
    fn base_parse_struct() {
        let linter = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None);

        let files =
            glob::glob("test/fixtures/dialects/ansi/*.sql").unwrap().flatten().collect_vec();

        files.par_iter().for_each(|file| {
            let _panic = helpers::enter_panic(file.display().to_string());

            let yaml = file.with_extension("yml");
            let yaml = std::path::absolute(yaml).unwrap();

            let actual = {
                let sql = std::fs::read_to_string(file).unwrap();
                let tree = parse_sql(&linter, &sql);
                let tree = tree.to_serialised(true, true, false);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml].assert_eq(&actual);
        });
    }
}
