use std::collections::HashSet;
use std::hash::Hash;

use itertools::{chain, Itertools};
use uuid::Uuid;

use super::ansi_keywords::{ANSI_RESERVED_KEYWORDS, ANSI_UNRESERVED_KEYWORDS};
use crate::core::dialects::base::Dialect;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::grammar::anyof::{one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::{Nothing, Ref};
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::{Matcher, RegexLexer, StringLexer};
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs, NewlineSegment,
    NewlineSegmentNewArgs, Segment, SegmentConstructorFn, SymbolSegment, SymbolSegmentNewArgs,
    WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::common::LiteralSegment;
use crate::core::parser::segments::generator::SegmentGenerator;
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::parser::types::ParseMode;
use crate::helpers::{Boxed, Config, ToMatchable};

macro_rules! from_segments {
    ($self:expr, $segments:expr) => {{
        let mut new_object = $self.clone();
        new_object.segments = $segments;
        new_object.to_matchable()
    }};
}

macro_rules! vec_of_erased {
    ($($elem:expr),*) => {{
        vec![$(Box::new($elem)),*]
    }};
}

pub fn ansi_dialect() -> Dialect {
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
            ("round".into(), "StartBracketSegment".into(), "EndBracketSegment".into(), true),
            (
                "square".into(),
                "StartSquareBracketSegment".into(),
                "EndSquareBracketSegment".into(),
                false,
            ),
            (
                "curly".into(),
                "StartCurlyBracketSegment".into(),
                "EndCurlyBracketSegment".into(),
                false,
            ),
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
        SymbolSegment::new(
            &segment.get_raw().unwrap(),
            &segment.get_position_marker().unwrap(),
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
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
                        SymbolSegmentNewArgs { r#type: "statement_terminator" },
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
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
            StringParser::new("[", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "EndSquareBracketSegment".into(),
            StringParser::new("]", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "StartCurlyBracketSegment".into(),
            StringParser::new("{", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "EndCurlyBracketSegment".into(),
            StringParser::new("}", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "CommaSegment".into(),
            StringParser::new(
                ",",
                |segment: &dyn Segment| {
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
            StringParser::new(".", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "StarSegment".into(),
            StringParser::new("*", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "TildeSegment".into(),
            StringParser::new(
                "~",
                |segment: &dyn Segment| {
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
            StringParser::new("::", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "PlusSegment".into(),
            StringParser::new(
                "+",
                |segment: &dyn Segment| {
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
            StringParser::new("-", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "PositiveSegment".into(),
            StringParser::new(
                "+",
                |segment: &dyn Segment| {
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
            StringParser::new("/", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "MultiplySegment".into(),
            StringParser::new("*", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "ModuloSegment".into(),
            StringParser::new("%", symbol_factory, None, false, None).to_matchable().into(),
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
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
            StringParser::new("^", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "LikeOperatorSegment".into(),
            TypedParser::new("like_operator", |_| unimplemented!(), None, false, None)
                .to_matchable()
                .into(),
        ),
        (
            "RawNotSegment".into(),
            StringParser::new("!", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "RawEqualsSegment".into(),
            StringParser::new("=", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "RawGreaterThanSegment".into(),
            StringParser::new(">", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "RawLessThanSegment".into(),
            StringParser::new("<", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            // The following functions can be called without parentheses per ANSI specification
            "BareFunctionSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(
                    dialect.sets("bare_functions").into_iter().map(Into::into).collect_vec(),
                    |segment| {
                        CodeSegment::new(
                            &segment.get_raw().unwrap(),
                            &segment.get_position_marker().unwrap(),
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
                        Box::new(KeywordSegment::new(
                            segment.get_raw().unwrap(),
                            segment.get_position_marker().unwrap().into(),
                        ))
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

                RegexParser::new(pattern, |_| todo!(), None, false, None, None).boxed()
            })
            .into(),
        ),
        (
            "FunctionNameIdentifierSegment".into(),
            TypedParser::new(
                "word",
                |segment: &dyn Segment| {
                    SymbolSegment::new(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
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
                            CodeSegment::new(
                                &segment.get_raw().unwrap(),
                                &segment.get_position_marker().unwrap(),
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
                        CodeSegment::new(
                            &segment.get_raw().unwrap(),
                            &segment.get_position_marker().unwrap(),
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
            "DatePartFunctionName".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(
                    dialect
                        .sets("date_part_function_name")
                        .into_iter()
                        .map(Into::into)
                        .collect::<Vec<_>>(),
                    |segment| {
                        CodeSegment::new(
                            &segment.get_raw().unwrap(),
                            &segment.get_position_marker().unwrap(),
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
            TypedParser::new("double_quote", symbol_factory, None, false, None)
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
            TypedParser::new("single_quote", symbol_factory, None, false, None)
                .to_matchable()
                .into(),
        ),
        (
            "NumericLiteralSegment".into(),
            TypedParser::new(
                "numeric_literal",
                |seg| {
                    LiteralSegment {
                        raw: seg.get_raw().unwrap(),
                        position_maker: seg.get_position_marker().unwrap(),
                        uuid: seg.get_uuid().unwrap(),
                    }
                    .boxed()
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
            StringParser::new("null", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "NanLiteralSegment".into(),
            StringParser::new("nan", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "TrueSegment".into(),
            StringParser::new("true", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "FalseSegment".into(),
            StringParser::new("false", symbol_factory, None, false, None).to_matchable().into(),
        ),
        // We use a GRAMMAR here not a Segment. Otherwise, we get an unnecessary layer
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment") // terminators=[Ref("DotSegment")],
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BooleanLiteralGrammar".into(),
            one_of(vec_of_erased![Ref::new("TrueSegment"), Ref::new("TrueSegment")])
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
                TypedParser::new("single_quote", |_| unimplemented!(), None, false, None)
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
            StringParser::new("AND", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "OrOperatorGrammar".into(),
            StringParser::new("OR", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "NotOperatorGrammar".into(),
            StringParser::new("NOT", symbol_factory, None, false, None).to_matchable().into(),
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
                one_of(vec![Ref::keyword("DISTINCT").boxed(), Ref::keyword("ALL").boxed()])
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
            "SelectClauseTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("FROM").boxed(),
                Ref::keyword("WHERE").boxed(),
                Sequence::new(vec![Ref::keyword("ORDER").boxed(), Ref::keyword("BY").boxed()])
                    .boxed(),
                Ref::keyword("LIMIT").boxed(),
                Ref::keyword("OVERLAPS").boxed(),
                Ref::new("SetOperatorSegment").boxed(),
                Ref::keyword("FETCH").boxed(),
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
                // Ref::new("WithNoSchemaBindingClauseSegment").boxed(),
                // Ref::new("WithDataClauseSegment").boxed(),
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
            .config(|_this| {
                // These terminators allow better performance by giving a signal
                // of a likely complete match if they come after a match. For
                // example "123," only needs to match against the LiteralGrammar
                // and because a comma follows, never be matched against
                // ExpressionSegment or FunctionSegment, which are both much
                // more complicated.

                // vec![
                //     Ref::new("CommaSegment").boxed(),
                //     Ref::keyword("AS").boxed(),
                //     // TODO: We can almost certainly add a few more here.
                // ]
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
            Sequence::new(vec![
                Ref::keyword("REFERENCES").boxed(),
                Ref::new("TableReferenceSegment").boxed(),
                // Optional foreign columns making up FOREIGN KEY constraint
                Ref::new("BracketedColumnReferenceListGrammar").optional().boxed(),
                Sequence::new(vec![
                    Ref::keyword("MATCH").boxed(),
                    one_of(vec![
                        Ref::keyword("FULL").boxed(),
                        Ref::keyword("PARTIAL").boxed(),
                        Ref::keyword("SIMPLE").boxed(),
                    ])
                    .config(|this| this.optional())
                    .boxed(),
                ])
                .boxed(),
                // AnySetOf::new(vec![
                //     // ON DELETE clause, e.g., ON DELETE NO ACTION
                //     Sequence::new(vec![
                //         Ref::keyword("ON").boxed(),
                //         Ref::keyword("DELETE").boxed(),
                //         Ref::new("ReferentialActionGrammar").boxed(),
                //     ]).boxed(),
                //     // ON UPDATE clause, e.g., ON UPDATE SET NULL
                //     Sequence::new(vec![
                //         Ref::keyword("ON").boxed(),
                //         Ref::keyword("UPDATE").boxed(),
                //         Ref::new("ReferentialActionGrammar").boxed(),
                //     ]).boxed(),
                // ]).boxed(),
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
                Ref::new("BracketedSegment").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        // (
        //     "AlterTableOptionsGrammar".into(),
        //     one_of(vec![
        //         // Table options
        //         Sequence::new(vec![
        //             Ref::new("ParameterNameSegment").boxed(),
        //             Ref::new("EqualsSegment").optional().boxed(),
        //             one_of(vec![
        //                 Ref::new("LiteralGrammar").boxed(),
        //                 Ref::new("NakedIdentifierSegment").boxed(),
        //             ])
        //             .boxed(),
        //         ])
        //         .boxed(),
        //         // Add things
        //         Sequence::new(vec![
        //             one_of(vec![Ref::keyword("ADD").boxed(), Ref::keyword("MODIFY").boxed()])
        //                 .boxed(),
        //             Ref::keyword("COLUMN").optional().boxed(),
        //             Ref::new("ColumnDefinitionSegment").boxed(),
        //             one_of(vec![
        //                 Sequence::new(vec![
        //                     one_of(vec![
        //                         Ref::keyword("FIRST").boxed(),
        //                         Ref::keyword("AFTER").boxed(),
        //                     ])
        //                     .boxed(),
        //                     Ref::new("ColumnReferenceSegment").boxed(),
        //                 ])
        //                 .boxed(),
        //                 // Bracketed Version of the same
        //                 Ref::new("BracketedColumnReferenceListGrammar").boxed(),
        //             ])
        //             .optional()
        //             .boxed(),
        //         ])
        //         .boxed(),
        //         // Rename
        //         Sequence::new(vec![
        //             Ref::keyword("RENAME").boxed(),
        //             one_of(vec![Ref::keyword("AS").boxed(), Ref::keyword("TO").boxed()])
        //                 .optional()
        //                 .boxed(),
        //             Ref::new("TableReferenceSegment").boxed(),
        //         ])
        //         .boxed(),
        //     ])
        //     .to_matchable()
        //     .into(),
        // ),
        // END
        ("TableReferenceSegment".into(), Ref::new("SingleIdentifierGrammar").to_matchable().into()),
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
            Sequence::new(vec![
                // This should be used instead of a recursive call to Expression_A_Grammar
                // whenever the repeating element in Expression_A_Grammar makes a recursive
                // call to itself at the _end_.
                AnyNumberOf::new(vec![Ref::new("Expression_A_Unary_Operator_Grammar").boxed()])
                    //  .with_terminators(vec![Ref::new("BinaryOperatorGrammar").boxed()])
                    .boxed(),
                Ref::new("Expression_C_Grammar").boxed(),
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
                // ShorthandCastSegment
                Ref::new("ShorthandCastSegment").boxed(),
            ])
            //.with_terminators(vec![Ref::new("CommaSegment").boxed()])
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
                // .with_terminators(vec![Ref::new("CommaSegment").boxed()])
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
        ("EqualsSegment".into(), Ref::new("RawEqualsSegment").to_matchable().into()),
        ("GreaterThanSegment".into(), Ref::new("RawGreaterThanSegment").to_matchable().into()),
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

    macro_rules! add_segments {
        ($dialect:ident, $( $segment:ident ),*) => {
            $(
                $dialect.add([(
                    stringify!($segment).into(),
                    $segment { segments: Vec::new(), uuid: Uuid::new_v4() }.to_matchable().into(),
                )]);
            )*
        }
    }

    #[rustfmt::skip]
    add_segments!(
        ansi_dialect, OverClauseSegment, FromExpressionElementSegment, SelectClauseElementSegment, FromExpressionSegment, FromClauseSegment,
        WildcardIdentifierSegment, ColumnReferenceSegment, WildcardExpressionSegment, SelectStatementSegment, StatementSegment, WindowSpecificationSegment,
        SetExpressionSegment, UnorderedSelectStatementSegment, SelectClauseSegment, JoinClauseSegment, TableExpressionSegment,
        ConcatSegment, EmptyStructLiteralSegment, ArrayLiteralSegment, LessThanSegment, GreaterThanOrEqualToSegment,
        LessThanOrEqualToSegment, NotEqualToSegment, JoinOnConditionSegment, PartitionClauseSegment,
        BitwiseAndSegment, ArrayTypeSegment, BitwiseOrSegment, BitwiseLShiftSegment, CTEDefinitionSegment,
        BitwiseRShiftSegment, IndexColumnDefinitionSegment, AggregateOrderByClause, ValuesClauseSegment,
        ArrayAccessorSegment, CaseExpressionSegment, WhenClauseSegment, BracketedArguments, CTEColumnList,
        TypedStructLiteralSegment, StructTypeSegment, TimeZoneGrammar, FrameClauseSegment,
        SetOperatorSegment, WhereClauseSegment, ElseClauseSegment, IntervalExpressionSegment,
        QualifiedNumericLiteralSegment, FunctionSegment, FunctionNameSegment, TypedArrayLiteralSegment,
        SelectClauseModifierSegment, OrderByClauseSegment, WithCompoundStatementSegment,
        TruncateStatementSegment, ExpressionSegment, ShorthandCastSegment, DatatypeSegment, AliasExpressionSegment,
        ObjectReferenceSegment, ObjectLiteralSegment, ArrayExpressionSegment, LocalAliasSegment,
        MergeStatementSegment, InsertStatementSegment, TransactionStatementSegment, DropTableStatementSegment,
        DropViewStatementSegment, CreateUserStatementSegment, DropUserStatementSegment, AccessStatementSegment,
        CreateTableStatementSegment, CreateRoleStatementSegment, DropRoleStatementSegment, AlterTableStatementSegment,
        CreateSchemaStatementSegment, SetSchemaStatementSegment, DropSchemaStatementSegment, DropTypeStatementSegment,
        CreateDatabaseStatementSegment, DropDatabaseStatementSegment, CreateIndexStatementSegment,
        DropIndexStatementSegment, CreateViewStatementSegment, DeleteStatementSegment, UpdateStatementSegment,
        CreateCastStatementSegment, DropCastStatementSegment, CreateFunctionStatementSegment, DropFunctionStatementSegment,
        CreateModelStatementSegment, DropModelStatementSegment, DescribeStatementSegment, UseStatementSegment, ExplainStatementSegment,
        CreateSequenceStatementSegment, AlterSequenceStatementSegment, DropSequenceStatementSegment, CreateTriggerStatementSegment, DropTriggerStatementSegment
    );

    ansi_dialect.expand();
    ansi_dialect
}

fn lexer_matchers() -> Vec<Box<dyn Matcher>> {
    vec![
        // Match all forms of whitespace except newlines and carriage returns:
        // https://stackoverflow.com/questions/3469080/match-whitespace-but-not-newlines
        // This pattern allows us to also match non-breaking spaces (#2189).
        Box::new(
            RegexLexer::new(
                "whitespace",
                r"[^\S\r\n]+",
                &WhitespaceSegment::new as SegmentConstructorFn<WhitespaceSegmentNewArgs>,
                WhitespaceSegmentNewArgs {},
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "inline_comment",
                r"(--|#)[^\n]*",
                &CommentSegment::new as SegmentConstructorFn<CommentSegmentNewArgs>,
                CommentSegmentNewArgs {
                    r#type: "inline_comment",
                    trim_start: Some(vec!["--", "#"]),
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "block_comment",
                r"\/\*([^\*]|\*(?!\/))*\*\/",
                &CommentSegment::new as SegmentConstructorFn<CommentSegmentNewArgs>,
                CommentSegmentNewArgs { r#type: "block_comment", trim_start: None },
                Some(Box::new(
                    RegexLexer::new(
                        "newline",
                        r"\r\n|\n",
                        &NewlineSegment::new as SegmentConstructorFn<NewlineSegmentNewArgs>,
                        NewlineSegmentNewArgs {},
                        None,
                        None,
                    )
                    .unwrap(),
                )),
                Some(Box::new(
                    RegexLexer::new(
                        "whitespace",
                        r"[^\S\r\n]+",
                        &WhitespaceSegment::new as SegmentConstructorFn<WhitespaceSegmentNewArgs>,
                        WhitespaceSegmentNewArgs {},
                        None,
                        None,
                    )
                    .unwrap(),
                )),
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "single_quote",
                r"'([^'\\]|\\.|'')*'",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "single_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "double_quote",
                r#""([^"\\]|\\.)*""#,
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "double_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "back_quote",
                r"`[^`]*`",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "back_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "dollar_quote",
                // r"\$(\w*)\$[^\1]*?\$\1\$" is the original regex, but it doesn't work in Rust.
                r"\$(\w*)\$[^\$]*?\$\1\$",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "dollar_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap(),
        ),
        //
        // NOTE: Instead of using a created LiteralSegment and ComparisonOperatorSegment in the
        // next two, in Rust we just use a CodeSegment
        Box::new(
            RegexLexer::new(
                "numeric_literal",
                r"(?>\d+\.\d+|\d+\.(?![\.\w])|\.\d+|\d+)(\.?[eE][+-]?\d+)?((?<=\.)|(?=\b))",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "numeric_literal",
                    ..CodeSegmentNewArgs::default()
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "like_operator",
                r"!?~~?\*?",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs { code_type: "like_operator", ..CodeSegmentNewArgs::default() },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "newline",
                r"\r\n|\n",
                &NewlineSegment::new as SegmentConstructorFn<NewlineSegmentNewArgs>,
                NewlineSegmentNewArgs {},
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(StringLexer::new(
            "casting_operator",
            "::",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "casting_operator",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "equals",
            "=",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "equals",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "greater_than",
            ">",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "greater_than",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "less_than",
            "<",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "less_than",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "not",
            "!",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "not",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "dot",
            ".",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "dot",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "comma",
            ",",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "comma",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "plus",
            "+",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "plus",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "minus",
            "-",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "minus",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "divide",
            "/",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "divide",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "percent",
            "%",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "percent",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "question",
            "?",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "question",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "ampersand",
            "&",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "ampersand",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "vertical_bar",
            "|",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "vertical_bar",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "caret",
            "^",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "caret",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "star",
            "*",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "star",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "start_bracket",
            "(",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "start_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "end_bracket",
            ")",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "end_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "start_square_bracket",
            "[",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "start_square_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "end_square_bracket",
            "]",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "end_square_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "start_curly_bracket",
            "{",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "start_curly_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "end_curly_bracket",
            "}",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "end_curly_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "colon",
            ":",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "colon",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "semicolon",
            ";",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "semicolon",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        //         Numeric literal matches integers, decimals, and exponential formats,
        // Pattern breakdown:
        // (?>                      Atomic grouping
        //                          (https://www.regular-expressions.info/atomic.html).
        //     \d+\.\d+             e.g. 123.456
        //     |\d+\.(?![\.\w])     e.g. 123.
        //                          (N.B. negative lookahead assertion to ensure we
        //                          don't match range operators `..` in Exasol, and
        //                          that in bigquery we don't match the "."
        //                          in "asd-12.foo").
        //     |\.\d+               e.g. .456
        //     |\d+                 e.g. 123
        // )
        // (\.?[eE][+-]?\d+)?          Optional exponential.
        // (
        //     (?<=\.)              If matched character ends with . (e.g. 123.) then
        //                          don't worry about word boundary check.
        //     |(?=\b)              Check that we are at word boundary to avoid matching
        //                          valid naked identifiers (e.g. 123column).
        // )

        // This is the "fallback" lexer for anything else which looks like SQL.
        Box::new(
            RegexLexer::new(
                "word",
                "[0-9a-zA-Z_]+",
                &CodeSegment::new,
                CodeSegmentNewArgs { code_type: "word", ..<_>::default() },
                None,
                None,
            )
            .unwrap(),
        ),
    ]
}

/// A segment representing a whole file or script.
/// This is also the default "root" segment of the dialect,
/// and so is usually instantiated directly. It therefore
/// has no match_grammar.
#[derive(Hash, Default, Debug, Clone, PartialEq)]
pub struct FileSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl FileSegment {
    pub fn root_parse(
        &self,
        segments: &[Box<dyn Segment>],
        parse_context: &mut ParseContext,
        _f_name: Option<String>,
    ) -> Result<Box<dyn Segment>, SQLParseError> {
        // Trim the start
        let start_idx = segments.iter().position(|segment| segment.is_code()).unwrap_or(0);

        // Trim the end
        // Note: The '+ 1' in the end is to include the segment at 'end_idx' in the
        // slice.
        let end_idx =
            segments.iter().rposition(|segment| segment.is_code()).map_or(start_idx, |idx| idx + 1);

        if start_idx == end_idx {
            return Ok(Box::new(FileSegment { segments: segments.to_vec(), uuid: Uuid::new_v4() }));
        }

        let final_seg = segments.last().unwrap();
        assert!(final_seg.get_position_marker().is_some());

        let _closing_position = final_seg.get_position_marker().unwrap().templated_slice.end;

        let match_result = parse_context.progress_bar(|this| {
            // NOTE: Don't call .match() on the segment class itself, but go
            // straight to the match grammar inside.
            self.match_grammar()
                .unwrap()
                .match_segments(segments[start_idx..end_idx].to_vec(), this)
        })?;

        let has_match = match_result.has_match();
        let unmatched = match_result.unmatched_segments;

        let content: Vec<_> = if !has_match {
            unimplemented!()
        } else if !unmatched.is_empty() {
            unimplemented!()
        } else {
            chain(match_result.matched_segments, unmatched).collect()
        };

        let mut result = Vec::new();
        result.extend_from_slice(&segments[..start_idx]);
        result.extend(content);
        result.extend_from_slice(&segments[end_idx..]);

        Ok(Self { segments: result, uuid: Uuid::new_v4() }.boxed())
    }
}

impl Segment for FileSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        FileSegment { segments, uuid: self.uuid }.to_matchable()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Delimited::new(vec![Ref::new("StatementSegment").boxed()])
            .config(|this| {
                this.allow_trailing();
                this.delimiter(
                    AnyNumberOf::new(vec![Ref::new("DelimiterGrammar").boxed()])
                        .config(|config| config.max_times(1)),
                );
            })
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn class_types(&self) -> HashSet<String> {
        ["file"].map(ToOwned::to_owned).into_iter().collect()
    }
}

impl Matchable for FileSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// An interval expression segment.
#[derive(Hash, Debug, PartialEq, Clone)]
pub struct IntervalExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for IntervalExpressionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("INTERVAL").boxed(),
            one_of(vec![
                // The Numeric Version
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
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for IntervalExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Prefix for array literals specifying the type.
/// Often "ARRAY" or "ARRAY<type>"
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ArrayTypeSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ArrayTypeSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Nothing::new().to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ArrayTypeSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Array type with a size.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SizedArrayTypeSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for SizedArrayTypeSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("ArrayTypeSegment").boxed(),
            Ref::new("ArrayAccessorSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for SizedArrayTypeSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct UnorderedSelectStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for UnorderedSelectStatementSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").boxed(),
            Ref::new("FromClauseSegment").optional().boxed(),
            Ref::new("WhereClauseSegment").optional().boxed(),
        ])
        .terminators(vec![Ref::new("OrderByClauseSegment").boxed()])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for UnorderedSelectStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SelectClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for SelectClauseSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "select_clause"
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("SELECT").boxed(),
            Ref::new("SelectClauseModifierSegment").optional().boxed(),
            Delimited::new(vec![Ref::new("SelectClauseElementSegment").boxed()])
                .config(|this| this.allow_trailing())
                .boxed(),
        ])
        .terminators(vec![Ref::new("SelectClauseTerminatorGrammar").boxed()])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn class_types(&self) -> HashSet<String> {
        ["select_clause"].map(ToOwned::to_owned).into_iter().collect()
    }
}

impl Matchable for SelectClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a generic segment, to any of its child subsegments.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct StatementSegment {
    uuid: Uuid,
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for StatementSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { uuid: self.uuid, segments }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
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
        // .with_terminators(vec![Ref::new("DelimiterGrammar").boxed()])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn class_types(&self) -> HashSet<String> {
        ["statement"].map(ToOwned::to_owned).into_iter().collect()
    }
}

impl Matchable for StatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SetExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for SetExpressionSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![Ref::new("NonSetSelectableGrammar").boxed()]).to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for SetExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct FromClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for FromClauseSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("FROM").boxed(),
            Delimited::new(vec![Ref::new("FromExpressionSegment").boxed()]).boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for FromClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SelectStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for SelectStatementSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        SelectStatementSegment { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        UnorderedSelectStatementSegment { segments: Vec::new(), uuid: Uuid::new_v4() }
            .match_grammar()
            .unwrap()
            .copy(
                vec![Ref::new("OrderByClauseSegment").optional().to_matchable()].into(),
                true,
                Vec::new(),
            )
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for SelectStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SelectClauseModifierSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for SelectClauseModifierSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![Ref::keyword("DISTINCT").boxed(), Ref::keyword("ALL").boxed()])
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        let Some(position_marker) = position_marker else {
            return;
        };

        dbg!("self.position_marker = position_marker");
    }

    fn get_type(&self) -> &'static str {
        "select_clause_modifier"
    }
}

impl Matchable for SelectClauseModifierSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SelectClauseElementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for SelectClauseElementSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "select_clause_element"
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            // *, blah.*, blah.blah.*, etc.
            Ref::new("WildcardExpressionSegment").boxed(),
            Sequence::new(vec![
                Ref::new("BaseExpressionElementGrammar").boxed(),
                Ref::new("AliasExpressionSegment").optional().boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for SelectClauseElementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// A star (*) expression for a SELECT clause.
/// This is separate from the identifier to allow for
/// some dialects which extend this logic to allow
/// REPLACE, EXCEPT or similar clauses e.g. BigQuery.
#[derive(Hash, Debug, PartialEq, Clone)]
pub struct WildcardExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for WildcardExpressionSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            // *, blah.*, blah.blah.*, etc.
            Ref::new("WildcardIdentifierSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for WildcardExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Any identifier of the form a.b.*.
/// This inherits iter_raw_references from the
/// ObjectReferenceSegment.
#[derive(Hash, Debug, PartialEq, Clone)]
pub struct WildcardIdentifierSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for WildcardIdentifierSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
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
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for WildcardIdentifierSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, PartialEq, Clone)]
pub struct OrderByClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for OrderByClauseSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("ORDER").boxed(),
            Ref::keyword("BY").boxed(),
            // Indent::default().boxed(),
            Delimited::new(vec![one_of(vec![Ref::new("NumericLiteralSegment").boxed()]).boxed()])
                .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for OrderByClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// `TRUNCATE TABLE` statement.
#[derive(Hash, Debug, PartialEq, Clone)]
pub struct TruncateStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for TruncateStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("TRUNCATE").boxed(),
            Ref::keyword("TABLE").optional().boxed(),
            Ref::new("TableReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for TruncateStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// An expression, either arithmetic or boolean.
/// NB: This is potentially VERY recursive and
/// mostly uses the grammars above. This version
/// also doesn't bound itself first, and so is potentially
/// VERY SLOW. I don't really like this solution.
/// We rely on elements of the expression to bound
/// themselves rather than bounding at the expression
/// level. Trying to bound the ExpressionSegment itself
/// has been too unstable and not resilient enough to
/// other bugs.
#[derive(Hash, Debug, PartialEq, Clone)]
pub struct ExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ExpressionSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Ref::new("Expression_A_Grammar").to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for ExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct FromExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for FromExpressionSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        optionally_bracketed(vec![
            Sequence::new(vec![
                one_of(vec![Ref::new("FromExpressionElementSegment").boxed()]).boxed(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("JoinClauseSegment").boxed(),
                            Ref::new("JoinLikeClauseGrammar").boxed(),
                        ])
                        .config(|this| this.optional())
                        .boxed(),
                    ])
                    .boxed(),
                ])
                .boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for FromExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Clone, PartialEq, Debug)]
pub struct FromExpressionElementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for FromExpressionElementSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "from_expression_element"
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            optionally_bracketed(vec![Ref::new("TableExpressionSegment").boxed()]).boxed(),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec![Ref::new("FromClauseTerminatorGrammar").boxed()]))
                .optional()
                .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for FromExpressionElementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ColumnReferenceSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ColumnReferenceSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "column_reference"
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
            .config(|this| this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar")))
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for ColumnReferenceSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ObjectReferenceSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ObjectReferenceSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()])
            .config(|this| this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar")))
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for ObjectReferenceSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// An array accessor e.g. [3:4].
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ArrayAccessorSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ArrayAccessorSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
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
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ArrayAccessorSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}
/// Represents an array literal segment.
///
/// An unqualified array literal, e.g. [1, 2, 3]
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ArrayLiteralSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ArrayLiteralSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Bracketed::new(vec![
            Delimited::new(vec![Ref::new("BaseExpressionElementGrammar").boxed()])
                .config(|this| {
                    this.delimiter(Ref::new("CommaSegment"));
                    this.optional();
                })
                .boxed(),
        ])
        .config(|this| {
            this.bracket_type("square");
            this.parse_mode(ParseMode::Greedy);
        })
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ArrayLiteralSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a typed array literal segment.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct TypedArrayLiteralSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for TypedArrayLiteralSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("ArrayTypeSegment").boxed(),
            Ref::new("ArrayLiteralSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for TypedArrayLiteralSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a struct type segment (used in some SQL dialects like BigQuery).
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct StructTypeSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for StructTypeSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Nothing::new().to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for StructTypeSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a struct literal segment.
/// Example: (1, 2 as foo, 3)
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct StructLiteralSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for StructLiteralSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Bracketed::new(vec![
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::new("BaseExpressionElementGrammar").boxed(),
                    Ref::new("AliasExpressionSegment").optional().boxed(),
                ])
                .boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}
/// Represents a typed struct literal segment.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct TypedStructLiteralSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for TypedStructLiteralSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("StructTypeSegment").boxed(),
            Ref::new("StructLiteralSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for TypedStructLiteralSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an empty struct literal segment - `()`.
/// NOTE: This is only to set the right type so spacing rules are applied
/// correctly.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct EmptyStructLiteralBracketsSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for EmptyStructLiteralBracketsSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Bracketed::new(vec![]).to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

/// Represents an empty array literal segment - `STRUCT()`.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct EmptyStructLiteralSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for EmptyStructLiteralSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("StructTypeSegment").boxed(),
            Ref::new("EmptyStructLiteralBracketsSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for EmptyStructLiteralSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an object literal segment.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ObjectLiteralSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ObjectLiteralSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
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
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ObjectLiteralSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an object literal element segment.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ObjectLiteralElementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ObjectLiteralElementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("QuotedLiteralSegment").boxed(),
            Ref::new("ColonSegment").boxed(),
            Ref::new("BaseExpressionElementGrammar").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

/// Represents a time zone grammar segment.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct TimeZoneGrammar {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for TimeZoneGrammar {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        AnyNumberOf::new(vec![
            Sequence::new(vec![
                Ref::keyword("AT").boxed(),
                Ref::keyword("TIME").boxed(),
                Ref::keyword("ZONE").boxed(),
                Ref::new("ExpressionSegment").boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for TimeZoneGrammar {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a series of bracketed arguments.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct BracketedArguments {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for BracketedArguments {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Bracketed::new(vec![
            Delimited::new(vec![Ref::new("LiteralGrammar").boxed()])
                .config(|this| {
                    this.optional();
                })
                .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for BracketedArguments {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a data type segment.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DatatypeSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DatatypeSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            // Handles TIME and TIMESTAMP with optional precision and time zone specification
            Sequence::new(vec![
                one_of(vec![Ref::keyword("TIME").boxed(), Ref::keyword("TIMESTAMP").boxed()])
                    .boxed(),
                Bracketed::new(vec![Ref::new("NumericLiteralSegment").boxed()])
                    .config(|this| this.optional())
                    .boxed(),
                Sequence::new(vec![
                    one_of(vec![Ref::keyword("WITH").boxed(), Ref::keyword("WITHOUT").boxed()])
                        .boxed(),
                    Ref::keyword("TIME").boxed(),
                    Ref::keyword("ZONE").boxed(),
                ])
                .config(|this| this.optional())
                .boxed(),
            ])
            .boxed(),
            // DOUBLE PRECISION
            Sequence::new(vec![Ref::keyword("DOUBLE").boxed(), Ref::keyword("PRECISION").boxed()])
                .boxed(),
            // Character and binary varying types, and other data types with optional brackets
            Sequence::new(vec![
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("CHARACTER").boxed(),
                            Ref::keyword("BINARY").boxed(),
                        ])
                        .boxed(),
                        one_of(vec![
                            Ref::keyword("VARYING").boxed(),
                            Sequence::new(vec![
                                Ref::keyword("LARGE").boxed(),
                                Ref::keyword("OBJECT").boxed(),
                            ])
                            .boxed(),
                        ])
                        .boxed(),
                    ])
                    .boxed(),
                    Sequence::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").boxed(),
                            Ref::new("DotSegment").boxed(),
                        ])
                        .config(|this| this.optional())
                        .boxed(),
                        Ref::new("DatatypeIdentifierSegment").boxed(),
                    ])
                    .boxed(),
                ])
                .boxed(),
                Ref::new("BracketedArguments").optional().boxed(),
                one_of(vec![
                    // MySQL UNSIGNED
                    Ref::keyword("UNSIGNED").boxed(),
                    Ref::new("CharCharacterSetGrammar").boxed(),
                ])
                .config(|config| config.optional())
                .boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DatatypeSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// A reference to an object with an `AS` clause.
/// The optional AS keyword allows both implicit and explicit aliasing.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct AliasExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for AliasExpressionSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("AS").optional().boxed(),
            one_of(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar").boxed()]).boxed()])
                .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_type(&self) -> &'static str {
        "alias_expression"
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for AliasExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// A casting operation using '::'.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ShorthandCastSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ShorthandCastSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            one_of(vec![Ref::new("Expression_D_Grammar").boxed()]).boxed(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::new("CastOperatorSegment").boxed(),
                    Ref::new("DatatypeSegment").boxed(),
                ])
                .boxed(),
            ])
            .config(|this| this.max_times(1))
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ShorthandCastSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// A numeric literal with one + or - sign preceding.
/// The qualified numeric literal is a compound of a raw
/// literal and a plus/minus sign. We do it this way rather
/// than at the lexing step because the lexer doesn't deal
/// well with ambiguity.
#[derive(Hash, Debug, PartialEq, Clone)]
pub struct QualifiedNumericLiteralSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for QualifiedNumericLiteralSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("SignedSegmentGrammar").boxed(),
            Ref::new("NumericLiteralSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for QualifiedNumericLiteralSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an order by clause for an aggregate function.
/// Defined as a class to allow a specific type for rule AM06.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct AggregateOrderByClause {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for AggregateOrderByClause {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Ref::new("OrderByClauseSegment").to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for AggregateOrderByClause {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct FunctionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for FunctionSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec_of_erased![Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("FunctionNameSegment"),
                Bracketed::new(vec_of_erased![Ref::new("FunctionContentsGrammar").optional()])
            ]),
            Ref::new("PostFunctionGrammar").optional()
        ])])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_type(&self) -> &'static str {
        "function"
    }
}

impl Matchable for FunctionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct FunctionNameSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for FunctionNameSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "function_name"
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            // Project name, schema identifier, etc.
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("DotSegment")
            ])]),
            // Base function name
            one_of(vec_of_erased![Ref::new("FunctionNameIdentifierSegment")])
        ])
        .allow_gaps(false)
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn class_types(&self) -> HashSet<String> {
        HashSet::from(["function_name".into()])
    }
}

impl Matchable for FunctionNameSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, PartialEq, Clone)]
pub struct CaseExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CaseExpressionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("CASE").boxed(),
                AnyNumberOf::new(vec![Ref::new("WhenClauseSegment").boxed()]).boxed(),
                Ref::new("ElseClauseSegment").optional().boxed(),
                Ref::keyword("END").boxed(),
            ])
            .boxed(),
            Sequence::new(vec![]).boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CaseExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct WhenClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for WhenClauseSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("WHEN").boxed(),
            Ref::new("ExpressionSegment").boxed(),
            Ref::keyword("THEN").boxed(),
            Ref::new("ExpressionSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for WhenClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, PartialEq, Clone)]
pub struct ElseClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ElseClauseSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![Ref::keyword("ELSE").boxed(), Ref::new("ExpressionSegment").boxed()])
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ElseClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// A `WHERE` clause like in `SELECT` or `INSERT`.
#[derive(Hash, PartialEq, Clone, Debug)]
pub struct WhereClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for WhereClauseSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("WHERE").boxed(),
            // NOTE: The indent here is implicit to allow
            // constructions like:
            //    WHERE a
            //        AND b
            //
            // to be valid without forcing an indent between
            // "WHERE" and "a".
            optionally_bracketed(vec![Ref::new("ExpressionSegment").boxed()]).boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for WhereClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Clone, PartialEq, Debug)]
pub struct SetOperatorSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for SetOperatorSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            Ref::new("UnionGrammar").boxed(),
            Sequence::new(vec![
                one_of(vec![Ref::keyword("INTERSECT").boxed(), Ref::keyword("EXCEPT").boxed()])
                    .boxed(),
                Ref::keyword("ALL").optional().boxed(),
            ])
            .boxed(),
            Ref::keyword("MINUS").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for SetOperatorSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `VALUES` clause like in `INSERT`.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ValuesClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ValuesClauseSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            one_of(vec![Ref::keyword("VALUE").boxed(), Ref::keyword("VALUES").boxed()]).boxed(),
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
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ValuesClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a column definition for CREATE INDEX.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct IndexColumnDefinitionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for IndexColumnDefinitionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("SingleIdentifierGrammar").boxed(), // Column name
            one_of(vec![Ref::keyword("ASC").boxed(), Ref::keyword("DESC").boxed()])
                .config(|this| this.optional())
                .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for IndexColumnDefinitionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a bitwise and operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct BitwiseAndSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for BitwiseAndSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Ref::new("AmpersandSegment").to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for BitwiseAndSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a bitwise or operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct BitwiseOrSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for BitwiseOrSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Ref::new("PipeSegment").to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for BitwiseOrSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a bitwise left-shift operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct BitwiseLShiftSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for BitwiseLShiftSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("RawLessThanSegment").boxed(),
            Ref::new("RawLessThanSegment").boxed(),
        ])
        .allow_gaps(false)
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for BitwiseLShiftSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a bitwise right-shift operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct BitwiseRShiftSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for BitwiseRShiftSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("RawGreaterThanSegment").boxed(),
            Ref::new("RawGreaterThanSegment").boxed(),
        ])
        .allow_gaps(false)
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for BitwiseRShiftSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}
/// Represents a less than operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct LessThanSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for LessThanSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Ref::new("RawLessThanSegment").to_matchable().into()
    }
}

/// Represents a greater than or equal to operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct GreaterThanOrEqualToSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for GreaterThanOrEqualToSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("RawGreaterThanSegment").boxed(),
            Ref::new("RawEqualsSegment").boxed(),
        ])
        .allow_gaps(false)
        .to_matchable()
        .into()
    }
}

/// Represents a less than or equal to operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct LessThanOrEqualToSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for LessThanOrEqualToSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("RawLessThanSegment").boxed(),
            Ref::new("RawEqualsSegment").boxed(),
        ])
        .allow_gaps(false)
        .to_matchable()
        .into()
    }
}

/// Represents a not equal to operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct NotEqualToSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for NotEqualToSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
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
        .to_matchable()
        .into()
    }
}

/// Represents a concat operator.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ConcatSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ConcatSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![Ref::new("PipeSegment").boxed(), Ref::new("PipeSegment").boxed()])
            .allow_gaps(false)
            .to_matchable()
            .into()
    }

    fn get_type(&self) -> &'static str {
        "binary_operator"
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn class_types(&self) -> HashSet<String> {
        HashSet::from(["binary_operator".into()])
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for LessThanSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

impl Matchable for GreaterThanOrEqualToSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

impl Matchable for LessThanOrEqualToSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

impl Matchable for NotEqualToSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

impl Matchable for ConcatSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an expression to construct an ARRAY from a subquery.
/// This differs from an array literal in that it takes the form of an
/// expression.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ArrayExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ArrayExpressionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Nothing::new().to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ArrayExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents the `LOCAL.ALIAS` syntax, which allows using an alias name of a
/// column within clauses. A hookpoint for other dialects, e.g., Exasol.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct LocalAliasSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for LocalAliasSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Nothing::new().to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for LocalAliasSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `MERGE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct MergeStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for MergeStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("MergeIntoLiteralGrammar").boxed(),
            // Indent::new().boxed(),
            one_of(vec![
                Ref::new("TableReferenceSegment").boxed(),
                Ref::new("AliasedTableReferenceGrammar").boxed(),
            ])
            .boxed(),
            // Dedent::new().boxed(),
            Ref::keyword("USING").boxed(),
            // Indent::new().boxed(),
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
            // Dedent::new().boxed(),
            // Conditional::new(Indent::new(), true, "indented_using_on").boxed(),
            Ref::new("JoinOnConditionSegment").boxed(),
            // Conditional::new(Dedent::new(), true, "indented_using_on").boxed(),
            Ref::new("MergeMatchSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for MergeStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// An `INSERT` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct InsertStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for InsertStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("INSERT"),
            // Maybe OVERWRITE is just snowflake?
            // (It's also Hive but that has full insert grammar implementation)
            Ref::keyword("OVERWRITE").optional(),
            Ref::keyword("INTO"),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                // As SelectableGrammar can be bracketed too, the parse gets confused,
                // so we need slightly odd syntax here to allow those to parse (rather
                // than just add optional=True to BracketedColumnReferenceListGrammar).
                Ref::new("SelectableGrammar"),
                Sequence::new(vec_of_erased![
                    Ref::new("BracketedColumnReferenceListGrammar"),
                    Ref::new("SelectableGrammar")
                ]),
                Ref::new("DefaultValuesGrammar")
            ])
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for InsertStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `COMMIT`, `ROLLBACK`, or `TRANSACTION` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct TransactionStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for TransactionStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
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
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for TransactionStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP TABLE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropTableStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropTableStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("TABLE"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment")]),
            Ref::new("DropBehaviorGrammar").optional()
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropTableStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP VIEW` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropViewStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropViewStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("VIEW"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("DropBehaviorGrammar").optional()
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropViewStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE USER` statement.
/// A very simple create user syntax which can be extended by other dialects.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateUserStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateUserStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Some(
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("USER"),
                Ref::new("RoleReferenceSegment")
            ])
            .to_matchable(),
        )
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateUserStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP USER` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropUserStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropUserStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("USER"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("RoleReferenceSegment")
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropUserStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `GRANT` or `REVOKE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct AccessStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for AccessStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![Ref::keyword("GRANT")]),
            Sequence::new(vec_of_erased![Ref::keyword("REVOKE")])
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for AccessStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE TABLE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateTableStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateTableStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::new("TemporaryTransientGrammar").optional(),
            Ref::keyword("TABLE"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("TableConstraintSegment"),
                            Ref::new("ColumnDefinitionSegment")
                        ]
                    )])]),
                    Ref::new("CommentClauseSegment").optional()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIKE"),
                    Ref::new("TableReferenceSegment")
                ])
            ]),
            Ref::new("TableEndClauseSegment").optional()
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateTableStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE ROLE` statement.
/// A very simple create role syntax which can be extended by other dialects.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateRoleStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateRoleStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Some(
            Sequence::new(vec![
                Ref::keyword("CREATE").boxed(),
                Ref::keyword("ROLE").boxed(),
                Ref::new("RoleReferenceSegment").boxed(),
            ])
            .to_matchable(),
        )
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateRoleStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP ROLE` statement with CASCADE option.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropRoleStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropRoleStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("ROLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("SingleIdentifierGrammar")
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropRoleStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an `ALTER TABLE` statement.
/// Based loosely on MySQL's ALTER TABLE syntax.
/// TODO: Flesh this out with more detail.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct AlterTableStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for AlterTableStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Some(
            Sequence::new(vec![
                Ref::keyword("ALTER").boxed(),
                Ref::keyword("TABLE").boxed(),
                Ref::new("TableReferenceSegment").boxed(),
                Delimited::new(vec![Ref::new("AlterTableOptionsGrammar").boxed()]).boxed(),
            ])
            .to_matchable(),
        )
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for AlterTableStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE SCHEMA` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateSchemaStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateSchemaStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("CREATE").boxed(),
            Ref::keyword("SCHEMA").boxed(),
            Ref::new("IfNotExistsGrammar").optional().boxed(),
            Ref::new("SchemaReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateSchemaStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `SET SCHEMA` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SetSchemaStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for SetSchemaStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("SET").boxed(),
            Ref::keyword("SCHEMA").boxed(),
            Ref::new("IfNotExistsGrammar").optional().boxed(),
            Ref::new("SchemaReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for SetSchemaStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP SCHEMA` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropSchemaStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropSchemaStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("SCHEMA").boxed(),
            Ref::new("IfExistsGrammar").optional().boxed(),
            Ref::new("SchemaReferenceSegment").boxed(),
            Ref::new("DropBehaviorGrammar").optional().boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropSchemaStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP TYPE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropTypeStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropTypeStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("TYPE").boxed(),
            Ref::new("IfExistsGrammar").optional().boxed(),
            Ref::new("ObjectReferenceSegment").boxed(),
            Ref::new("DropBehaviorGrammar").optional().boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropTypeStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE DATABASE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateDatabaseStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateDatabaseStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("CREATE").boxed(),
            Ref::keyword("DATABASE").boxed(),
            Ref::new("IfNotExistsGrammar").optional().boxed(),
            Ref::new("DatabaseReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateDatabaseStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP DATABASE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropDatabaseStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropDatabaseStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("DATABASE").boxed(),
            Ref::new("IfExistsGrammar").optional().boxed(),
            Ref::new("DatabaseReferenceSegment").boxed(),
            Ref::new("DropBehaviorGrammar").optional().boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropDatabaseStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE INDEX` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateIndexStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateIndexStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("CREATE").boxed(),
            Ref::new("OrReplaceGrammar").optional().boxed(),
            Ref::keyword("UNIQUE").optional().boxed(),
            Ref::keyword("INDEX").boxed(),
            Ref::new("IfNotExistsGrammar").optional().boxed(),
            Ref::new("IndexReferenceSegment").boxed(),
            Ref::keyword("ON").boxed(),
            Ref::new("TableReferenceSegment").boxed(),
            Sequence::new(vec![
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("IndexColumnDefinitionSegment").boxed()]).boxed(),
                ])
                .boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateIndexStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP INDEX` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropIndexStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropIndexStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("INDEX").boxed(),
            Ref::new("IfExistsGrammar").optional().boxed(),
            Ref::new("IndexReferenceSegment").boxed(),
            Ref::new("DropBehaviorGrammar").optional().boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropIndexStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE VIEW` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateViewStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateViewStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::keyword("VIEW"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
            Ref::keyword("AS"),
            optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
            // Ref::new("WithNoSchemaBindingClauseSegment").optional().boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateViewStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DELETE` statement.
/// DELETE FROM <table name> [ WHERE <search condition> ]
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DeleteStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DeleteStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DELETE"),
            Ref::new("FromClauseSegment"),
            Ref::new("WhereClauseSegment").optional()
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DeleteStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an `Update` statement.
/// UPDATE <table name> SET <set clause list> [ WHERE <search condition> ]
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct UpdateStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for UpdateStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("UPDATE").boxed(),
            Ref::new("TableReferenceSegment").boxed(),
            Ref::new("AliasExpressionSegment").exclude(Ref::keyword("SET")).optional().boxed(),
            Ref::new("SetClauseListSegment").boxed(),
            Ref::new("FromClauseSegment").optional().boxed(),
            Ref::new("WhereClauseSegment").optional().boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for UpdateStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE CAST` statement.
/// Reference: https://jakewheat.github.io/sql-overview/sql-2016-foundation-grammar.html#_11_63_user_defined_cast_definition
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateCastStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateCastStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("CREATE").boxed(),
            Ref::keyword("CAST").boxed(),
            Bracketed::new(vec![
                Ref::new("DatatypeSegment").boxed(),
                Ref::keyword("AS").boxed(),
                Ref::new("DatatypeSegment").boxed(),
            ])
            .boxed(),
            Ref::keyword("WITH").boxed(),
            Ref::keyword("SPECIFIC").optional().boxed(),
            one_of(vec![
                Ref::keyword("ROUTINE").boxed(),
                Ref::keyword("FUNCTION").boxed(),
                Ref::keyword("PROCEDURE").boxed(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("INSTANCE").optional().boxed(),
                        Ref::keyword("STATIC").optional().boxed(),
                        Ref::keyword("CONSTRUCTOR").optional().boxed(),
                    ])
                    .boxed(),
                    Ref::keyword("METHOD").boxed(),
                ])
                .boxed(),
            ])
            .boxed(),
            Ref::new("FunctionNameSegment").boxed(),
            Ref::new("FunctionParameterListGrammar").optional().boxed(),
            Sequence::new(vec![
                Ref::keyword("FOR").boxed(),
                Ref::new("ObjectReferenceSegment").boxed(),
            ])
            .config(|this| this.optional())
            .boxed(),
            Sequence::new(vec![Ref::keyword("AS").boxed(), Ref::keyword("ASSIGNMENT").boxed()])
                .config(|this| this.optional())
                .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateCastStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP CAST` statement.
/// Reference: https://jakewheat.github.io/sql-overview/sql-2016-foundation-grammar.html#_11_64_drop_user_defined_cast_statement
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropCastStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropCastStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("CAST").boxed(),
            Bracketed::new(vec![
                Ref::new("DatatypeSegment").boxed(),
                Ref::keyword("AS").boxed(),
                Ref::new("DatatypeSegment").boxed(),
            ])
            .boxed(),
            Ref::new("DropBehaviorGrammar").optional().boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropCastStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE FUNCTION` statement.
/// This version in the ANSI dialect should be a "common subset" of the
/// structure of the code for those dialects.
/// Reference:
/// - PostgreSQL: https://www.postgresql.org/docs/9.1/sql-createfunction.html
/// - Snowflake: https://docs.snowflake.com/en/sql-reference/sql/create-function.html
/// - BigQuery: https://cloud.google.com/bigquery/docs/reference/standard-sql/user-defined-functions
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateFunctionStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateFunctionStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("CREATE").boxed(),
            Ref::new("OrReplaceGrammar").optional().boxed(),
            Ref::new("TemporaryGrammar").optional().boxed(),
            Ref::keyword("FUNCTION").boxed(),
            Ref::new("IfNotExistsGrammar").optional().boxed(),
            Ref::new("FunctionNameSegment").boxed(),
            Ref::new("FunctionParameterListGrammar").boxed(),
            Sequence::new(vec![
                Ref::keyword("RETURNS").boxed(),
                Ref::new("DatatypeSegment").boxed(),
            ])
            .config(|this| this.optional())
            .boxed(),
            Ref::new("FunctionDefinitionGrammar").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateFunctionStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP FUNCTION` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropFunctionStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropFunctionStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("FUNCTION").boxed(),
            Ref::new("IfExistsGrammar").optional().boxed(),
            Ref::new("FunctionNameSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropFunctionStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a BigQuery `CREATE MODEL` statement.
/// Reference: https://cloud.google.com/bigquery-ml/docs/reference/standard-sql/bigqueryml-syntax-create
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateModelStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateModelStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("CREATE").boxed(),
            Ref::new("OrReplaceGrammar").optional().boxed(),
            Ref::keyword("MODEL").boxed(),
            Ref::new("IfNotExistsGrammar").optional().boxed(),
            Ref::new("ObjectReferenceSegment").boxed(),
            Sequence::new(vec![
                Ref::keyword("OPTIONS").boxed(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("ParameterNameSegment").boxed(),
                            Ref::new("EqualsSegment").boxed(),
                            one_of(vec![
                                Ref::new("LiteralGrammar").boxed(), // Single value
                                Bracketed::new(vec![
                                    Delimited::new(vec![Ref::new("QuotedLiteralSegment").boxed()])
                                        .boxed(),
                                ])
                                .config(|this| {
                                    this.bracket_type("square");
                                    this.optional();
                                })
                                .boxed(),
                            ])
                            .boxed(),
                        ])
                        .boxed(),
                    ])
                    .boxed(),
                ])
                .boxed(),
            ])
            .config(|this| this.optional())
            .boxed(),
            Ref::keyword("AS").boxed(),
            Ref::new("SelectableGrammar").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateModelStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP MODEL` statement.
/// Reference: https://cloud.google.com/bigquery-ml/docs/reference/standard-sql/bigqueryml-syntax-drop-model
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropModelStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropModelStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("MODEL").boxed(),
            Ref::new("IfExistsGrammar").optional().boxed(),
            Ref::new("ObjectReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropModelStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `Describe` statement.
/// DESCRIBE <object type> <object name>
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DescribeStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DescribeStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DESCRIBE").boxed(),
            Ref::new("NakedIdentifierSegment").boxed(),
            Ref::new("ObjectReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DescribeStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `USE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct UseStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for UseStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("USE").boxed(),
            Ref::new("DatabaseReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for UseStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an `Explain` statement.
/// EXPLAIN explainable_stmt
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ExplainStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for ExplainStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("EXPLAIN").boxed(),
            one_of(vec![
                Ref::new("SelectableGrammar").boxed(),
                Ref::new("InsertStatementSegment").boxed(),
                Ref::new("UpdateStatementSegment").boxed(),
                Ref::new("DeleteStatementSegment").boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ExplainStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE SEQUENCE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateSequenceStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateSequenceStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("CREATE").boxed(),
            Ref::keyword("SEQUENCE").boxed(),
            Ref::new("SequenceReferenceSegment").boxed(),
            AnyNumberOf::new(vec![Ref::new("CreateSequenceOptionsSegment").boxed()])
                .config(|this| this.optional())
                .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateSequenceStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents an `ALTER SEQUENCE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct AlterSequenceStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for AlterSequenceStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("ALTER").boxed(),
            Ref::keyword("SEQUENCE").boxed(),
            Ref::new("SequenceReferenceSegment").boxed(),
            AnyNumberOf::new(vec![Ref::new("AlterSequenceOptionsSegment").boxed()]).boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for AlterSequenceStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP SEQUENCE` statement.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropSequenceStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropSequenceStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("SEQUENCE").boxed(),
            Ref::new("SequenceReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropSequenceStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `CREATE TRIGGER` statement.
/// Reference: https://www.postgresql.org/docs/14/sql-createtrigger.html
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CreateTriggerStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CreateTriggerStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("CREATE").boxed(),
            Ref::keyword("TRIGGER").boxed(),
            Ref::new("TriggerReferenceSegment").boxed(),
            one_of(vec![
                Ref::keyword("BEFORE").boxed(),
                Ref::keyword("AFTER").boxed(),
                Sequence::new(vec![Ref::keyword("INSTEAD").boxed(), Ref::keyword("OF").boxed()])
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
            AnyNumberOf::new(vec![
                // Implement remaining sequences...
            ])
            .boxed(),
            Sequence::new(vec![
                Ref::keyword("EXECUTE").boxed(),
                Ref::keyword("PROCEDURE").boxed(),
                Ref::new("FunctionNameIdentifierSegment").boxed(),
                Bracketed::new(vec![Ref::new("FunctionContentsGrammar").optional().boxed()])
                    .boxed(),
            ])
            .config(|this| this.optional())
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CreateTriggerStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// Represents a `DROP TRIGGER` statement.
/// Reference: https://www.postgresql.org/docs/14/sql-droptrigger.html
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct DropTriggerStatementSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for DropTriggerStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("DROP").boxed(),
            Ref::keyword("TRIGGER").boxed(),
            Ref::new("IfExistsGrammar").optional().boxed(),
            Ref::new("TriggerReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DropTriggerStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct TableExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for TableExpressionSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: Uuid::new_v4() }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            Ref::new("ValuesClauseSegment").boxed(),
            Ref::new("BareFunctionSegment").boxed(),
            Ref::new("FunctionSegment").boxed(),
            Ref::new("TableReferenceSegment").boxed(),
            // Nested Selects
            Bracketed::new(vec![Ref::new("SelectableGrammar").boxed()]).boxed(),
            Bracketed::new(vec![Ref::new("MergeStatementSegment").boxed()]).boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for TableExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct JoinClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for JoinClauseSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec_of_erased![Sequence::new(vec_of_erased![
            Ref::new("JoinTypeKeywordsGrammar").optional(),
            Ref::new("JoinKeywordsGrammar"),
            Ref::new("FromExpressionElementSegment"),
            AnyNumberOf::new(vec_of_erased![Ref::new("NestedJoinGrammar")]),
            Sequence::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("JoinOnConditionSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SingleIdentifierGrammar"
                    )])])
                ])
            ])])
            .config(|this| this.optional())
        ])])
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for JoinClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct JoinOnConditionSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for JoinOnConditionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ON"),
            optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")])
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for JoinOnConditionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct OverClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for OverClauseSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "over_clause"
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::new("IgnoreRespectNullsGrammar").optional(),
            Ref::keyword("OVER"),
            one_of(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Bracketed::new(vec_of_erased![Ref::new("WindowSpecificationSegment").optional()])
            ])
        ])
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for OverClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct WindowSpecificationSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for WindowSpecificationSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "window_specification"
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::new("SingleIdentifierGrammar").optional().exclude(Ref::keyword("PARTITION")),
            Ref::new("PartitionClauseSegment").optional(),
            Ref::new("OrderByClauseSegment").optional(),
            Ref::new("FrameClauseSegment").optional()
        ])
        .config(|this| this.optional())
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for WindowSpecificationSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct PartitionClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for PartitionClauseSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "partitionby_clause"
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("PARTITION"),
            Ref::keyword("BY"),
            optionally_bracketed(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "ExpressionSegment"
            )])])
        ])
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for PartitionClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct FrameClauseSegment {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for FrameClauseSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn get_type(&self) -> &'static str {
        "frame_clause"
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        let frame_extent = one_of(vec_of_erased![
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
        ]);

        Sequence::new(vec_of_erased![
            Ref::new("FrameClauseUnitGrammar"),
            one_of(vec_of_erased![
                frame_extent.clone(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("BETWEEN"),
                    frame_extent.clone(),
                    Ref::keyword("AND"),
                    frame_extent
                ])
            ])
        ])
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}

impl Matchable for FrameClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct WithCompoundStatementSegment {
    // Assuming other relevant fields based on your existing code
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for WithCompoundStatementSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WITH"),
            Ref::keyword("RECURSIVE").optional(),
            // Conditional::new(Indent::new(), "indented_ctes"),
            Delimited::new(vec_of_erased![Ref::new("CTEDefinitionSegment")])
                .config(|this| this.allow_trailing()),
            // Conditional::new(Dedent::new(), "indented_ctes"),
            one_of(vec_of_erased![
                Ref::new("NonWithSelectableGrammar"),
                Ref::new("NonWithNonSelectableGrammar")
            ])
        ])
        .to_matchable()
        .into()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_type(&self) -> &'static str {
        "with_compound_statement"
    }
}

impl Matchable for WithCompoundStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        // Assuming you have a macro or function similar to `from_segments!` in your
        // existing code
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct CTEDefinitionSegment {
    // Other relevant fields based on your existing code
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CTEDefinitionSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { segments, uuid: self.uuid }.boxed()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec_of_erased![
            Ref::new("SingleIdentifierGrammar"),
            Ref::new("CTEColumnList").optional(),
            Ref::keyword("AS").optional(),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")])
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_type(&self) -> &'static str {
        "common_table_expression"
    }
}

impl Matchable for CTEDefinitionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct CTEColumnList {
    segments: Vec<Box<dyn Segment>>,
    uuid: Uuid,
}

impl Segment for CTEColumnList {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for CTEColumnList {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::linter::Linter;
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::lexer::{Lexer, StringOrTemplate};
    use crate::core::parser::segments::test_functions::{fresh_ansi_dialect, lex};

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
            let config = FluffConfig::new(None, None, None, Some("ansi"));

            let lexer = Lexer::new(config, None);

            // Assume that the lex function returns a Result with tokens
            let tokens_result = lexer.lex(StringOrTemplate::String(raw.to_string()));

            // Check if lexing was successful, and if not, fail the test
            assert!(tokens_result.is_ok(), "Lexing failed for input: {}", raw);

            let (tokens, errors) = tokens_result.unwrap();
            assert_eq!(errors.len(), 0, "Lexing failed for input: {}", raw);

            // Check if the raw components of the tokens match the expected result
            let raw_list: Vec<String> =
                tokens.iter().map(|token| token.get_raw().unwrap()).collect();
            assert_eq!(raw_list, res, "Mismatch for input: {:?}", raw);

            // Check if the concatenated raw components of the tokens match the original raw
            // string
            let concatenated: String =
                tokens.iter().map(|token| token.get_raw().unwrap()).collect();
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
            ("ExpressionSegment", "DATE_ADD(CURRENT_DATE('America/New_York'), INTERVAL 1 year)"),
            // Array accessors
            ("ExpressionSegment", "my_array[1]"),
            ("ExpressionSegment", "my_array[OFFSET(1)]"),
            ("ExpressionSegment", "my_array[5:8]"),
            ("ExpressionSegment", "4 + my_array[OFFSET(1)]"),
            ("ExpressionSegment", "bits[OFFSET(0)] + 7"),
            (
                "SelectClauseElementSegment",
                ("(count_18_24 * bits[OFFSET(0)]) / audience_size AS relative_abundance"),
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
        ];

        for (segment_ref, sql_string) in cases {
            let dialect = fresh_ansi_dialect();
            let mut ctx = ParseContext::new(dialect.clone());

            let segment = dialect.r#ref(segment_ref);
            let mut segments = lex(sql_string);

            if segments.last().unwrap().get_type() == "end_of_file" {
                segments.pop();
            }

            let mut match_result = segment.match_segments(segments, &mut ctx).unwrap();

            assert_eq!(match_result.len(), 1, "failed {segment_ref}, {sql_string}");

            let parsed = match_result.matched_segments.pop().unwrap();

            assert_eq!(sql_string, parsed.get_raw().unwrap());
        }
    }

    #[test]
    fn test__dialect__ansi_specific_segment_not_match() {
        let cases = [("ObjectReferenceSegment", "\n     ")];

        let dialect = fresh_ansi_dialect();
        for (segment_ref, sql) in cases {
            let config = FluffConfig::new(None, None, None, None);
            let segments = lex(sql);

            let mut parse_cx = ParseContext::from_config(config);
            let segment = dialect.r#ref(segment_ref);

            let match_result = segment.match_segments(segments, &mut parse_cx).unwrap();
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
            let lnt = Linter::new(FluffConfig::new(None, None, None, None), None, None);
            let parsed = lnt.parse_string(raw.to_string(), None, None, None, None).unwrap();
            assert!(!parsed.violations.is_empty());

            let locs: Vec<(usize, usize)> =
                parsed.violations.iter().map(|v| (v.line_no, v.line_pos)).collect();
            assert_eq!(locs, err_locations);
        }
    }

    #[test]
    #[ignore = "WIP"]
    fn test__dialect__ansi_is_whitespace() {
        let lnt = Linter::new(FluffConfig::new(None, None, None, None), None, None);
        let file_content =
            std::fs::read_to_string("test/fixtures/dialects/ansi/select_in_multiline_comment.sql")
                .expect("Unable to read file");

        let parsed = lnt.parse_string(file_content, None, None, None, None).unwrap();

        #[allow(clippy::never_loop)]
        for _raw_seg in parsed.tree.unwrap().get_raw_segments() {
            unimplemented!()
            // if raw_seg.is_type("whitespace", "newline") {
            //     assert!(raw_seg.is_whitespace());
            // }
        }
    }

    #[test]
    fn test__dialect__ansi_parse_indented_joins() {
        let cases = [("select field_1 from my_table as alias_1",)];
        let lnt = Linter::new(FluffConfig::new(None, None, None, None), None, None);

        for (sql_string,) in cases {
            let parsed = lnt.parse_string(sql_string.to_string(), None, None, None, None).unwrap();
            dbg!(parsed.tree.unwrap().get_raw().unwrap());
        }
    }
}
