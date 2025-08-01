use itertools::Itertools;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::conditional::Conditional;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Nothing, Ref};
use sqruff_lib_core::parser::lexer::{Cursor, Matcher, Pattern};
use sqruff_lib_core::parser::lookahead::LookaheadExclude;
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTrait};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::bracketed::BracketedSegmentMatcher;
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::vec_of_erased;

use super::ansi_keywords::{ANSI_RESERVED_KEYWORDS, ANSI_UNRESERVED_KEYWORDS};

pub fn dialect() -> Dialect {
    raw_dialect().config(|this| this.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut ansi_dialect = Dialect::new();

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

    ansi_dialect
        .sets_mut("date_part_function_name")
        .extend(["DATEADD"]);

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
            (
                "square",
                "StartSquareBracketSegment",
                "EndSquareBracketSegment",
                false,
            ),
            (
                "curly",
                "StartCurlyBracketSegment",
                "EndCurlyBracketSegment",
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

    ansi_dialect.add([
        (
            "ArrayTypeSchemaSegment".into(),
            NodeMatcher::new(SyntaxKind::ArrayType, |_| Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "ObjectReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::ObjectReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    ansi_dialect.add([
        // Real segments
        (
            "DelimiterGrammar".into(),
            Ref::new("SemicolonSegment").to_matchable().into(),
        ),
        (
            "SemicolonSegment".into(),
            StringParser::new(";", SyntaxKind::StatementTerminator)
                .to_matchable()
                .into(),
        ),
        (
            "ColonSegment".into(),
            StringParser::new(":", SyntaxKind::Colon)
                .to_matchable()
                .into(),
        ),
        (
            "SliceSegment".into(),
            StringParser::new(":", SyntaxKind::Slice)
                .to_matchable()
                .into(),
        ),
        // NOTE: The purpose of the colon_delimiter is that it has different layout rules.
        // It assumes no whitespace on either side.
        (
            "ColonDelimiterSegment".into(),
            StringParser::new(":", SyntaxKind::ColonDelimiter)
                .to_matchable()
                .into(),
        ),
        (
            "StartBracketSegment".into(),
            StringParser::new("(", SyntaxKind::StartBracket)
                .to_matchable()
                .into(),
        ),
        (
            "EndBracketSegment".into(),
            StringParser::new(")", SyntaxKind::EndBracket)
                .to_matchable()
                .into(),
        ),
        (
            "StartSquareBracketSegment".into(),
            StringParser::new("[", SyntaxKind::StartSquareBracket)
                .to_matchable()
                .into(),
        ),
        (
            "EndSquareBracketSegment".into(),
            StringParser::new("]", SyntaxKind::EndSquareBracket)
                .to_matchable()
                .into(),
        ),
        (
            "StartCurlyBracketSegment".into(),
            StringParser::new("{", SyntaxKind::StartCurlyBracket)
                .to_matchable()
                .into(),
        ),
        (
            "EndCurlyBracketSegment".into(),
            StringParser::new("}", SyntaxKind::EndCurlyBracket)
                .to_matchable()
                .into(),
        ),
        (
            "CommaSegment".into(),
            StringParser::new(",", SyntaxKind::Comma)
                .to_matchable()
                .into(),
        ),
        (
            "DotSegment".into(),
            StringParser::new(".", SyntaxKind::Dot)
                .to_matchable()
                .into(),
        ),
        (
            "StarSegment".into(),
            StringParser::new("*", SyntaxKind::Star)
                .to_matchable()
                .into(),
        ),
        (
            "TildeSegment".into(),
            StringParser::new("~", SyntaxKind::Tilde)
                .to_matchable()
                .into(),
        ),
        (
            "ParameterSegment".into(),
            StringParser::new("?", SyntaxKind::Parameter)
                .to_matchable()
                .into(),
        ),
        (
            "CastOperatorSegment".into(),
            StringParser::new("::", SyntaxKind::CastingOperator)
                .to_matchable()
                .into(),
        ),
        (
            "PlusSegment".into(),
            StringParser::new("+", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "MinusSegment".into(),
            StringParser::new("-", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "PositiveSegment".into(),
            StringParser::new("+", SyntaxKind::SignIndicator)
                .to_matchable()
                .into(),
        ),
        (
            "NegativeSegment".into(),
            StringParser::new("-", SyntaxKind::SignIndicator)
                .to_matchable()
                .into(),
        ),
        (
            "DivideSegment".into(),
            StringParser::new("/", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "MultiplySegment".into(),
            StringParser::new("*", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "ModuloSegment".into(),
            StringParser::new("%", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "SlashSegment".into(),
            StringParser::new("/", SyntaxKind::Slash)
                .to_matchable()
                .into(),
        ),
        (
            "AmpersandSegment".into(),
            StringParser::new("&", SyntaxKind::Ampersand)
                .to_matchable()
                .into(),
        ),
        (
            "PipeSegment".into(),
            StringParser::new("|", SyntaxKind::Pipe)
                .to_matchable()
                .into(),
        ),
        (
            "BitwiseXorSegment".into(),
            StringParser::new("^", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "LikeOperatorSegment".into(),
            TypedParser::new(SyntaxKind::LikeOperator, SyntaxKind::ComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "RawNotSegment".into(),
            StringParser::new("!", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "RawEqualsSegment".into(),
            StringParser::new("=", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "RawGreaterThanSegment".into(),
            StringParser::new(">", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "RawLessThanSegment".into(),
            StringParser::new("<", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            // The following functions can be called without parentheses per ANSI specification
            "BareFunctionSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(
                    dialect
                        .sets("bare_functions")
                        .into_iter()
                        .map_into()
                        .collect_vec(),
                    SyntaxKind::BareFunction,
                )
                .to_matchable()
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
                let anti_template = format!("^({pattern})$");

                RegexParser::new("[A-Z0-9_]*[A-Z][A-Z0-9_]*", SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
            })
            .into(),
        ),
        (
            "ParameterNameSegment".into(),
            RegexParser::new(r#"\"?[A-Z][A-Z0-9_]*\"?"#, SyntaxKind::Parameter)
                .to_matchable()
                .into(),
        ),
        (
            "FunctionNameIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::Word, SyntaxKind::FunctionNameIdentifier)
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
                    RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::DataTypeIdentifier)
                        .anti_template(&anti_template)
                        .to_matchable(),
                    Ref::new("SingleIdentifierGrammar")
                        .exclude(Ref::new("NakedIdentifierSegment"))
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .into(),
        ),
        // Ansi Intervals
        (
            "DatetimeUnitSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(
                    dialect
                        .sets("datetime_units")
                        .into_iter()
                        .map_into()
                        .collect_vec(),
                    SyntaxKind::DatePart,
                )
                .to_matchable()
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
                        .map_into()
                        .collect::<Vec<_>>(),
                    SyntaxKind::FunctionNameIdentifier,
                )
                .to_matchable()
            })
            .into(),
        ),
        (
            "QuotedIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "SingleQuotedIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "NumericLiteralSegment".into(),
            TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral)
                .to_matchable()
                .into(),
        ),
        // NullSegment is defined separately to the keyword, so we can give it a different
        // type
        (
            "NullLiteralSegment".into(),
            StringParser::new("null", SyntaxKind::NullLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "NanLiteralSegment".into(),
            StringParser::new("nan", SyntaxKind::NullLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "TrueSegment".into(),
            StringParser::new("true", SyntaxKind::BooleanLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "FalseSegment".into(),
            StringParser::new("false", SyntaxKind::BooleanLiteral)
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
            one_of(vec_of_erased![
                Ref::new("TrueSegment"),
                Ref::new("FalseSegment")
            ])
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
            one_of(vec_of_erased![
                Ref::new("PositiveSegment"),
                Ref::new("NegativeSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StringBinaryOperatorGrammar".into(),
            one_of(vec![Ref::new("ConcatSegment").to_matchable()])
                .to_matchable()
                .into(),
        ),
        (
            "BooleanBinaryOperatorGrammar".into(),
            one_of(vec![
                Ref::new("AndOperatorGrammar").to_matchable(),
                Ref::new("OrOperatorGrammar").to_matchable(),
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
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral,)
            ])
            .to_matchable()
            .into(),
        ),
        // Hookpoint for other dialects
        // e.g. INTO is optional in BIGQUERY
        (
            "MergeIntoLiteralGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("MERGE").to_matchable(),
                Ref::keyword("INTO").to_matchable(),
            ])
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
            StringParser::new("AND", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "OrOperatorGrammar".into(),
            StringParser::new("OR", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "NotOperatorGrammar".into(),
            StringParser::new("NOT", SyntaxKind::Keyword)
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
                Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrReplaceGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("OR").to_matchable(),
                Ref::keyword("REPLACE").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TemporaryTransientGrammar".into(),
            one_of(vec![
                Ref::keyword("TRANSIENT").to_matchable(),
                Ref::new("TemporaryGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TemporaryGrammar".into(),
            one_of(vec![
                Ref::keyword("TEMP").to_matchable(),
                Ref::keyword("TEMPORARY").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IfExistsGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("IF").to_matchable(),
                Ref::keyword("EXISTS").to_matchable(),
            ])
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
                one_of(vec_of_erased![
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("ALL")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IsClauseGrammar".into(),
            one_of(vec![
                Ref::new("NullLiteralSegment").to_matchable(),
                Ref::new("NanLiteralSegment").to_matchable(),
                Ref::new("BooleanLiteralGrammar").to_matchable(),
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
            one_of(select_clause_terminators()).to_matchable().into(),
        ),
        // Define these as grammars to allow child dialects to enable them (since they are
        // non-standard keywords)
        ("IsNullGrammar".into(), Nothing::new().to_matchable().into()),
        (
            "NotNullGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "CollateGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "FromClauseTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("WHERE").to_matchable(),
                Ref::keyword("LIMIT").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("GROUP").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("HAVING").to_matchable(),
                Ref::keyword("QUALIFY").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::new("SetOperatorSegment").to_matchable(),
                Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
                Ref::new("WithDataClauseSegment").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("LIMIT").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("GROUP").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("HAVING").to_matchable(),
                Ref::keyword("QUALIFY").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::keyword("OVERLAPS").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseTerminatorGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("LIMIT").to_matchable(),
                Ref::keyword("HAVING").to_matchable(),
                Ref::keyword("QUALIFY").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseTerminatorGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("LIMIT").to_matchable(),
                Ref::keyword("QUALIFY").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrderByClauseTerminators".into(),
            one_of(vec![
                Ref::keyword("LIMIT").to_matchable(),
                Ref::keyword("HAVING").to_matchable(),
                Ref::keyword("QUALIFY").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::new("FrameClauseUnitGrammar").to_matchable(),
                Ref::keyword("SEPARATOR").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PrimaryKeyGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("PRIMARY").to_matchable(),
                Ref::keyword("KEY").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ForeignKeyGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("FOREIGN").to_matchable(),
                Ref::keyword("KEY").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "UniqueKeyGrammar".into(),
            Sequence::new(vec![Ref::keyword("UNIQUE").to_matchable()])
                .to_matchable()
                .into(),
        ),
        // Odd syntax, but prevents eager parameters being confused for data types
        (
            "FunctionParameterGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("ParameterNameSegment").optional().to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ANY").to_matchable(),
                            Ref::keyword("TYPE").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("ANY").to_matchable(),
                        Ref::keyword("TYPE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DatatypeSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AutoIncrementGrammar".into(),
            Sequence::new(vec![Ref::keyword("AUTO_INCREMENT").to_matchable()])
                .to_matchable()
                .into(),
        ),
        // Base Expression element is the right thing to reference for everything
        // which functions as an expression, but could include literals.
        (
            "BaseExpressionElementGrammar".into(),
            one_of(vec![
                Ref::new("LiteralGrammar").to_matchable(),
                Ref::new("BareFunctionSegment").to_matchable(),
                Ref::new("IntervalExpressionSegment").to_matchable(),
                Ref::new("FunctionSegment").to_matchable(),
                Ref::new("ColumnReferenceSegment").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::new("DatatypeSegment").to_matchable(),
                    Ref::new("LiteralGrammar").to_matchable(),
                ])
                .to_matchable(),
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
                Ref::keyword("FILTER").to_matchable(),
                Bracketed::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("WHERE").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IgnoreRespectNullsGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("IGNORE").to_matchable(),
                    Ref::keyword("RESPECT").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("NULLS").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FrameClauseUnitGrammar".into(),
            one_of(vec![
                Ref::keyword("ROWS").to_matchable(),
                Ref::keyword("RANGE").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "JoinTypeKeywordsGrammar".into(),
            one_of(vec![
                Ref::keyword("CROSS").to_matchable(),
                Ref::keyword("INNER").to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FULL").to_matchable(),
                        Ref::keyword("LEFT").to_matchable(),
                        Ref::keyword("RIGHT").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("OUTER").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable()
            .into(),
        ),
        (
            // It's as a sequence to allow to parametrize that in Postgres dialect with LATERAL
            "JoinKeywordsGrammar".into(),
            Sequence::new(vec![Ref::keyword("JOIN").to_matchable()])
                .to_matchable()
                .into(),
        ),
        (
            // NATURAL joins are not supported in all dialects (e.g. not in Bigquery
            // or T-SQL). So define here to allow override with Nothing() for those.
            "NaturalJoinKeywordsGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("NATURAL").to_matchable(),
                one_of(vec![
                    // Note: NATURAL joins do not support CROSS joins
                    Ref::keyword("INNER").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("LEFT").to_matchable(),
                            Ref::keyword("RIGHT").to_matchable(),
                            Ref::keyword("FULL").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("OUTER").optional().to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // This can be overwritten by dialects
        (
            "ExtendedNaturalJoinKeywordsGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "NestedJoinGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "ReferentialActionGrammar".into(),
            one_of(vec![
                Ref::keyword("RESTRICT").to_matchable(),
                Ref::keyword("CASCADE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::keyword("NULL").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("NO").to_matchable(),
                    Ref::keyword("ACTION").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::keyword("DEFAULT").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DropBehaviorGrammar".into(),
            one_of(vec![
                Ref::keyword("RESTRICT").to_matchable(),
                Ref::keyword("CASCADE").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable()
            .into(),
        ),
        (
            "ColumnConstraintDefaultGrammar".into(),
            one_of(vec![
                Ref::new("ShorthandCastSegment").to_matchable(),
                Ref::new("LiteralGrammar").to_matchable(),
                Ref::new("FunctionSegment").to_matchable(),
                Ref::new("BareFunctionSegment").to_matchable(),
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
                Ref::keyword("BOTH").to_matchable(),
                Ref::keyword("LEADING").to_matchable(),
                Ref::keyword("TRAILING").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DefaultValuesGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("DEFAULT").to_matchable(),
                Ref::keyword("VALUES").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ObjectReferenceDelimiterGrammar".into(),
            one_of(vec![
                Ref::new("DotSegment").to_matchable(),
                // NOTE: The double dot syntax allows for default values.
                Sequence::new(vec![
                    Ref::new("DotSegment").to_matchable(),
                    Ref::new("DotSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ObjectReferenceTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::keyword("USING").to_matchable(),
                Ref::new("CommaSegment").to_matchable(),
                Ref::new("CastOperatorSegment").to_matchable(),
                Ref::new("StartSquareBracketSegment").to_matchable(),
                Ref::new("StartBracketSegment").to_matchable(),
                Ref::new("BinaryOperatorGrammar").to_matchable(),
                Ref::new("ColonSegment").to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
                Ref::new("JoinLikeClauseGrammar").to_matchable(),
                Bracketed::new(vec![]).to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AlterTableDropColumnGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("COLUMN").optional(),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("SingleIdentifierGrammar"),
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
                    one_of(vec_of_erased![Sequence::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::keyword("FIRST"),
                            Ref::keyword("AFTER"),
                            Ref::new("ColumnReferenceSegment"),
                            // Bracketed Version of the same
                            Ref::new("BracketedColumnReferenceListGrammar")
                        ]
                    )])])
                    .config(|this| this.optional())
                ]),
                // Drop Column
                Ref::new("AlterTableDropColumnGrammar"),
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
            NodeMatcher::new(SyntaxKind::File, |_| {
                Delimited::new(vec![Ref::new("StatementSegment").to_matchable()])
                    .config(|this| {
                        this.allow_trailing();
                        this.delimiter(
                            AnyNumberOf::new(vec![Ref::new("DelimiterGrammar").to_matchable()])
                                .config(|config| config.min_times(1)),
                        );
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar")))
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::Expression, |_| {
                Ref::new("Expression_A_Grammar").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WildcardIdentifierSegment".into(),
            NodeMatcher::new(SyntaxKind::WildcardIdentifier, |_| {
                Sequence::new(vec![
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("ObjectReferenceDelimiterGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("StarSegment").to_matchable(),
                ])
                .allow_gaps(false)
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NamedWindowExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::NamedWindowExpression, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("AS"),
                    one_of(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Bracketed::new(vec_of_erased![Ref::new("WindowSpecificationSegment")])
                            .config(|this| this.parse_mode(ParseMode::Greedy)),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            // DateTimeFunctionContentsSegment(BaseSegment):
            //     """Datetime function contents."""
            //
            //     type = "function_contents"
            //
            //     match_grammar = Sequence(
            //         Bracketed(
            //             Delimited(
            //                 Ref("DatetimeUnitSegment"),
            //                 Ref(
            //                     "FunctionContentsGrammar",
            //                     # The brackets might be empty for some functions...
            //                     optional=True,
            //                 ),
            //             ),
            //         ),
            //     )
            "DateTimeFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_| {
                Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("DatetimeUnitSegment"),
                        Ref::new("FunctionContentsGrammar").optional()
                    ])
                ])])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::Function, |_| {
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("DatePartFunctionNameSegment"),
                        Ref::new("DateTimeFunctionContentsSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("FunctionNameSegment").exclude(one_of(vec_of_erased![
                                Ref::new("DatePartFunctionNameSegment"),
                                Ref::new("ValuesClauseSegment")
                            ])),
                            Ref::new("FunctionContentsSegment"),
                        ]),
                        Ref::new("PostFunctionGrammar").optional()
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::HavingClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("HAVING"),
                    MetaSegment::implicit_indent(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PathSegment".into(),
            NodeMatcher::new(SyntaxKind::PathSegment, |_| {
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("SlashSegment"),
                        Delimited::new(vec_of_erased![TypedParser::new(
                            SyntaxKind::Word,
                            SyntaxKind::PathSegment,
                        )])
                        .config(|this| {
                            this.allow_gaps = false;
                            this.delimiter(Ref::new("SlashSegment"));
                        }),
                    ]),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LimitClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::LimitClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CubeRollupClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::CubeRollupClause, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("CubeFunctionNameSegment"),
                        Ref::new("RollupFunctionNameSegment"),
                    ]),
                    Bracketed::new(vec_of_erased![Ref::new("GroupingExpressionList")]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RollupFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                StringParser::new("ROLLUP", SyntaxKind::FunctionNameIdentifier).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CubeFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                StringParser::new("CUBE", SyntaxKind::FunctionNameIdentifier).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "GroupingSetsClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::GroupingSetsClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("GROUPING"),
                    Ref::keyword("SETS"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Ref::new("CubeRollupClauseSegment"),
                        Ref::new("GroupingExpressionList"),
                    ])]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "GroupingExpressionList".into(),
            NodeMatcher::new(SyntaxKind::GroupingExpressionList, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SetClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FetchClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FetchClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("FETCH"),
                    one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("NEXT")]),
                    Ref::new("NumericLiteralSegment").optional(),
                    one_of(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("ROWS")]),
                    Ref::keyword("ONLY"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FunctionDefinitionGrammar".into(),
            NodeMatcher::new(SyntaxKind::FunctionDefinition, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    Ref::new("QuotedLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LANGUAGE"),
                        Ref::new("NakedIdentifierSegment")
                    ])
                    .config(|this| this.optional()),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AlterSequenceOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterSequenceOptionsSegment, |_| {
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
                    one_of(vec_of_erased![
                        Ref::keyword("CYCLE"),
                        Ref::keyword("NOCYCLE")
                    ]),
                    one_of(vec_of_erased![
                        Ref::keyword("ORDER"),
                        Ref::keyword("NOORDER")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RoleReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::RoleReference, |_| {
                Ref::new("SingleIdentifierGrammar").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TablespaceReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::TablespaceReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExtensionReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::ExtensionReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TagReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::TagReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnDefinition, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"), // Column name
                    Ref::new("DatatypeSegment"),         // Column type,
                    Bracketed::new(vec_of_erased![Anything::new()]).config(|this| this.optional()),
                    AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")])
                        .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CommentClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::CommentClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::new("QuotedLiteralSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableEndClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::TableEndClause, |_| {
                Nothing::new().to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeMatchSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeMatch, |_| {
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("MergeMatchedClauseSegment"),
                    Ref::new("MergeNotMatchedClauseSegment")
                ])
                .config(|this| this.min_times(1))
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeMatchedClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeWhenMatchedClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeNotMatchedClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeWhenNotMatchedClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeInsertClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeInsertClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    MetaSegment::indent(),
                    Ref::new("BracketedColumnReferenceListGrammar").optional(),
                    MetaSegment::dedent(),
                    Ref::new("ValuesClauseSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeUpdateClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeUpdateClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("UPDATE"),
                    MetaSegment::indent(),
                    Ref::new("SetClauseListSegment"),
                    MetaSegment::dedent(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeDeleteClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeDeleteClause, |_| {
                Ref::keyword("DELETE").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetClauseListSegment".into(),
            NodeMatcher::new(SyntaxKind::SetClauseList, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::TableReference, |ansi_dialect| {
                ansi_dialect
                    .grammar("ObjectReferenceSegment")
                    .match_grammar(ansi_dialect)
                    .unwrap()
                    .clone()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SchemaReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::TableReference, |_| {
                Ref::new("ObjectReferenceSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SingleIdentifierListSegment".into(),
            NodeMatcher::new(SyntaxKind::IdentifierList, |_| {
                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                    .config(|this| this.optional())
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::GroupbyClause, |_| {
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
                                    vec![Ref::new("GroupByClauseTerminatorGrammar").to_matchable()];
                            }),
                            MetaSegment::dedent()
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FrameClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FrameClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WithCompoundStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WithCompoundStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("RECURSIVE").optional(),
                    Conditional::new(MetaSegment::indent()).indented_ctes(),
                    Delimited::new(vec_of_erased![Ref::new("CTEDefinitionSegment")]).config(
                        |this| {
                            this.terminators = vec_of_erased![Ref::keyword("SELECT")];
                            this.allow_trailing();
                        }
                    ),
                    Conditional::new(MetaSegment::dedent()).indented_ctes(),
                    Ref::new("NonWithSelectableGrammar"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WithCompoundNonSelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WithCompoundStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("RECURSIVE").optional(),
                    Conditional::new(MetaSegment::indent()).indented_ctes(),
                    Delimited::new(vec_of_erased![Ref::new("CTEDefinitionSegment")]).config(
                        |this| {
                            this.terminators = vec_of_erased![Ref::keyword("SELECT")];
                            this.allow_trailing();
                        }
                    ),
                    Conditional::new(MetaSegment::dedent()).indented_ctes(),
                    Ref::new("NonWithNonSelectableGrammar"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CTEDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::CommonTableExpression, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("CTEColumnList").optional(),
                    Ref::keyword("AS").optional(),
                    Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CTEColumnList".into(),
            NodeMatcher::new(SyntaxKind::CTEColumnList, |_| {
                Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SequenceReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TriggerReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::TriggerReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::TableConstraint, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "JoinOnConditionSegment".into(),
            NodeMatcher::new(SyntaxKind::JoinOnCondition, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Conditional::new(MetaSegment::implicit_indent()).indented_on_contents(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    Conditional::new(MetaSegment::dedent()).indented_on_contents()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatabaseReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::DatabaseReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "IndexReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::DatabaseReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.disallow_gaps();
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CollationReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::CollationReference, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "OverClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OverClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NamedWindowSegment".into(),
            NodeMatcher::new(SyntaxKind::NamedWindow, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WINDOW"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Ref::new("NamedWindowExpressionSegment")]),
                    MetaSegment::dedent(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WindowSpecificationSegment".into(),
            NodeMatcher::new(SyntaxKind::WindowSpecification, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar")
                        .optional()
                        .exclude(one_of(vec_of_erased![
                            Ref::keyword("PARTITION"),
                            Ref::keyword("ORDER")
                        ])),
                    Ref::new("PartitionClauseSegment").optional(),
                    Ref::new("OrderByClauseSegment").optional(),
                    Ref::new("FrameClauseSegment").optional()
                ])
                .config(|this| this.optional())
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PartitionClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::PartitionbyClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARTITION"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    optionally_bracketed(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ExpressionSegment"
                    )])]),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "JoinClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::JoinClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTriggerStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("TRIGGER"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TriggerReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SampleExpression, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESAMPLE"),
                    one_of(vec_of_erased![
                        Ref::keyword("BERNOULLI"),
                        Ref::keyword("SYSTEM")
                    ]),
                    Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("REPEATABLE"),
                        Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableExpression, |_| {
                one_of(vec_of_erased![
                    Ref::new("ValuesClauseSegment"),
                    Ref::new("BareFunctionSegment"),
                    Ref::new("FunctionSegment"),
                    Ref::new("TableReferenceSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
                    Bracketed::new(vec_of_erased![Ref::new("MergeStatementSegment")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTriggerStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("TRIGGER"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TriggerReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SampleExpression, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESAMPLE"),
                    one_of(vec_of_erased![
                        Ref::keyword("BERNOULLI"),
                        Ref::keyword("SYSTEM")
                    ]),
                    Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("REPEATABLE"),
                        Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableExpression, |_| {
                one_of(vec_of_erased![
                    Ref::new("ValuesClauseSegment"),
                    Ref::new("BareFunctionSegment"),
                    Ref::new("FunctionSegment"),
                    Ref::new("TableReferenceSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
                    Bracketed::new(vec_of_erased![Ref::new("MergeStatementSegment")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTriggerStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("TRIGGER").to_matchable(),
                    Ref::new("TriggerReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("BEFORE").to_matchable(),
                        Ref::keyword("AFTER").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INSTEAD").to_matchable(),
                            Ref::keyword("OF").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Delimited::new(vec![
                        Ref::keyword("INSERT").to_matchable(),
                        Ref::keyword("DELETE").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("UPDATE").to_matchable(),
                            Ref::keyword("OF").to_matchable(),
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                //.with_terminators(vec!["OR", "ON"])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.delimiter(Ref::keyword("OR"));
                        // .with_terminators(vec!["ON"]);
                    })
                    .to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
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
                            one_of(vec_of_erased![
                                Ref::keyword("ROW"),
                                Ref::keyword("STATEMENT"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WHEN"),
                            Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                        ]),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("EXECUTE").to_matchable(),
                        Ref::keyword("PROCEDURE").to_matchable(),
                        Ref::new("FunctionNameIdentifierSegment").to_matchable(),
                        Ref::new("FunctionContentsSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropModelStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropModelStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("MODEL"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DescribeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DescribeStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DESCRIBE"),
                    Ref::new("NakedIdentifierSegment"),
                    Ref::new("ObjectReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UseStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("USE"),
                    Ref::new("DatabaseReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExplainStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ExplainStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXPLAIN"),
                    one_of(vec_of_erased![
                        Ref::new("SelectableGrammar"),
                        Ref::new("InsertStatementSegment"),
                        Ref::new("UpdateStatementSegment"),
                        Ref::new("DeleteStatementSegment")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateSequenceStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSequenceStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("SEQUENCE"),
                    Ref::new("SequenceReferenceSegment"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("CreateSequenceOptionsSegment")])
                        .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateSequenceOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSequenceOptionsSegment, |_| {
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
                    one_of(vec_of_erased![
                        Ref::keyword("CYCLE"),
                        Ref::keyword("NOCYCLE")
                    ]),
                    one_of(vec_of_erased![
                        Ref::keyword("ORDER"),
                        Ref::keyword("NOORDER")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AlterSequenceStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterSequenceStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("SEQUENCE"),
                    Ref::new("SequenceReferenceSegment"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("AlterSequenceOptionsSegment")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropSequenceStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropSequenceStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("SEQUENCE"),
                    Ref::new("SequenceReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropCastStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropCastStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateFunctionStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropFunctionStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("FUNCTION"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("FunctionNameSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateModelStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateModelStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateViewStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DELETE"),
                    Ref::new("FromClauseSegment"),
                    Ref::new("WhereClauseSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UpdateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UpdateStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("UPDATE"),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("AliasExpressionSegment")
                        .exclude(Ref::keyword("SET"))
                        .optional(),
                    Ref::new("SetClauseListSegment"),
                    Ref::new("FromClauseSegment").optional(),
                    Ref::new("WhereClauseSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateCastStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateCastStatement, |_| {
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
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::keyword("ASSIGNMENT")
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateRoleStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateRoleStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("ROLE"),
                    Ref::new("RoleReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropRoleStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropRoleStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("ROLE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AlterTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Delimited::new(vec_of_erased![Ref::new("AlterTableOptionsGrammar")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetSchemaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetSchemaStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("SchemaReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropSchemaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropSchemaStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SchemaReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropTypeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTypeStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("TYPE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateDatabaseStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("DATABASE"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("DatabaseReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropDatabaseStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("DATABASE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("DatabaseReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FunctionParameterListGrammar".into(),
            NodeMatcher::new(SyntaxKind::FunctionParameterList, |_| {
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("FunctionParameterGrammar")])
                        .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateIndexStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropIndexStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("INDEX"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("IndexReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AccessStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AccessStatement, |_| {
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
                            Ref::new("AccessStatementSegmentGrantRoleWithOptionGrammar").optional(),
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TransactionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::TransactionStatement, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("START"),
                        Ref::keyword("BEGIN"),
                        Ref::keyword("COMMIT"),
                        Ref::keyword("ROLLBACK"),
                        Ref::keyword("END")
                    ]),
                    one_of(vec_of_erased![
                        Ref::keyword("TRANSACTION"),
                        Ref::keyword("WORK")
                    ])
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
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::new("TemporaryGrammar").optional(),
                    Ref::keyword("TABLE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment")]),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropViewStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("VIEW"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateUserStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateUserStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("USER"),
                    Ref::new("RoleReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropUserStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropUserStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("USER"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("RoleReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NotEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("RawNotSegment").to_matchable(),
                        Ref::new("RawEqualsSegment").to_matchable(),
                    ])
                    .allow_gaps(false)
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("RawLessThanSegment").to_matchable(),
                        Ref::new("RawGreaterThanSegment").to_matchable(),
                    ])
                    .allow_gaps(false)
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ConcatSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Sequence::new(vec![
                    Ref::new("PipeSegment").to_matchable(),
                    Ref::new("PipeSegment").to_matchable(),
                ])
                .allow_gaps(false)
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ArrayExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::ArrayExpression, |_| {
                Nothing::new().to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LocalAliasSegment".into(),
            NodeMatcher::new(SyntaxKind::LocalAlias, |_| Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "MergeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeStatement, |_| {
                Sequence::new(vec![
                    Ref::new("MergeIntoLiteralGrammar").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    one_of(vec![
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::new("AliasedTableReferenceGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("USING").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    one_of(vec![
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::new("AliasedTableReferenceGrammar").to_matchable(),
                        Sequence::new(vec![
                            Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
                                .to_matchable(),
                            Ref::new("AliasExpressionSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Conditional::new(MetaSegment::indent())
                        .indented_using_on()
                        .to_matchable(),
                    Ref::new("JoinOnConditionSegment").to_matchable(),
                    Conditional::new(MetaSegment::dedent())
                        .indented_using_on()
                        .to_matchable(),
                    Ref::new("MergeMatchSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "IndexColumnDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::IndexColumnDefinition, |_| {
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(), // Column name
                    one_of(vec![
                        Ref::keyword("ASC").to_matchable(),
                        Ref::keyword("DESC").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BitwiseAndSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Ref::new("AmpersandSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BitwiseOrSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Ref::new("PipeSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BitwiseLShiftSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec![
                    Ref::new("RawLessThanSegment").to_matchable(),
                    Ref::new("RawLessThanSegment").to_matchable(),
                ])
                .allow_gaps(false)
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BitwiseRShiftSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec![
                    Ref::new("RawGreaterThanSegment").to_matchable(),
                    Ref::new("RawGreaterThanSegment").to_matchable(),
                ])
                .allow_gaps(false)
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LessThanSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Ref::new("RawLessThanSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "GreaterThanOrEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec![
                    Ref::new("RawGreaterThanSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .allow_gaps(false)
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LessThanOrEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec![
                    Ref::new("RawLessThanSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .allow_gaps(false)
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "EqualsSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Ref::new("RawEqualsSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "GreaterThanSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Ref::new("RawGreaterThanSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "QualifiedNumericLiteralSegment".into(),
            NodeMatcher::new(SyntaxKind::NumericLiteral, |_| {
                Sequence::new(vec![
                    Ref::new("SignedSegmentGrammar").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AggregateOrderByClause".into(),
            NodeMatcher::new(SyntaxKind::AggregateOrderByClause, |_| {
                Ref::new("OrderByClauseSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CaseExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::CaseExpression, |_| {
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CASE"),
                        MetaSegment::implicit_indent(),
                        AnyNumberOf::new(vec_of_erased![Ref::new("WhenClauseSegment")],).config(
                            |this| {
                                this.reset_terminators = true;
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
                                this.reset_terminators = true;
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
                    this.terminators = vec_of_erased![
                        Ref::new("ComparisonOperatorGrammar"),
                        Ref::new("CommaSegment"),
                        Ref::new("BinaryOperatorGrammar")
                    ]
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WhenClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WhenClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ElseClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ElseClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("ELSE").to_matchable(),
                    MetaSegment::implicit_indent().to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WhereClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHERE"),
                    MetaSegment::implicit_indent(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::SetOperator, |_| {
                one_of(vec_of_erased![
                    Ref::new("UnionGrammar"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("INTERSECT"),
                            Ref::keyword("EXCEPT")
                        ]),
                        Ref::keyword("ALL").optional(),
                    ]),
                    Ref::keyword("MINUS"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ValuesClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ValuesClause, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("VALUE").to_matchable(),
                        Ref::keyword("VALUES").to_matchable(),
                    ])
                    .to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("ROW").optional().to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::keyword("DEFAULT").to_matchable(),
                                    Ref::new("LiteralGrammar").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.parse_mode(ParseMode::Greedy))
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "EmptyStructLiteralSegment".into(),
            NodeMatcher::new(SyntaxKind::EmptyStructLiteral, |_| {
                Sequence::new(vec![
                    Ref::new("StructTypeSegment").to_matchable(),
                    Ref::new("EmptyStructLiteralBracketsSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ObjectLiteralSegment".into(),
            NodeMatcher::new(SyntaxKind::ObjectLiteral, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ObjectLiteralElementSegment").to_matchable()])
                        .config(|this| {
                            this.optional();
                        })
                        .to_matchable(),
                ])
                .config(|this| {
                    this.bracket_type("curly");
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ObjectLiteralElementSegment".into(),
            NodeMatcher::new(SyntaxKind::ObjectLiteralElement, |_| {
                Sequence::new(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("ColonSegment").to_matchable(),
                    Ref::new("BaseExpressionElementGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TimeZoneGrammar".into(),
            NodeMatcher::new(SyntaxKind::TimeZoneGrammar, |_| {
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("AT").to_matchable(),
                        Ref::keyword("TIME").to_matchable(),
                        Ref::keyword("ZONE").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BracketedArguments".into(),
            NodeMatcher::new(SyntaxKind::BracketedArguments, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("LiteralGrammar").to_matchable()])
                        .config(|this| {
                            this.optional();
                        })
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(SyntaxKind::DataType, |_| {
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("TIME"),
                            Ref::keyword("TIMESTAMP")
                        ]),
                        Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")])
                            .config(|this| this.optional()),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("WITH"),
                                Ref::keyword("WITHOUT")
                            ]),
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AliasExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::AliasExpression, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ShorthandCastSegment".into(),
            NodeMatcher::new(SyntaxKind::CastExpression, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ArrayAccessorSegment".into(),
            NodeMatcher::new(SyntaxKind::ArrayAccessor, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.delimiter(Ref::new("SliceSegment")))
                    .to_matchable(),
                ])
                .config(|this| {
                    this.bracket_type("square");
                    this.parse_mode(ParseMode::Greedy);
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ArrayLiteralSegment".into(),
            NodeMatcher::new(SyntaxKind::ArrayLiteral, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TypedArrayLiteralSegment".into(),
            NodeMatcher::new(SyntaxKind::TypedArrayLiteral, |_| {
                Sequence::new(vec![
                    Ref::new("ArrayTypeSegment").to_matchable(),
                    Ref::new("ArrayLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "StructTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::StructType, |_| Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "StructLiteralSegment".into(),
            NodeMatcher::new(SyntaxKind::StructLiteral, |_| {
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("BaseExpressionElementGrammar"),
                        Ref::new("AliasExpressionSegment").optional(),
                    ])
                ])])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TypedStructLiteralSegment".into(),
            NodeMatcher::new(SyntaxKind::TypedStructLiteral, |_| {
                Sequence::new(vec![
                    Ref::new("StructTypeSegment").to_matchable(),
                    Ref::new("StructLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "IntervalExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::IntervalExpression, |_| {
                Sequence::new(vec![
                    Ref::keyword("INTERVAL").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            one_of(vec![
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                                Ref::new("DatetimeUnitSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ArrayTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::ArrayType, |_| Nothing::new().to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "SizedArrayTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::SizedArrayType, |_| {
                Sequence::new(vec![
                    Ref::new("ArrayTypeSegment").to_matchable(),
                    Ref::new("ArrayAccessorSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UnorderedSelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, |_| {
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
            })
            .to_matchable()
            .into(),
        ),
        (
            "OverlapsClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OverlapsClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        ("SelectClauseSegment".into(), {
            NodeMatcher::new(SyntaxKind::SelectClause, |_| select_clause_segment())
                .to_matchable()
                .into()
        }),
        (
            "StatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, |_| statement_segment())
                .to_matchable()
                .into(),
        ),
        (
            "WithNoSchemaBindingClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WithNoSchemaBindingClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("NO"),
                    Ref::keyword("SCHEMA"),
                    Ref::keyword("BINDING"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WithDataClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WithDataClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Sequence::new(vec_of_erased![Ref::keyword("NO")])
                        .config(|this| this.optional()),
                    Ref::keyword("DATA"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SetExpression, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FromClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FromClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("FROM"),
                    Delimited::new(vec_of_erased![Ref::new("FromExpressionSegment")]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "EmptyStructLiteralBracketsSegment".into(),
            NodeMatcher::new(SyntaxKind::EmptyStructLiteralBrackets, |_| {
                Bracketed::new(vec![]).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WildcardExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::WildcardExpression, |_| {
                wildcard_expression_segment()
            })
            .to_matchable()
            .into(),
        ),
        (
            "OrderByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OrderbyClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TruncateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::TruncateStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("TRUNCATE").to_matchable(),
                    Ref::keyword("TABLE").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FromExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::FromExpression, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatePartFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                Ref::new("DatePartFunctionName").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FromExpressionElementSegment".into(),
            NodeMatcher::new(SyntaxKind::FromExpressionElement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("PreTableFunctionKeywordsGrammar").optional(),
                    optionally_bracketed(vec_of_erased![Ref::new("TableExpressionSegment")]),
                    Ref::new("AliasExpressionSegment")
                        .exclude(one_of(vec_of_erased![
                            Ref::new("FromClauseTerminatorGrammar"),
                            Ref::new("SamplingExpressionSegment"),
                            Ref::new("JoinLikeClauseGrammar"),
                            LookaheadExclude::new("WITH", "(")
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, |_| select_statement())
                .to_matchable()
                .into(),
        ),
        (
            "CreateSchemaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSchemaStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("SchemaReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseModifierSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClauseModifier, |_| {
                one_of(vec![
                    Ref::keyword("DISTINCT").to_matchable(),
                    Ref::keyword("ALL").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseElementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClauseElement, |_| select_clause_element())
                .to_matchable()
                .into(),
        ),
    ]);

    // hookpoint
    ansi_dialect.add([(
        "CharCharacterSetGrammar".into(),
        Nothing::new().to_matchable().into(),
    )]);

    // This is a hook point to allow subclassing for other dialects
    ansi_dialect.add([(
        "AliasedTableReferenceGrammar".into(),
        Sequence::new(vec_of_erased![
            Ref::new("TableReferenceSegment"),
            Ref::new("AliasExpressionSegment"),
        ])
        .to_matchable()
        .into(),
    )]);

    ansi_dialect.add([
        (
            "FunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_| {
                Sequence::new(vec![
                    Bracketed::new(vec![Ref::new("FunctionContentsGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // FunctionContentsExpressionGrammar intended as a hook to override in other dialects.
        (
            "FunctionContentsExpressionGrammar".into(),
            Ref::new("ExpressionSegment").to_matchable().into(),
        ),
        (
            "FunctionContentsGrammar".into(),
            AnyNumberOf::new(vec![
                Ref::new("ExpressionSegment").to_matchable(),
                // A Cast-like function
                Sequence::new(vec![
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("DatatypeSegment").to_matchable(),
                ])
                .to_matchable(),
                // Trim function
                Sequence::new(vec![
                    Ref::new("TrimParametersGrammar").to_matchable(),
                    Ref::new("ExpressionSegment")
                        .optional()
                        .exclude(Ref::keyword("FROM"))
                        .to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
                // An extract-like or substring-like function
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("DatetimeUnitSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    // Allow an optional distinct keyword here.
                    Ref::keyword("DISTINCT").optional().to_matchable(),
                    one_of(vec![
                        // For COUNT(*) or similar
                        Ref::new("StarSegment").to_matchable(),
                        Delimited::new(vec![
                            Ref::new("FunctionContentsExpressionGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("AggregateOrderByClause").to_matchable(), // Used in various functions
                Sequence::new(vec![
                    Ref::keyword("SEPARATOR").to_matchable(),
                    Ref::new("LiteralGrammar").to_matchable(),
                ])
                .to_matchable(),
                // Position-like function
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("IN").to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("IgnoreRespectNullsGrammar").to_matchable(),
                Ref::new("IndexColumnDefinitionSegment").to_matchable(),
                Ref::new("EmptyStructLiteralSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PostFunctionGrammar".into(),
            one_of(vec![
                Ref::new("OverClauseSegment").to_matchable(),
                Ref::new("FilterClauseGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Assuming `ansi_dialect` is an instance of a struct representing a SQL dialect
    // and `add_grammar` is a method to add a new grammar rule to the dialect.
    ansi_dialect.add([(
        "JoinLikeClauseGrammar".into(),
        Nothing::new().to_matchable().into(),
    )]);

    ansi_dialect.add([
        (
            "AccessStatementSegmentGrantRoleWithOptionGrammar".into(),
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
            .to_matchable()
            .into(),
        ),
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
                        Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
                    ]))
                    .to_matchable(),
                Ref::new("TildeSegment").to_matchable(),
                Ref::new("NotOperatorGrammar").to_matchable(),
                // Used in CONNECT BY clauses (EXASOL, Snowflake, Postgres...)
                Ref::keyword("PRIOR").to_matchable(),
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
                Ref::new("Tail_Recurse_Expression_A_Grammar").to_matchable(),
                AnyNumberOf::new(vec![
                    one_of(vec![
                        // Like grammar with NOT and optional ESCAPE
                        Sequence::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("NOT").optional().to_matchable(),
                                Ref::new("LikeGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("Expression_A_Grammar").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ESCAPE").to_matchable(),
                                Ref::new("Tail_Recurse_Expression_A_Grammar").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Binary operator grammar
                        Sequence::new(vec![
                            Ref::new("BinaryOperatorGrammar").to_matchable(),
                            Ref::new("Tail_Recurse_Expression_A_Grammar").to_matchable(),
                        ])
                        .to_matchable(),
                        // IN grammar
                        Ref::new("InOperatorGrammar").to_matchable(),
                        // IS grammar
                        Sequence::new(vec![
                            Ref::keyword("IS").to_matchable(),
                            Ref::keyword("NOT").optional().to_matchable(),
                            Ref::new("IsClauseGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        // IS NULL and NOT NULL grammars
                        Ref::new("IsNullGrammar").to_matchable(),
                        Ref::new("NotNullGrammar").to_matchable(),
                        // COLLATE grammar
                        Ref::new("CollateGrammar").to_matchable(),
                        // BETWEEN grammar
                        Sequence::new(vec![
                            Ref::keyword("NOT").optional().to_matchable(),
                            Ref::keyword("BETWEEN").to_matchable(),
                            Ref::new("Expression_B_Grammar").to_matchable(),
                            Ref::keyword("AND").to_matchable(),
                            Ref::new("Tail_Recurse_Expression_A_Grammar").to_matchable(),
                        ])
                        .to_matchable(),
                        // Additional sequences and grammar rules can be added here
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
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
                        Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
                    ]))
                    .to_matchable(),
                Ref::new("TildeSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Tail_Recurse_Expression_B_Grammar".into(),
            Sequence::new(vec![
                // Only safe to use if the recursive call is at the END of the repeating
                // element in the main b_expr portion.
                AnyNumberOf::new(vec![
                    Ref::new("Expression_B_Unary_Operator_Grammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("Expression_C_Grammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_B_Grammar".into(),
            Sequence::new(vec![
                // Always start with the tail recursion element
                Ref::new("Tail_Recurse_Expression_B_Grammar").to_matchable(),
                AnyNumberOf::new(vec![
                    one_of(vec![
                        // Arithmetic, string, or comparison binary operators followed by tail
                        // recursion
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::new("ArithmeticBinaryOperatorGrammar").to_matchable(),
                                Ref::new("StringBinaryOperatorGrammar").to_matchable(),
                                Ref::new("ComparisonOperatorGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("Tail_Recurse_Expression_B_Grammar").to_matchable(),
                        ])
                        .to_matchable(),
                        // Additional sequences and rules from b_expr can be added here
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_C_Grammar".into(),
            one_of(vec![
                // Sequence for "EXISTS" with a bracketed selectable grammar
                Sequence::new(vec![
                    Ref::keyword("EXISTS").to_matchable(),
                    Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                // Sequence for Expression_D_Grammar or CaseExpressionSegment
                // followed by any number of TimeZoneGrammar
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("Expression_D_Grammar").to_matchable(),
                        Ref::new("CaseExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("TimeZoneGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("ShorthandCastSegment").to_matchable(),
            ])
            .config(|this| this.terminators = vec_of_erased![Ref::new("CommaSegment")])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_D_Grammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("BareFunctionSegment").to_matchable(),
                    Ref::new("FunctionSegment").to_matchable(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Ref::new("ExpressionSegment").to_matchable(),
                            Ref::new("SelectableGrammar").to_matchable(),
                            Delimited::new(vec![
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                Ref::new("FunctionSegment").to_matchable(),
                                Ref::new("LiteralGrammar").to_matchable(),
                                Ref::new("LocalAliasSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.parse_mode(ParseMode::Greedy))
                    .to_matchable(),
                    Ref::new("SelectStatementSegment").to_matchable(),
                    Ref::new("LiteralGrammar").to_matchable(),
                    Ref::new("IntervalExpressionSegment").to_matchable(),
                    Ref::new("TypedStructLiteralSegment").to_matchable(),
                    Ref::new("ArrayExpressionSegment").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ObjectReferenceDelimiterGrammar").to_matchable(),
                        Ref::new("StarSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("StructTypeSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("DatatypeSegment").to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("BooleanLiteralGrammar").to_matchable(),
                            Ref::new("NullLiteralSegment").to_matchable(),
                            Ref::new("DateTimeLiteralGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("LocalAliasSegment").to_matchable(),
                ])
                .config(|this| this.terminators = vec_of_erased![Ref::new("CommaSegment")])
                .to_matchable(),
                Ref::new("AccessorGrammar").optional().to_matchable(),
            ])
            .allow_gaps(true)
            .to_matchable()
            .into(),
        ),
        (
            "AccessorGrammar".into(),
            AnyNumberOf::new(vec![Ref::new("ArrayAccessorSegment").to_matchable()])
                .to_matchable()
                .into(),
        ),
    ]);

    ansi_dialect.add([
        (
            "SelectableGrammar".into(),
            one_of(vec_of_erased![
                optionally_bracketed(vec_of_erased![Ref::new("WithCompoundStatementSegment")]),
                optionally_bracketed(vec_of_erased![Ref::new(
                    "WithCompoundNonSelectStatementSegment"
                )]),
                Ref::new("NonWithSelectableGrammar"),
                Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NonWithSelectableGrammar".into(),
            one_of(vec![
                Ref::new("SetExpressionSegment").to_matchable(),
                optionally_bracketed(vec![Ref::new("SelectStatementSegment").to_matchable()])
                    .to_matchable(),
                Ref::new("NonSetSelectableGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NonWithNonSelectableGrammar".into(),
            one_of(vec![
                Ref::new("UpdateStatementSegment").to_matchable(),
                Ref::new("InsertStatementSegment").to_matchable(),
                Ref::new("DeleteStatementSegment").to_matchable(),
                Ref::new("MergeStatementSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NonSetSelectableGrammar".into(),
            one_of(vec![
                Ref::new("ValuesClauseSegment").to_matchable(),
                Ref::new("UnorderedSelectStatementSegment").to_matchable(),
                Bracketed::new(vec![Ref::new("SelectStatementSegment").to_matchable()])
                    .to_matchable(),
                Bracketed::new(vec![
                    Ref::new("WithCompoundStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Bracketed::new(vec![Ref::new("NonSetSelectableGrammar").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // This is a hook point to allow subclassing for other dialects
    ansi_dialect.add([
        (
            "PostTableExpressionGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "BracketedSegment".into(),
            BracketedSegmentMatcher::new().to_matchable().into(),
        ),
    ]);

    ansi_dialect
}

pub fn select_clause_element() -> Matchable {
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
        Matcher::regex("whitespace", r"[^\S\r\n]+", SyntaxKind::Whitespace),
        Matcher::regex("inline_comment", r"(--|#)[^\n]*", SyntaxKind::InlineComment),
        Matcher::native("block_comment", block_comment, SyntaxKind::BlockComment)
            .subdivider(Pattern::legacy(
                "newline",
                |_| true,
                r"\r\n|\n",
                SyntaxKind::Newline,
            ))
            .post_subdivide(Pattern::legacy(
                "whitespace",
                |_| true,
                r"[^\S\r\n]+",
                SyntaxKind::Whitespace,
            )),
        Matcher::regex(
            "single_quote",
            r"'([^'\\]|\\.|'')*'",
            SyntaxKind::SingleQuote,
        ),
        Matcher::regex(
            "double_quote",
            r#""([^"\\]|\\.)*""#,
            SyntaxKind::DoubleQuote,
        ),
        Matcher::regex("back_quote", r"`[^`]*`", SyntaxKind::BackQuote),
        Matcher::legacy(
            "dollar_quote",
            |s| s.starts_with("$"),
            r"\$(\w*)\$[\s\S]*?\$\1\$",
            SyntaxKind::DollarQuote,
        ),
        Matcher::native(
            "numeric_literal",
            numeric_literal,
            SyntaxKind::NumericLiteral,
        ),
        Matcher::regex("like_operator", r"!?~~?\*?", SyntaxKind::LikeOperator),
        Matcher::regex("newline", r"(\r\n|\n)", SyntaxKind::Newline),
        Matcher::string("casting_operator", "::", SyntaxKind::CastingOperator),
        Matcher::string("equals", "=", SyntaxKind::RawComparisonOperator),
        Matcher::string("greater_than", ">", SyntaxKind::RawComparisonOperator),
        Matcher::string("less_than", "<", SyntaxKind::RawComparisonOperator),
        Matcher::string("not", "!", SyntaxKind::RawComparisonOperator),
        Matcher::string("dot", ".", SyntaxKind::Dot),
        Matcher::string("comma", ",", SyntaxKind::Comma),
        Matcher::string("plus", "+", SyntaxKind::Plus),
        Matcher::string("minus", "-", SyntaxKind::Minus),
        Matcher::string("divide", "/", SyntaxKind::Divide),
        Matcher::string("percent", "%", SyntaxKind::Percent),
        Matcher::string("question", "?", SyntaxKind::Question),
        Matcher::string("ampersand", "&", SyntaxKind::Ampersand),
        Matcher::string("vertical_bar", "|", SyntaxKind::VerticalBar),
        Matcher::string("caret", "^", SyntaxKind::Caret),
        Matcher::string("star", "*", SyntaxKind::Star),
        Matcher::string("start_bracket", "(", SyntaxKind::StartBracket),
        Matcher::string("end_bracket", ")", SyntaxKind::EndBracket),
        Matcher::string("start_square_bracket", "[", SyntaxKind::StartSquareBracket),
        Matcher::string("end_square_bracket", "]", SyntaxKind::EndSquareBracket),
        Matcher::string("start_curly_bracket", "{", SyntaxKind::StartCurlyBracket),
        Matcher::string("end_curly_bracket", "}", SyntaxKind::EndCurlyBracket),
        Matcher::string("colon", ":", SyntaxKind::Colon),
        Matcher::string("semicolon", ";", SyntaxKind::Semicolon),
        Matcher::regex("word", "[0-9a-zA-Z_]+", SyntaxKind::Word),
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
            one_of(vec_of_erased![
                Ref::keyword("PRECEDING"),
                Ref::keyword("FOLLOWING")
            ])
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

pub fn get_unordered_select_statement_segment_grammar() -> Matchable {
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

pub fn select_statement() -> Matchable {
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

pub fn select_clause_segment() -> Matchable {
    Sequence::new(vec_of_erased![
        Ref::keyword("SELECT"),
        Ref::new("SelectClauseModifierSegment").optional(),
        MetaSegment::indent(),
        Delimited::new(vec_of_erased![Ref::new("SelectClauseElementSegment")])
            .config(|this| this.allow_trailing()),
    ])
    .terminators(vec_of_erased![Ref::new("SelectClauseTerminatorGrammar")])
    .config(|this| {
        this.parse_mode(ParseMode::GreedyOnceStarted);
    })
    .to_matchable()
}

pub fn statement_segment() -> Matchable {
    one_of(vec![
        Ref::new("SelectableGrammar").to_matchable(),
        Ref::new("MergeStatementSegment").to_matchable(),
        Ref::new("InsertStatementSegment").to_matchable(),
        Ref::new("TransactionStatementSegment").to_matchable(),
        Ref::new("DropTableStatementSegment").to_matchable(),
        Ref::new("DropViewStatementSegment").to_matchable(),
        Ref::new("CreateUserStatementSegment").to_matchable(),
        Ref::new("DropUserStatementSegment").to_matchable(),
        Ref::new("TruncateStatementSegment").to_matchable(),
        Ref::new("AccessStatementSegment").to_matchable(),
        Ref::new("CreateTableStatementSegment").to_matchable(),
        Ref::new("CreateRoleStatementSegment").to_matchable(),
        Ref::new("DropRoleStatementSegment").to_matchable(),
        Ref::new("AlterTableStatementSegment").to_matchable(),
        Ref::new("CreateSchemaStatementSegment").to_matchable(),
        Ref::new("SetSchemaStatementSegment").to_matchable(),
        Ref::new("DropSchemaStatementSegment").to_matchable(),
        Ref::new("DropTypeStatementSegment").to_matchable(),
        Ref::new("CreateDatabaseStatementSegment").to_matchable(),
        Ref::new("DropDatabaseStatementSegment").to_matchable(),
        Ref::new("CreateIndexStatementSegment").to_matchable(),
        Ref::new("DropIndexStatementSegment").to_matchable(),
        Ref::new("CreateViewStatementSegment").to_matchable(),
        Ref::new("DeleteStatementSegment").to_matchable(),
        Ref::new("UpdateStatementSegment").to_matchable(),
        Ref::new("CreateCastStatementSegment").to_matchable(),
        Ref::new("DropCastStatementSegment").to_matchable(),
        Ref::new("CreateFunctionStatementSegment").to_matchable(),
        Ref::new("DropFunctionStatementSegment").to_matchable(),
        Ref::new("CreateModelStatementSegment").to_matchable(),
        Ref::new("DropModelStatementSegment").to_matchable(),
        Ref::new("DescribeStatementSegment").to_matchable(),
        Ref::new("UseStatementSegment").to_matchable(),
        Ref::new("ExplainStatementSegment").to_matchable(),
        Ref::new("CreateSequenceStatementSegment").to_matchable(),
        Ref::new("AlterSequenceStatementSegment").to_matchable(),
        Ref::new("DropSequenceStatementSegment").to_matchable(),
        Ref::new("CreateTriggerStatementSegment").to_matchable(),
        Ref::new("DropTriggerStatementSegment").to_matchable(),
    ])
    .config(|this| this.terminators = vec_of_erased![Ref::new("DelimiterGrammar")])
    .to_matchable()
}

pub fn wildcard_expression_segment() -> Matchable {
    Sequence::new(vec![Ref::new("WildcardIdentifierSegment").to_matchable()]).to_matchable()
}

fn numeric_literal(cursor: &mut Cursor) -> bool {
    let first_char = cursor.shift();
    match first_char {
        '0'..='9' | '.' => {
            let has_decimal = first_char == '.';

            if has_decimal {
                if cursor.peek().is_ascii_digit() {
                    cursor.shift_while(|c| c.is_ascii_digit());
                } else {
                    return false;
                }
            } else {
                cursor.shift_while(|c| c.is_ascii_digit());
                if cursor.peek() == '.' {
                    cursor.shift();
                    cursor.shift_while(|c| c.is_ascii_digit());
                }
            }

            if let 'e' | 'E' = cursor.peek() {
                cursor.shift();
                if let '+' | '-' = cursor.peek() {
                    cursor.shift();
                }
                let mut exp_digits = false;
                while cursor.peek().is_ascii_digit() {
                    cursor.shift();
                    exp_digits = true;
                }
                if !exp_digits {
                    return false;
                }
            }

            let next_char = cursor.peek();
            if next_char == '.' || next_char.is_ascii_alphanumeric() || next_char == '_' {
                return false;
            }

            true
        }
        _ => false,
    }
}

fn block_comment(cursor: &mut Cursor) -> bool {
    if cursor.shift() != '/' {
        return false;
    }

    if cursor.shift() != '*' {
        return false;
    }

    let mut depth = 1usize;

    loop {
        match cursor.shift() {
            '\0' => return false,
            '/' if cursor.peek() == '*' => {
                cursor.shift();
                depth += 1;
            }
            '*' if cursor.peek() == '/' => {
                cursor.shift();
                depth -= 1;
                if depth == 0 {
                    break true;
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn select_clause_terminators() -> Vec<Matchable> {
    vec_of_erased![
        Ref::keyword("FROM"),
        Ref::keyword("WHERE"),
        Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
        Ref::keyword("LIMIT"),
        Ref::keyword("OVERLAPS"),
        Ref::new("SetOperatorSegment"),
        Ref::keyword("FETCH"),
    ]
}
