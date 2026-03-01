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

use super::ansi_keywords::{ANSI_RESERVED_KEYWORDS, ANSI_UNRESERVED_KEYWORDS};
use sqruff_lib_core::dialects::init::{DialectConfig, NullDialectConfig};
use sqruff_lib_core::value::Value;

/// Configuration for the ANSI dialect.
/// Currently empty but can be extended with dialect-specific options.
pub type AnsiDialectConfig = NullDialectConfig;

pub fn dialect(config: Option<&Value>) -> Dialect {
    // Parse and validate dialect configuration, falling back to defaults on failure
    let _dialect_config: AnsiDialectConfig = config
        .map(AnsiDialectConfig::from_value)
        .unwrap_or_default();

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
                            vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
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

                RegexParser::new(
                    "[\\p{L}\\p{N}_]*[\\p{L}][\\p{L}\\p{N}_]*",
                    SyntaxKind::NakedIdentifier,
                )
                .anti_template(&anti_template)
                .to_matchable()
            })
            .into(),
        ),
        (
            "ParameterNameSegment".into(),
            RegexParser::new(r#"\"?[\p{L}][\p{L}\p{N}_]*\"?"#, SyntaxKind::Parameter)
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
                    RegexParser::new("[\\p{L}_][\\p{L}\\p{N}_]*", SyntaxKind::DataTypeIdentifier)
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
            one_of(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::new("QuotedIdentifierSegment").to_matchable(),
            ])
            .config(|this| this.terminators = vec![Ref::new("DotSegment").to_matchable()])
            .to_matchable()
            .into(),
        ),
        (
            "BooleanLiteralGrammar".into(),
            one_of(vec![
                Ref::new("TrueSegment").to_matchable(),
                Ref::new("FalseSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // We specifically define a group of arithmetic operators to make it easier to
        // override this if some dialects have different available operators
        (
            "ArithmeticBinaryOperatorGrammar".into(),
            one_of(vec![
                Ref::new("PlusSegment").to_matchable(),
                Ref::new("MinusSegment").to_matchable(),
                Ref::new("DivideSegment").to_matchable(),
                Ref::new("MultiplySegment").to_matchable(),
                Ref::new("ModuloSegment").to_matchable(),
                Ref::new("BitwiseAndSegment").to_matchable(),
                Ref::new("BitwiseOrSegment").to_matchable(),
                Ref::new("BitwiseXorSegment").to_matchable(),
                Ref::new("BitwiseLShiftSegment").to_matchable(),
                Ref::new("BitwiseRShiftSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SignedSegmentGrammar".into(),
            one_of(vec![
                Ref::new("PositiveSegment").to_matchable(),
                Ref::new("NegativeSegment").to_matchable(),
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
            one_of(vec![
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("GreaterThanSegment").to_matchable(),
                Ref::new("LessThanSegment").to_matchable(),
                Ref::new("GreaterThanOrEqualToSegment").to_matchable(),
                Ref::new("LessThanOrEqualToSegment").to_matchable(),
                Ref::new("NotEqualToSegment").to_matchable(),
                Ref::new("LikeOperatorSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("IS").to_matchable(),
                    Ref::keyword("DISTINCT").to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("IS").to_matchable(),
                    Ref::keyword("NOT").to_matchable(),
                    Ref::keyword("DISTINCT").to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                ])
                .to_matchable(),
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
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("DATE").to_matchable(),
                    Ref::keyword("TIME").to_matchable(),
                    Ref::keyword("TIMESTAMP").to_matchable(),
                    Ref::keyword("INTERVAL").to_matchable(),
                ])
                .to_matchable(),
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral)
                    .to_matchable(),
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
            one_of(vec![
                Ref::new("QuotedLiteralSegment").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
                Ref::new("BooleanLiteralGrammar").to_matchable(),
                Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
                // NB: Null is included in the literals, because it is a keyword which
                // can otherwise be easily mistaken for an identifier.
                Ref::new("NullLiteralSegment").to_matchable(),
                Ref::new("DateTimeLiteralGrammar").to_matchable(),
                Ref::new("ArrayLiteralSegment").to_matchable(),
                Ref::new("TypedArrayLiteralSegment").to_matchable(),
                Ref::new("ObjectLiteralSegment").to_matchable(),
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
            one_of(vec![
                Ref::new("ArithmeticBinaryOperatorGrammar").to_matchable(),
                Ref::new("StringBinaryOperatorGrammar").to_matchable(),
                Ref::new("BooleanBinaryOperatorGrammar").to_matchable(),
                Ref::new("ComparisonOperatorGrammar").to_matchable(),
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
            Sequence::new(vec![
                Ref::keyword("IF").to_matchable(),
                Ref::keyword("NOT").to_matchable(),
                Ref::keyword("EXISTS").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LikeGrammar".into(),
            one_of(vec![
                Ref::keyword("LIKE").to_matchable(),
                Ref::keyword("RLIKE").to_matchable(),
                Ref::keyword("ILIKE").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "UnionGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("UNION").to_matchable(),
                one_of(vec![
                    Ref::keyword("DISTINCT").to_matchable(),
                    Ref::keyword("ALL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
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
            Sequence::new(vec![
                Ref::keyword("NOT").optional().to_matchable(),
                Ref::keyword("IN").to_matchable(),
                one_of(vec![
                    Bracketed::new(vec![
                        one_of(vec![
                            Delimited::new(vec![Ref::new("Expression_A_Grammar").to_matchable()])
                                .to_matchable(),
                            Ref::new("SelectableGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.parse_mode(ParseMode::Greedy))
                    .to_matchable(),
                    Ref::new("FunctionSegment").to_matchable(),
                ])
                .to_matchable(),
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

                this.terminators = vec![
                    Ref::new("CommaSegment").to_matchable(),
                    Ref::keyword("AS").to_matchable(),
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
            Sequence::new(vec![
                Ref::keyword("REFERENCES").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                // Foreign columns making up FOREIGN KEY constraint
                Ref::new("BracketedColumnReferenceListGrammar")
                    .optional()
                    .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("MATCH").to_matchable(),
                    one_of(vec![
                        Ref::keyword("FULL").to_matchable(),
                        Ref::keyword("PARTIAL").to_matchable(),
                        Ref::keyword("SIMPLE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                AnyNumberOf::new(vec![
                    // ON DELETE clause, e.g. ON DELETE NO ACTION
                    Sequence::new(vec![
                        Ref::keyword("ON").to_matchable(),
                        Ref::keyword("DELETE").to_matchable(),
                        Ref::new("ReferentialActionGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ON").to_matchable(),
                        Ref::keyword("UPDATE").to_matchable(),
                        Ref::new("ReferentialActionGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
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
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("COLUMN").optional().to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AlterTableOptionsGrammar".into(),
            one_of(vec![
                // Table options
                Sequence::new(vec![
                    Ref::new("ParameterNameSegment").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::new("LiteralGrammar").to_matchable(),
                        Ref::new("NakedIdentifierSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                // Add things
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("ADD").to_matchable(),
                        Ref::keyword("MODIFY").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("COLUMN").optional().to_matchable(),
                    Ref::new("ColumnDefinitionSegment").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("FIRST").to_matchable(),
                                Ref::keyword("AFTER").to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                // Bracketed Version of the same
                                Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                // Drop Column
                Ref::new("AlterTableDropColumnGrammar").to_matchable(),
                // Rename
                Sequence::new(vec![
                    Ref::keyword("RENAME").to_matchable(),
                    one_of(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
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
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    one_of(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Bracketed::new(vec![Ref::new("WindowSpecificationSegment").to_matchable()])
                            .config(|this| this.parse_mode(ParseMode::Greedy))
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
                Sequence::new(vec![
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Ref::new("DatetimeUnitSegment").to_matchable(),
                            Ref::new("FunctionContentsGrammar")
                                .optional()
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
            "FunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::Function, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("DatePartFunctionNameSegment").to_matchable(),
                        Ref::new("DateTimeFunctionContentsSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Sequence::new(vec![
                            Ref::new("FunctionNameSegment")
                                .exclude(one_of(vec![
                                    Ref::new("DatePartFunctionNameSegment").to_matchable(),
                                    Ref::new("ValuesClauseSegment").to_matchable(),
                                ]))
                                .to_matchable(),
                            Ref::new("FunctionContentsSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("PostFunctionGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::HavingClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("HAVING").to_matchable(),
                    MetaSegment::implicit_indent().to_matchable(),
                    optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PathSegment".into(),
            NodeMatcher::new(SyntaxKind::PathSegment, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("SlashSegment").to_matchable(),
                        Delimited::new(vec![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::PathSegment)
                                .to_matchable(),
                        ])
                        .config(|this| {
                            this.allow_gaps = false;
                            this.delimiter(Ref::new("SlashSegment"));
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LimitClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::LimitClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("LIMIT").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    optionally_bracketed(vec![
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                            Ref::keyword("ALL").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("OFFSET").to_matchable(),
                            one_of(vec![
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("CommaSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CubeRollupClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::CubeRollupClause, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("CubeFunctionNameSegment").to_matchable(),
                        Ref::new("RollupFunctionNameSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Bracketed::new(vec![Ref::new("GroupingExpressionList").to_matchable()])
                        .to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("GROUPING").to_matchable(),
                    Ref::keyword("SETS").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Ref::new("CubeRollupClauseSegment").to_matchable(),
                            Ref::new("GroupingExpressionList").to_matchable(),
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
            "GroupingExpressionList".into(),
            NodeMatcher::new(SyntaxKind::GroupingExpressionList, |_| {
                Sequence::new(vec![
                    MetaSegment::indent().to_matchable(),
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                            Bracketed::new(vec![]).to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("GroupByClauseTerminatorGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SetClause, |_| {
                Sequence::new(vec![
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("LiteralGrammar").to_matchable(),
                        Ref::new("BareFunctionSegment").to_matchable(),
                        Ref::new("FunctionSegment").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                        Ref::new("ValuesClauseSegment").to_matchable(),
                        Ref::keyword("DEFAULT").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FetchClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FetchClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("FETCH").to_matchable(),
                    one_of(vec![
                        Ref::keyword("FIRST").to_matchable(),
                        Ref::keyword("NEXT").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("NumericLiteralSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("ROW").to_matchable(),
                        Ref::keyword("ROWS").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("ONLY").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FunctionDefinitionGrammar".into(),
            NodeMatcher::new(SyntaxKind::FunctionDefinition, |_| {
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("LANGUAGE").to_matchable(),
                        Ref::new("NakedIdentifierSegment").to_matchable(),
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
            "AlterSequenceOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterSequenceOptionsSegment, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("INCREMENT").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("MINVALUE").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NO").to_matchable(),
                            Ref::keyword("MINVALUE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("MAXVALUE").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NO").to_matchable(),
                            Ref::keyword("MAXVALUE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("CACHE").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("NOCACHE").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("CYCLE").to_matchable(),
                        Ref::keyword("NOCYCLE").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ORDER").to_matchable(),
                        Ref::keyword("NOORDER").to_matchable(),
                    ])
                    .to_matchable(),
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
                            vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
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
                            vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
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
                            vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnDefinition, |_| {
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(), // Column name
                    Ref::new("DatatypeSegment").to_matchable(),         // Column type,
                    Bracketed::new(vec![Anything::new().to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("ColumnConstraintSegment").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_| {
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONSTRAINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("NOT").optional().to_matchable(),
                            Ref::keyword("NULL").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CHECK").to_matchable(),
                            Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("DEFAULT").to_matchable(),
                            Ref::new("ColumnConstraintDefaultGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("PrimaryKeyGrammar").to_matchable(),
                        Ref::new("UniqueKeyGrammar").to_matchable(), // UNIQUE
                        Ref::new("AutoIncrementGrammar").to_matchable(),
                        Ref::new("ReferenceDefinitionGrammar").to_matchable(), /* REFERENCES reftable [ (
                                                                                * refcolumn) ] */
                        Ref::new("CommentClauseSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("COLLATE").to_matchable(),
                            Ref::new("CollationReferenceSegment").to_matchable(),
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
            "CommentClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::CommentClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("COMMENT").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
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
                AnyNumberOf::new(vec![
                    Ref::new("MergeMatchedClauseSegment").to_matchable(),
                    Ref::new("MergeNotMatchedClauseSegment").to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("WHEN").to_matchable(),
                    Ref::keyword("MATCHED").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AND").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    one_of(vec![
                        Ref::new("MergeUpdateClauseSegment").to_matchable(),
                        Ref::new("MergeDeleteClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeNotMatchedClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeWhenNotMatchedClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("WHEN").to_matchable(),
                    Ref::keyword("NOT").to_matchable(),
                    Ref::keyword("MATCHED").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AND").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("MergeInsertClauseSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeInsertClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeInsertClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("INSERT").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::new("ValuesClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeUpdateClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeUpdateClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("UPDATE").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("SetClauseListSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("SetClauseSegment").to_matchable(),
                    AnyNumberOf::new(vec![
                        Ref::new("CommaSegment").to_matchable(),
                        Ref::new("SetClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
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
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|this| this.optional())
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::GroupbyClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("GROUP").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    one_of(vec![
                        Ref::new("CubeRollupClauseSegment").to_matchable(),
                        Sequence::new(vec![
                            MetaSegment::indent().to_matchable(),
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                    Ref::new("NumericLiteralSegment").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| {
                                this.terminators =
                                    vec![Ref::new("GroupByClauseTerminatorGrammar").to_matchable()];
                            })
                            .to_matchable(),
                            MetaSegment::dedent().to_matchable(),
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
            "FrameClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FrameClause, |_| {
                Sequence::new(vec![
                    Ref::new("FrameClauseUnitGrammar").to_matchable(),
                    one_of(vec![
                        frame_extent().to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("BETWEEN").to_matchable(),
                            frame_extent().to_matchable(),
                            Ref::keyword("AND").to_matchable(),
                            frame_extent().to_matchable(),
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
            "WithCompoundStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WithCompoundStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("RECURSIVE").optional().to_matchable(),
                    Conditional::new(MetaSegment::indent())
                        .indented_ctes()
                        .to_matchable(),
                    Delimited::new(vec![Ref::new("CTEDefinitionSegment").to_matchable()])
                        .config(|this| {
                            this.terminators = vec![Ref::keyword("SELECT").to_matchable()];
                            this.allow_trailing();
                        })
                        .to_matchable(),
                    Conditional::new(MetaSegment::dedent())
                        .indented_ctes()
                        .to_matchable(),
                    Ref::new("NonWithSelectableGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WithCompoundNonSelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WithCompoundStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("RECURSIVE").optional().to_matchable(),
                    Conditional::new(MetaSegment::indent())
                        .indented_ctes()
                        .to_matchable(),
                    Delimited::new(vec![Ref::new("CTEDefinitionSegment").to_matchable()])
                        .config(|this| {
                            this.terminators = vec![Ref::keyword("SELECT").to_matchable()];
                            this.allow_trailing();
                        })
                        .to_matchable(),
                    Conditional::new(MetaSegment::dedent())
                        .indented_ctes()
                        .to_matchable(),
                    Ref::new("NonWithNonSelectableGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CTEDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::CommonTableExpression, |_| {
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("CTEColumnList").optional().to_matchable(),
                    Ref::keyword("AS").optional().to_matchable(),
                    Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CTEColumnList".into(),
            NodeMatcher::new(SyntaxKind::CTEColumnList, |_| {
                Bracketed::new(vec![Ref::new("SingleIdentifierListSegment").to_matchable()])
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
                            vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
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
                            vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::TableConstraint, |_| {
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONSTRAINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("UNIQUE").to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("PrimaryKeyGrammar").to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("ForeignKeyGrammar").to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                            Ref::new("ReferenceDefinitionGrammar").to_matchable(),
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
            "JoinOnConditionSegment".into(),
            NodeMatcher::new(SyntaxKind::JoinOnCondition, |_| {
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Conditional::new(MetaSegment::implicit_indent())
                        .indented_on_contents()
                        .to_matchable(),
                    optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                    Conditional::new(MetaSegment::dedent())
                        .indented_on_contents()
                        .to_matchable(),
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
                            vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
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
                            vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CollationReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::CollationReference, |_| {
                one_of(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                        .config(|this| {
                            this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                            this.terminators =
                                vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
                            this.allow_gaps = false;
                        })
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "OverClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OverClause, |_| {
                Sequence::new(vec![
                    MetaSegment::indent().to_matchable(),
                    Ref::new("IgnoreRespectNullsGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::keyword("OVER").to_matchable(),
                    one_of(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Bracketed::new(vec![
                            Ref::new("WindowSpecificationSegment")
                                .optional()
                                .to_matchable(),
                        ])
                        .config(|this| this.parse_mode(ParseMode::Greedy))
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NamedWindowSegment".into(),
            NodeMatcher::new(SyntaxKind::NamedWindow, |_| {
                Sequence::new(vec![
                    Ref::keyword("WINDOW").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Delimited::new(vec![
                        Ref::new("NamedWindowExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WindowSpecificationSegment".into(),
            NodeMatcher::new(SyntaxKind::WindowSpecification, |_| {
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar")
                        .optional()
                        .exclude(one_of(vec![
                            Ref::keyword("PARTITION").to_matchable(),
                            Ref::keyword("ORDER").to_matchable(),
                        ]))
                        .to_matchable(),
                    Ref::new("PartitionClauseSegment").optional().to_matchable(),
                    Ref::new("OrderByClauseSegment").optional().to_matchable(),
                    Ref::new("FrameClauseSegment").optional().to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    optionally_bracketed(vec![
                        Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "JoinClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::JoinClause, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("JoinTypeKeywordsGrammar")
                            .optional()
                            .to_matchable(),
                        Ref::new("JoinKeywordsGrammar").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::new("FromExpressionElementSegment").to_matchable(),
                        AnyNumberOf::new(vec![Ref::new("NestedJoinGrammar").to_matchable()])
                            .to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                        Sequence::new(vec![
                            Conditional::new(MetaSegment::indent())
                                .indented_using_on()
                                .to_matchable(),
                            one_of(vec![
                                Ref::new("JoinOnConditionSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("USING").to_matchable(),
                                    MetaSegment::indent().to_matchable(),
                                    Bracketed::new(vec![
                                        Delimited::new(vec![
                                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .config(|this| this.parse_mode = ParseMode::Greedy)
                                    .to_matchable(),
                                    MetaSegment::dedent().to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Conditional::new(MetaSegment::dedent())
                                .indented_using_on()
                                .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("NaturalJoinKeywordsGrammar").to_matchable(),
                        Ref::new("JoinKeywordsGrammar").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::new("FromExpressionElementSegment").to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("ExtendedNaturalJoinKeywordsGrammar").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::new("FromExpressionElementSegment").to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTriggerStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("TRIGGER").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("TriggerReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SampleExpression, |_| {
                Sequence::new(vec![
                    Ref::keyword("TABLESAMPLE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("BERNOULLI").to_matchable(),
                        Ref::keyword("SYSTEM").to_matchable(),
                    ])
                    .to_matchable(),
                    Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("REPEATABLE").to_matchable(),
                        Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                            .to_matchable(),
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
            "TableExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableExpression, |_| {
                one_of(vec![
                    Ref::new("ValuesClauseSegment").to_matchable(),
                    Ref::new("BareFunctionSegment").to_matchable(),
                    Ref::new("FunctionSegment").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                    Bracketed::new(vec![Ref::new("MergeStatementSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTriggerStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("TRIGGER").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("TriggerReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SampleExpression, |_| {
                Sequence::new(vec![
                    Ref::keyword("TABLESAMPLE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("BERNOULLI").to_matchable(),
                        Ref::keyword("SYSTEM").to_matchable(),
                    ])
                    .to_matchable(),
                    Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("REPEATABLE").to_matchable(),
                        Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                            .to_matchable(),
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
            "TableExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableExpression, |_| {
                one_of(vec![
                    Ref::new("ValuesClauseSegment").to_matchable(),
                    Ref::new("BareFunctionSegment").to_matchable(),
                    Ref::new("FunctionSegment").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                    Bracketed::new(vec![Ref::new("MergeStatementSegment").to_matchable()])
                        .to_matchable(),
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
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("REFERENCING").to_matchable(),
                            Ref::keyword("OLD").to_matchable(),
                            Ref::keyword("ROW").to_matchable(),
                            Ref::keyword("AS").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                            Ref::keyword("NEW").to_matchable(),
                            Ref::keyword("ROW").to_matchable(),
                            Ref::keyword("AS").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FROM").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("NOT").to_matchable(),
                                Ref::keyword("DEFERRABLE").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("DEFERRABLE").optional().to_matchable(),
                                one_of(vec![
                                    Sequence::new(vec![
                                        Ref::keyword("INITIALLY").to_matchable(),
                                        Ref::keyword("IMMEDIATE").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("INITIALLY").to_matchable(),
                                        Ref::keyword("DEFERRED").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FOR").to_matchable(),
                            Ref::keyword("EACH").optional().to_matchable(),
                            one_of(vec![
                                Ref::keyword("ROW").to_matchable(),
                                Ref::keyword("STATEMENT").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("WHEN").to_matchable(),
                            Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("MODEL").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DescribeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DescribeStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DESCRIBE").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UseStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("USE").to_matchable(),
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExplainStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ExplainStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("EXPLAIN").to_matchable(),
                    one_of(vec![
                        Ref::new("SelectableGrammar").to_matchable(),
                        Ref::new("InsertStatementSegment").to_matchable(),
                        Ref::new("UpdateStatementSegment").to_matchable(),
                        Ref::new("DeleteStatementSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateSequenceStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSequenceStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("SEQUENCE").to_matchable(),
                    Ref::new("SequenceReferenceSegment").to_matchable(),
                    AnyNumberOf::new(vec![
                        Ref::new("CreateSequenceOptionsSegment").to_matchable(),
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
            "CreateSequenceOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSequenceOptionsSegment, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("INCREMENT").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("WITH").optional().to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("MINVALUE").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NO").to_matchable(),
                            Ref::keyword("MINVALUE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("MAXVALUE").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NO").to_matchable(),
                            Ref::keyword("MAXVALUE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("CACHE").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("NOCACHE").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("CYCLE").to_matchable(),
                        Ref::keyword("NOCYCLE").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ORDER").to_matchable(),
                        Ref::keyword("NOORDER").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AlterSequenceStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterSequenceStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("SEQUENCE").to_matchable(),
                    Ref::new("SequenceReferenceSegment").to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("AlterSequenceOptionsSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropSequenceStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropSequenceStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("SEQUENCE").to_matchable(),
                    Ref::new("SequenceReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropCastStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropCastStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("CAST").to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("DatatypeSegment").to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DropBehaviorGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateFunctionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::new("TemporaryGrammar").optional().to_matchable(),
                    Ref::keyword("FUNCTION").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                    Ref::new("FunctionParameterListGrammar").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RETURNS").to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("FunctionDefinitionGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropFunctionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("FUNCTION").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateModelStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateModelStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::keyword("MODEL").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OPTIONS").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::new("ParameterNameSegment").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::new("LiteralGrammar").to_matchable(), // Single value
                                        Bracketed::new(vec![
                                            Delimited::new(vec![
                                                Ref::new("QuotedLiteralSegment").to_matchable(),
                                            ])
                                            .to_matchable(),
                                        ])
                                        .config(|this| {
                                            this.bracket_type("square");
                                            this.optional();
                                        })
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("SelectableGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateViewStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::keyword("VIEW").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                    Ref::new("WithNoSchemaBindingClauseSegment")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DELETE").to_matchable(),
                    Ref::new("FromClauseSegment").to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UpdateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UpdateStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("UPDATE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("AliasExpressionSegment")
                        .exclude(Ref::keyword("SET"))
                        .optional()
                        .to_matchable(),
                    Ref::new("SetClauseListSegment").to_matchable(),
                    Ref::new("FromClauseSegment").optional().to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateCastStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateCastStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("CAST").to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("DatatypeSegment").to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("SPECIFIC").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("ROUTINE").to_matchable(),
                        Ref::keyword("FUNCTION").to_matchable(),
                        Ref::keyword("PROCEDURE").to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("INSTANCE").to_matchable(),
                                Ref::keyword("STATIC").to_matchable(),
                                Ref::keyword("CONSTRUCTOR").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::keyword("METHOD").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                    Ref::new("FunctionParameterListGrammar")
                        .optional()
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::keyword("ASSIGNMENT").to_matchable(),
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
            "CreateRoleStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateRoleStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("ROLE").to_matchable(),
                    Ref::new("RoleReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropRoleStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropRoleStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("ROLE").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AlterTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterTableStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Delimited::new(vec![Ref::new("AlterTableOptionsGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetSchemaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetSchemaStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::keyword("SCHEMA").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("SchemaReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropSchemaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropSchemaStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("SCHEMA").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SchemaReferenceSegment").to_matchable(),
                    Ref::new("DropBehaviorGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropTypeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTypeStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("TYPE").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::new("DropBehaviorGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateDatabaseStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("DATABASE").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropDatabaseStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("DATABASE").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                    Ref::new("DropBehaviorGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FunctionParameterListGrammar".into(),
            NodeMatcher::new(SyntaxKind::FunctionParameterList, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("FunctionParameterGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateIndexStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::keyword("UNIQUE").optional().to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("IndexReferenceSegment").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Ref::new("IndexColumnDefinitionSegment").to_matchable(),
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
            "DropIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropIndexStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("IndexReferenceSegment").to_matchable(),
                    Ref::new("DropBehaviorGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::new("TemporaryTransientGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    one_of(vec![
                        // Columns and comment syntax
                        Sequence::new(vec![
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    one_of(vec![
                                        Ref::new("TableConstraintSegment").to_matchable(),
                                        Ref::new("ColumnDefinitionSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("CommentClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        // Create AS syntax:
                        Sequence::new(vec![
                            Ref::keyword("AS").to_matchable(),
                            optionally_bracketed(vec![
                                Ref::new("SelectableGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Create LIKE syntax
                        Sequence::new(vec![
                            Ref::keyword("LIKE").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("TableEndClauseSegment").optional().to_matchable(),
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
                    let global_permissions = one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("CREATE").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ROLE").to_matchable(),
                                Ref::keyword("USER").to_matchable(),
                                Ref::keyword("WAREHOUSE").to_matchable(),
                                Ref::keyword("DATABASE").to_matchable(),
                                Ref::keyword("INTEGRATION").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("APPLY").to_matchable(),
                            Ref::keyword("MASKING").to_matchable(),
                            Ref::keyword("POLICY").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("EXECUTE").to_matchable(),
                            Ref::keyword("TASK").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("MANAGE").to_matchable(),
                            Ref::keyword("GRANTS").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("MONITOR").to_matchable(),
                            one_of(vec![
                                Ref::keyword("EXECUTION").to_matchable(),
                                Ref::keyword("USAGE").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ]);

                    let schema_object_types = one_of(vec![
                        Ref::keyword("TABLE").to_matchable(),
                        Ref::keyword("VIEW").to_matchable(),
                        Ref::keyword("STAGE").to_matchable(),
                        Ref::keyword("FUNCTION").to_matchable(),
                        Ref::keyword("PROCEDURE").to_matchable(),
                        Ref::keyword("ROUTINE").to_matchable(),
                        Ref::keyword("SEQUENCE").to_matchable(),
                        Ref::keyword("STREAM").to_matchable(),
                        Ref::keyword("TASK").to_matchable(),
                    ]);

                    let permissions = Sequence::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("CREATE").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("SCHEMA").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("MASKING").to_matchable(),
                                        Ref::keyword("POLICY").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::keyword("PIPE").to_matchable(),
                                    schema_object_types.clone().to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("IMPORTED").to_matchable(),
                                Ref::keyword("PRIVILEGES").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("APPLY").to_matchable(),
                            Ref::keyword("CONNECT").to_matchable(),
                            Ref::keyword("CREATE").to_matchable(),
                            Ref::keyword("DELETE").to_matchable(),
                            Ref::keyword("EXECUTE").to_matchable(),
                            Ref::keyword("INSERT").to_matchable(),
                            Ref::keyword("MODIFY").to_matchable(),
                            Ref::keyword("MONITOR").to_matchable(),
                            Ref::keyword("OPERATE").to_matchable(),
                            Ref::keyword("OWNERSHIP").to_matchable(),
                            Ref::keyword("READ").to_matchable(),
                            Ref::keyword("REFERENCE_USAGE").to_matchable(),
                            Ref::keyword("REFERENCES").to_matchable(),
                            Ref::keyword("SELECT").to_matchable(),
                            Ref::keyword("TEMP").to_matchable(),
                            Ref::keyword("TEMPORARY").to_matchable(),
                            Ref::keyword("TRIGGER").to_matchable(),
                            Ref::keyword("TRUNCATE").to_matchable(),
                            Ref::keyword("UPDATE").to_matchable(),
                            Ref::keyword("USAGE").to_matchable(),
                            Ref::keyword("USE_ANY_ROLE").to_matchable(),
                            Ref::keyword("WRITE").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ALL").to_matchable(),
                                Ref::keyword("PRIVILEGES").optional().to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("BracketedColumnReferenceListGrammar")
                            .optional()
                            .to_matchable(),
                    ]);

                    let objects = one_of(vec![
                        Ref::keyword("ACCOUNT").to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("RESOURCE").to_matchable(),
                                    Ref::keyword("MONITOR").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("WAREHOUSE").to_matchable(),
                                Ref::keyword("DATABASE").to_matchable(),
                                Ref::keyword("DOMAIN").to_matchable(),
                                Ref::keyword("INTEGRATION").to_matchable(),
                                Ref::keyword("LANGUAGE").to_matchable(),
                                Ref::keyword("SCHEMA").to_matchable(),
                                Ref::keyword("ROLE").to_matchable(),
                                Ref::keyword("TABLESPACE").to_matchable(),
                                Ref::keyword("TYPE").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("FOREIGN").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("SERVER").to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("DATA").to_matchable(),
                                            Ref::keyword("WRAPPER").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ALL").to_matchable(),
                                    Ref::keyword("SCHEMAS").to_matchable(),
                                    Ref::keyword("IN").to_matchable(),
                                    Ref::keyword("DATABASE").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("FUTURE").to_matchable(),
                                    Ref::keyword("SCHEMAS").to_matchable(),
                                    Ref::keyword("IN").to_matchable(),
                                    Ref::keyword("DATABASE").to_matchable(),
                                ])
                                .to_matchable(),
                                schema_object_types.clone().to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ALL").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("TABLES").to_matchable(),
                                        Ref::keyword("VIEWS").to_matchable(),
                                        Ref::keyword("STAGES").to_matchable(),
                                        Ref::keyword("FUNCTIONS").to_matchable(),
                                        Ref::keyword("PROCEDURES").to_matchable(),
                                        Ref::keyword("ROUTINES").to_matchable(),
                                        Ref::keyword("SEQUENCES").to_matchable(),
                                        Ref::keyword("STREAMS").to_matchable(),
                                        Ref::keyword("TASKS").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::keyword("IN").to_matchable(),
                                    Ref::keyword("SCHEMA").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("FUTURE").to_matchable(),
                                    Ref::keyword("IN").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("DATABASE").to_matchable(),
                                        Ref::keyword("SCHEMA").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Delimited::new(vec![
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("FunctionNameSegment").to_matchable(),
                                    Ref::new("FunctionParameterListGrammar")
                                        .optional()
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| {
                                this.terminators = vec![
                                    Ref::keyword("TO").to_matchable(),
                                    Ref::keyword("FROM").to_matchable(),
                                ]
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("LARGE").to_matchable(),
                            Ref::keyword("OBJECT").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ]);

                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("GRANT").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Delimited::new(vec![
                                        one_of(vec![
                                            global_permissions.clone().to_matchable(),
                                            permissions.clone().to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .config(|this| {
                                        this.terminators = vec![Ref::keyword("ON").to_matchable()]
                                    })
                                    .to_matchable(),
                                    Ref::keyword("ON").to_matchable(),
                                    objects.clone().to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ROLE").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("OWNERSHIP").to_matchable(),
                                    Ref::keyword("ON").to_matchable(),
                                    Ref::keyword("USER").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("TO").to_matchable(),
                            one_of(vec![
                                Ref::keyword("GROUP").to_matchable(),
                                Ref::keyword("USER").to_matchable(),
                                Ref::keyword("ROLE").to_matchable(),
                                Ref::keyword("SHARE").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("RoleReferenceSegment").to_matchable(),
                                    Ref::new("FunctionSegment").to_matchable(),
                                    Ref::keyword("PUBLIC").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("AccessStatementSegmentGrantRoleWithOptionGrammar")
                                .optional()
                                .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("GRANTED").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("CURRENT_USER").to_matchable(),
                                    Ref::keyword("SESSION_USER").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("REVOKE").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("GRANT").to_matchable(),
                                Ref::keyword("OPTION").to_matchable(),
                                Ref::keyword("FOR").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Delimited::new(vec![
                                        one_of(vec![
                                            global_permissions.to_matchable(),
                                            permissions.to_matchable(),
                                        ])
                                        .config(|this| {
                                            this.terminators =
                                                vec![Ref::keyword("ON").to_matchable()]
                                        })
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::keyword("ON").to_matchable(),
                                    objects.to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ROLE").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("OWNERSHIP").to_matchable(),
                                    Ref::keyword("ON").to_matchable(),
                                    Ref::keyword("USER").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("FROM").to_matchable(),
                            one_of(vec![
                                Ref::keyword("GROUP").to_matchable(),
                                Ref::keyword("USER").to_matchable(),
                                Ref::keyword("ROLE").to_matchable(),
                                Ref::keyword("SHARE").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Delimited::new(vec![Ref::new("ObjectReferenceSegment").to_matchable()])
                                .to_matchable(),
                            Ref::new("DropBehaviorGrammar").optional().to_matchable(),
                        ])
                        .to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("INSERT").to_matchable(),
                    Ref::keyword("OVERWRITE").optional().to_matchable(),
                    Ref::keyword("INTO").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("SelectableGrammar").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                            Ref::new("SelectableGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DefaultValuesGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TransactionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::TransactionStatement, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("BEGIN").to_matchable(),
                        Ref::keyword("COMMIT").to_matchable(),
                        Ref::keyword("ROLLBACK").to_matchable(),
                        Ref::keyword("END").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("TRANSACTION").to_matchable(),
                        Ref::keyword("WORK").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NAME").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AND").to_matchable(),
                        Ref::keyword("NO").optional().to_matchable(),
                        Ref::keyword("CHAIN").to_matchable(),
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
            "DropTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTableStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::new("TemporaryGrammar").optional().to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                        .to_matchable(),
                    Ref::new("DropBehaviorGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropViewStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("VIEW").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("DropBehaviorGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateUserStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateUserStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("USER").to_matchable(),
                    Ref::new("RoleReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropUserStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropUserStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("USER").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("RoleReferenceSegment").to_matchable(),
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
                Sequence::new(vec![
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("DotSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.terminators = vec![Ref::new("BracketedSegment").to_matchable()]
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::new("FunctionNameIdentifierSegment").to_matchable(),
                        Ref::new("QuotedIdentifierSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .terminators(vec![Ref::new("BracketedSegment").to_matchable()])
                .allow_gaps(false)
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CaseExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::CaseExpression, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CASE").to_matchable(),
                        MetaSegment::implicit_indent().to_matchable(),
                        AnyNumberOf::new(vec![Ref::new("WhenClauseSegment").to_matchable()])
                            .config(|this| {
                                this.reset_terminators = true;
                                this.terminators = vec![
                                    Ref::keyword("ELSE").to_matchable(),
                                    Ref::keyword("END").to_matchable(),
                                ];
                            })
                            .to_matchable(),
                        Ref::new("ElseClauseSegment").optional().to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                        Ref::keyword("END").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CASE").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                        MetaSegment::implicit_indent().to_matchable(),
                        AnyNumberOf::new(vec![Ref::new("WhenClauseSegment").to_matchable()])
                            .config(|this| {
                                this.reset_terminators = true;
                                this.terminators = vec![
                                    Ref::keyword("ELSE").to_matchable(),
                                    Ref::keyword("END").to_matchable(),
                                ];
                            })
                            .to_matchable(),
                        Ref::new("ElseClauseSegment").optional().to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                        Ref::keyword("END").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![
                        Ref::new("ComparisonOperatorGrammar").to_matchable(),
                        Ref::new("CommaSegment").to_matchable(),
                        Ref::new("BinaryOperatorGrammar").to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("WHEN").to_matchable(),
                    Sequence::new(vec![
                        MetaSegment::implicit_indent().to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .to_matchable(),
                    Conditional::new(MetaSegment::indent())
                        .indented_then()
                        .to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    Conditional::new(MetaSegment::implicit_indent())
                        .indented_then_contents()
                        .to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Conditional::new(MetaSegment::dedent())
                        .indented_then_contents()
                        .to_matchable(),
                    Conditional::new(MetaSegment::dedent())
                        .indented_then()
                        .to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("WHERE").to_matchable(),
                    MetaSegment::implicit_indent().to_matchable(),
                    optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::SetOperator, |_| {
                one_of(vec![
                    Ref::new("UnionGrammar").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("INTERSECT").to_matchable(),
                            Ref::keyword("EXCEPT").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("ALL").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("MINUS").to_matchable(),
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
                one_of(vec![
                    Ref::new("TimeWithTZGrammar").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DOUBLE").to_matchable(),
                        Ref::keyword("PRECISION").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("CHARACTER").to_matchable(),
                                    Ref::keyword("BINARY").to_matchable(),
                                ])
                                .to_matchable(),
                                one_of(vec![
                                    Ref::keyword("VARYING").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("LARGE").to_matchable(),
                                        Ref::keyword("OBJECT").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Sequence::new(vec![
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                    Ref::new("DotSegment").to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                                Ref::new("DatatypeIdentifierSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("BracketedArguments").optional().to_matchable(),
                        one_of(vec![
                            Ref::keyword("UNSIGNED").to_matchable(),
                            Ref::new("CharCharacterSetGrammar").to_matchable(),
                        ])
                        .config(|config| config.optional())
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
            "AliasExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::AliasExpression, |_| {
                Sequence::new(vec![
                    MetaSegment::indent().to_matchable(),
                    Ref::keyword("AS").optional().to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Bracketed::new(vec![
                                Ref::new("SingleIdentifierListSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ShorthandCastSegment".into(),
            NodeMatcher::new(SyntaxKind::CastExpression, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("Expression_D_Grammar").to_matchable(),
                        Ref::new("CaseExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("CastOperatorSegment").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                            Ref::new("TimeZoneGrammar").optional().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.min_times(1))
                    .to_matchable(),
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
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("BaseExpressionElementGrammar").to_matchable(),
                    ])
                    .config(|this| {
                        this.delimiter(Ref::new("CommaSegment"));
                        this.optional();
                    })
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
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("BaseExpressionElementGrammar").to_matchable(),
                            Ref::new("AliasExpressionSegment").optional().to_matchable(),
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
                Sequence::new(vec![
                    Ref::new("SelectClauseSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::new("FromClauseSegment").optional().to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                    Ref::new("GroupByClauseSegment").optional().to_matchable(),
                    Ref::new("HavingClauseSegment").optional().to_matchable(),
                    Ref::new("OverlapsClauseSegment").optional().to_matchable(),
                    Ref::new("NamedWindowSegment").optional().to_matchable(),
                ])
                .terminators(vec![
                    Ref::new("SetOperatorSegment").to_matchable(),
                    Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
                    Ref::new("WithDataClauseSegment").to_matchable(),
                    Ref::new("OrderByClauseSegment").to_matchable(),
                    Ref::new("LimitClauseSegment").to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("OVERLAPS").to_matchable(),
                    one_of(vec![
                        Bracketed::new(vec![
                            Ref::new("DateTimeLiteralGrammar").to_matchable(),
                            Ref::new("CommaSegment").to_matchable(),
                            Ref::new("DateTimeLiteralGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("NO").to_matchable(),
                    Ref::keyword("SCHEMA").to_matchable(),
                    Ref::keyword("BINDING").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WithDataClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WithDataClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Sequence::new(vec![Ref::keyword("NO").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                    Ref::keyword("DATA").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SetExpression, |_| {
                Sequence::new(vec![
                    Ref::new("NonSetSelectableGrammar").to_matchable(),
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("SetOperatorSegment").to_matchable(),
                            Ref::new("NonSetSelectableGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.min_times(1))
                    .to_matchable(),
                    Ref::new("OrderByClauseSegment").optional().to_matchable(),
                    Ref::new("LimitClauseSegment").optional().to_matchable(),
                    Ref::new("NamedWindowSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FromClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FromClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("FROM").to_matchable(),
                    Delimited::new(vec![Ref::new("FromExpressionSegment").to_matchable()])
                        .to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            one_of(vec![
                                Ref::keyword("ASC").to_matchable(),
                                Ref::keyword("DESC").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("NULLS").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("FIRST").to_matchable(),
                                    Ref::keyword("LAST").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.terminators = vec![
                            Ref::keyword("LIMIT").to_matchable(),
                            Ref::new("FrameClauseUnitGrammar").to_matchable(),
                        ]
                    })
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
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
                optionally_bracketed(vec![
                    Sequence::new(vec![
                        MetaSegment::indent().to_matchable(),
                        one_of(vec![
                            Ref::new("FromExpressionElementSegment").to_matchable(),
                            Bracketed::new(vec![Ref::new("FromExpressionSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .config(|this| {
                            this.terminators = vec![
                                Sequence::new(vec![
                                    Ref::keyword("ORDER").to_matchable(),
                                    Ref::keyword("BY").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("GROUP").to_matchable(),
                                    Ref::keyword("BY").to_matchable(),
                                ])
                                .to_matchable(),
                            ]
                        })
                        .to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                        Conditional::new(MetaSegment::indent())
                            .indented_joins()
                            .to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::new("JoinClauseSegment").to_matchable(),
                                    Ref::new("JoinLikeClauseGrammar").to_matchable(),
                                ])
                                .config(|this| {
                                    this.optional();
                                    this.terminators = vec![
                                        Sequence::new(vec![
                                            Ref::keyword("ORDER").to_matchable(),
                                            Ref::keyword("BY").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("GROUP").to_matchable(),
                                            Ref::keyword("BY").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ];
                                })
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Conditional::new(MetaSegment::dedent())
                            .indented_joins()
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
                Sequence::new(vec![
                    Ref::new("PreTableFunctionKeywordsGrammar")
                        .optional()
                        .to_matchable(),
                    optionally_bracketed(vec![Ref::new("TableExpressionSegment").to_matchable()])
                        .to_matchable(),
                    Ref::new("AliasExpressionSegment")
                        .exclude(one_of(vec![
                            Ref::new("FromClauseTerminatorGrammar").to_matchable(),
                            Ref::new("SamplingExpressionSegment").to_matchable(),
                            Ref::new("JoinLikeClauseGrammar").to_matchable(),
                            LookaheadExclude::new("WITH", "(").to_matchable(),
                        ]))
                        .optional()
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("OFFSET").to_matchable(),
                        Ref::new("AliasExpressionSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("SamplingExpressionSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("PostTableExpressionGrammar")
                        .optional()
                        .to_matchable(),
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
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("SCHEMA").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("SchemaReferenceSegment").to_matchable(),
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
        Sequence::new(vec![
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("AliasExpressionSegment").to_matchable(),
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
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("GRANT").to_matchable(),
                    Ref::keyword("OPTION").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("ADMIN").to_matchable(),
                    Ref::keyword("OPTION").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("COPY").to_matchable(),
                    Ref::keyword("CURRENT").to_matchable(),
                    Ref::keyword("GRANTS").to_matchable(),
                ])
                .to_matchable(),
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
            Sequence::new(vec![
                // This should be used instead of a recursive call to Expression_A_Grammar
                // whenever the repeating element in Expression_A_Grammar makes a recursive
                // call to itself at the _end_.
                AnyNumberOf::new(vec![
                    Ref::new("Expression_A_Unary_Operator_Grammar").to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![Ref::new("BinaryOperatorGrammar").to_matchable()]
                })
                .to_matchable(),
                Ref::new("Expression_C_Grammar").to_matchable(),
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
            .config(|this| this.terminators = vec![Ref::new("CommaSegment").to_matchable()])
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
                .config(|this| this.terminators = vec![Ref::new("CommaSegment").to_matchable()])
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
            one_of(vec![
                optionally_bracketed(vec![
                    Ref::new("WithCompoundStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("NonWithSelectableGrammar").to_matchable(),
                Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()]).to_matchable(),
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
        // NOTE: In ANSI SQL, CTEs (WITH clause) can only precede SELECT statements,
        // not DML statements like INSERT/UPDATE/DELETE. This grammar is kept as a
        // hook point for dialects that support CTE+DML (e.g., PostgreSQL, SQL Server,
        // SQLite). Those dialects should either:
        // 1. Add DML statements to NonWithSelectableGrammar (like PostgreSQL), or
        // 2. Add WithCompoundNonSelectStatementSegment to their SelectableGrammar
        (
            "NonWithNonSelectableGrammar".into(),
            Nothing::new().to_matchable().into(),
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
        (
            "TimeWithTZGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("TIME").to_matchable(),
                    Ref::keyword("TIMESTAMP").to_matchable(),
                ])
                .to_matchable(),
                Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                    .config(|this| this.optional())
                    .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("WITHOUT").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("TIME").to_matchable(),
                    Ref::keyword("ZONE").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    ansi_dialect
}

pub fn select_clause_element() -> Matchable {
    one_of(vec![
        // *, blah.*, blah.blah.*, etc.
        Ref::new("WildcardExpressionSegment").to_matchable(),
        Sequence::new(vec![
            Ref::new("BaseExpressionElementGrammar").to_matchable(),
            Ref::new("AliasExpressionSegment").optional().to_matchable(),
        ])
        .to_matchable(),
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
        Matcher::regex("word", "[\\p{L}\\p{N}_]+", SyntaxKind::Word),
    ]
}

pub fn frame_extent() -> AnyNumberOf {
    one_of(vec![
        Sequence::new(vec![
            Ref::keyword("CURRENT").to_matchable(),
            Ref::keyword("ROW").to_matchable(),
        ])
        .to_matchable(),
        Sequence::new(vec![
            one_of(vec![
                Ref::new("NumericLiteralSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("INTERVAL").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("UNBOUNDED").to_matchable(),
            ])
            .to_matchable(),
            one_of(vec![
                Ref::keyword("PRECEDING").to_matchable(),
                Ref::keyword("FOLLOWING").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    ])
}

pub fn explainable_stmt() -> AnyNumberOf {
    one_of(vec![
        Ref::new("SelectableGrammar").to_matchable(),
        Ref::new("InsertStatementSegment").to_matchable(),
        Ref::new("UpdateStatementSegment").to_matchable(),
        Ref::new("DeleteStatementSegment").to_matchable(),
    ])
}

pub fn get_unordered_select_statement_segment_grammar() -> Matchable {
    Sequence::new(vec![
        Ref::new("SelectClauseSegment").to_matchable(),
        MetaSegment::dedent().to_matchable(),
        Ref::new("FromClauseSegment").optional().to_matchable(),
        Ref::new("WhereClauseSegment").optional().to_matchable(),
        Ref::new("GroupByClauseSegment").optional().to_matchable(),
        Ref::new("HavingClauseSegment").optional().to_matchable(),
        Ref::new("OverlapsClauseSegment").optional().to_matchable(),
        Ref::new("NamedWindowSegment").optional().to_matchable(),
    ])
    .terminators(vec![
        Ref::new("SetOperatorSegment").to_matchable(),
        Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
        Ref::new("WithDataClauseSegment").to_matchable(),
        Ref::new("OrderByClauseSegment").to_matchable(),
        Ref::new("LimitClauseSegment").to_matchable(),
    ])
    .config(|this| {
        this.parse_mode(ParseMode::GreedyOnceStarted);
    })
    .to_matchable()
}

pub fn select_statement() -> Matchable {
    get_unordered_select_statement_segment_grammar().copy(
        Some(vec![
            Ref::new("OrderByClauseSegment").optional().to_matchable(),
            Ref::new("FetchClauseSegment").optional().to_matchable(),
            Ref::new("LimitClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
        ]),
        None,
        None,
        None,
        vec![
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
        ],
        true,
    )
}

pub fn select_clause_segment() -> Matchable {
    Sequence::new(vec![
        Ref::keyword("SELECT").to_matchable(),
        Ref::new("SelectClauseModifierSegment")
            .optional()
            .to_matchable(),
        MetaSegment::indent().to_matchable(),
        Delimited::new(vec![Ref::new("SelectClauseElementSegment").to_matchable()])
            .config(|this| this.allow_trailing())
            .to_matchable(),
    ])
    .terminators(vec![
        Ref::new("SelectClauseTerminatorGrammar").to_matchable(),
    ])
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
    .config(|this| this.terminators = vec![Ref::new("DelimiterGrammar").to_matchable()])
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
    vec![
        Ref::keyword("FROM").to_matchable(),
        Ref::keyword("WHERE").to_matchable(),
        Sequence::new(vec![
            Ref::keyword("ORDER").to_matchable(),
            Ref::keyword("BY").to_matchable(),
        ])
        .to_matchable(),
        Ref::keyword("LIMIT").to_matchable(),
        Ref::keyword("OVERLAPS").to_matchable(),
        Ref::new("SetOperatorSegment").to_matchable(),
        Ref::keyword("FETCH").to_matchable(),
    ]
}
