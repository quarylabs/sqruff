// T-SQL (Transact-SQL) dialect implementation for Microsoft SQL Server

use itertools::Itertools;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{
    AnyNumberOf, any_set_of, one_of, optionally_bracketed,
};
use sqruff_lib_core::parser::grammar::conditional::Conditional;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Nothing, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::lookahead::LookaheadExclude;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{
    CaseFold, MultiStringParser, RegexParser, StringParser, TypedParser,
};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use crate::{ansi, tsql_keywords};
use sqruff_lib_core::dialects::init::DialectConfig;
use sqruff_lib_core::value::Value;

sqruff_lib_core::dialect_config!(TSQLDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    // Parse and validate dialect configuration, falling back to defaults on failure
    let _dialect_config: TSQLDialectConfig = config
        .map(TSQLDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    // Start with ANSI SQL as the base dialect and customize for T-SQL
    let mut dialect = ansi::raw_dialect();
    dialect.name = DialectKind::Tsql;

    // SQLFluff keeps the keyword parsers inherited from ANSI even after it
    // replaces the classification sets below. Materialize the equivalent
    // parser references before clearing those sets in sqruff.
    let keyword_references = dialect
        .sets("reserved_keywords")
        .into_iter()
        .chain(dialect.sets("unreserved_keywords"))
        .chain(tsql_keywords::tsql_additional_parser_keywords())
        .collect_vec();
    dialect.add(keyword_references.into_iter().map(|keyword| {
        (
            keyword.into(),
            StringParser::new(keyword, SyntaxKind::Keyword)
                .to_matchable()
                .into(),
        )
    }));

    // T-SQL has its own complete keyword classification. In particular,
    // future-reserved words are legal as unquoted identifiers today.
    dialect.sets_mut("reserved_keywords").clear();
    dialect.sets_mut("unreserved_keywords").clear();
    dialect.sets_mut("future_reserved_keywords").clear();
    dialect
        .sets_mut("reserved_keywords")
        .extend(tsql_keywords::tsql_reserved_keywords());
    dialect
        .sets_mut("unreserved_keywords")
        .extend(tsql_keywords::tsql_unreserved_keywords());
    dialect
        .sets_mut("future_reserved_keywords")
        .extend(tsql_keywords::tsql_future_keywords());
    dialect.replace_grammar("NanLiteralSegment", Nothing::new().to_matchable());

    dialect.replace_grammar(
        "ConditionalCrossJoinKeywordsGrammar",
        Nothing::new().to_matchable(),
    );
    dialect.replace_grammar(
        "NaturalJoinKeywordsGrammar",
        Ref::keyword("CROSS").to_matchable(),
    );

    // T-SQL permits a `WITH ROLLUP` clause after the `GROUP BY` expression list.
    dialect.add([(
        "WithRollupClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::WithRollupClause, |_| {
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Ref::keyword("ROLLUP").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.replace_grammar(
        "GroupByClauseSegment",
        Sequence::new(vec![
            Ref::keyword("GROUP").to_matchable(),
            Ref::keyword("BY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            one_of(vec![
                Ref::new("ColumnReferenceSegment").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable(),
            AnyNumberOf::new(vec![
                Ref::new("CommaSegment").to_matchable(),
                one_of(vec![
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("WithRollupClauseSegment")
                .optional()
                .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL specific operators
    dialect.sets_mut("operator_symbols").extend([
        "%=", "&=", "*=", "+=", "-=", "/=", "^=", "|=", // Compound assignment
        "!<", "!>", // Special comparison operators
    ]);

    // T-SQL hexadecimal and file-size literals must be tokenized before ordinary
    // numeric literals, which would otherwise consume only their numeric prefix.
    dialect.insert_lexer_matchers(
        vec![
            Matcher::regex(
                "hexadecimal_literal",
                r"([xX]'([\da-fA-F][\da-fA-F])+'|0[xX][\da-fA-F]*)",
                SyntaxKind::NumericLiteral,
            ),
            Matcher::regex(
                "size_literal",
                r"[0-9]+(?i:KB|MB|GB|TB)(?-u:\b)",
                SyntaxKind::SizeLiteral,
            ),
        ],
        "numeric_literal",
    );

    // T-SQL supports square brackets for identifiers and @ for variables
    // Insert square bracket identifier before individual bracket matchers to ensure it's matched first
    dialect.insert_lexer_matchers(
        vec![
            // Square brackets for identifiers: [Column Name]
            Matcher::regex(
                "tsql_square_bracket_identifier",
                r"\[[^\]]*\]",
                SyntaxKind::DoubleQuote,
            ),
        ],
        "start_square_bracket",
    );

    // Insert other T-SQL specific matchers
    dialect.insert_lexer_matchers(
        vec![
            // Local and global temporary table names, including numeric names.
            Matcher::regex(
                "hash_identifier",
                r"##?[a-zA-Z0-9_]+",
                SyntaxKind::HashIdentifier,
            ),
            // Variables: @MyVar (local) or @@ROWCOUNT (global/system)
            Matcher::regex(
                "tsql_variable",
                r"@@?[a-zA-Z_][a-zA-Z0-9_]*",
                SyntaxKind::TsqlVariable,
            ),
            // OUTPUT clauses in MERGE statements expose the special $ACTION value.
            Matcher::regex(
                "var_prefix",
                r"\$[a-zA-Z0-9_]+",
                SyntaxKind::ActionParameter,
            ),
        ],
        "equals",
    );

    // sqlcmd `:r <path>` relative file paths (e.g. .\folder\script.sql) (#4653)
    dialect.insert_lexer_matchers(
        vec![Matcher::regex(
            "unquoted_relative_sql_file_path",
            r"[.\w\\/#-]+\.[sS][qQ][lL](?-u:\b)",
            SyntaxKind::UnquotedRelativeSqlFilePath,
        )],
        "back_quote",
    );

    // T-SQL specific lexer patches:
    // 1. T-SQL only uses -- for inline comments, not # (which is used in temp table names)
    // 2. Allow Unicode letters and a trailing # (SQL Server 2017+ syntax).
    dialect.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r"--[^\n]*", SyntaxKind::InlineComment),
        Matcher::regex("word", r"[0-9a-zA-Z_\p{L}]+#?", SyntaxKind::Word),
    ]);

    // Since T-SQL uses square brackets as quoted identifiers and the lexer
    // already maps them to SyntaxKind::DoubleQuote, the ANSI QuotedIdentifierSegment
    // should handle them correctly. No additional parser configuration needed.

    // Add T-SQL specific bare functions
    dialect.sets_mut("bare_functions").extend([
        "CURRENT_TIMESTAMP",
        "CURRENT_USER",
        "SESSION_USER",
        "SYSTEM_USER",
        "USER",
    ]);

    // Add aggregate and other functions
    dialect
        .sets_mut("aggregate_functions")
        .extend(["STRING_AGG"]);

    dialect
        .sets_mut("special_functions")
        .extend(["COALESCE", "NULLIF", "ISNULL"]);

    // T-SQL datetime units
    dialect.sets_mut("datetime_units").extend([
        "YEAR",
        "YY",
        "YYYY",
        "QUARTER",
        "QQ",
        "Q",
        "MONTH",
        "MM",
        "M",
        "DAYOFYEAR",
        "DY",
        "Y",
        "DAY",
        "DD",
        "D",
        "WEEK",
        "WK",
        "WW",
        "WEEKDAY",
        "DW",
        "HOUR",
        "HH",
        "ISO_WEEK",
        "ISOWK",
        "ISOWW",
        "MINUTE",
        "MI",
        "N",
        "SECOND",
        "SS",
        "S",
        "TZ",
        "TZOFFSET",
        "MILLISECOND",
        "MS",
        "MICROSECOND",
        "MCS",
        "NANOSECOND",
        "NS",
    ]);

    // Add T-SQL specific date functions
    dialect.sets_mut("date_part_function_name").extend([
        "DATEADD",
        "DATEDIFF",
        "DATENAME",
        "DATEPART",
        "DATETRUNC",
        "DAY",
        "MONTH",
        "YEAR",
        "GETDATE",
        "GETUTCDATE",
        "SYSDATETIME",
        "SYSUTCDATETIME",
        "SYSDATETIMEOFFSET",
    ]);

    // Add T-SQL string functions
    dialect.sets_mut("scalar_functions").extend([
        "SUBSTRING",
        "CHARINDEX",
        "LEN",
        "LEFT",
        "RIGHT",
        "LTRIM",
        "RTRIM",
        "REPLACE",
        "STUFF",
        "PATINDEX",
        "QUOTENAME",
        "REPLICATE",
        "REVERSE",
        "SPACE",
        "STR",
        "UNICODE",
    ]);

    // T-SQL specific value table functions
    dialect.sets_mut("value_table_functions").extend([
        "OPENROWSET",
        "OPENQUERY",
        "OPENDATASOURCE",
        "OPENXML",
    ]);

    // Add T-SQL specific grammar

    // TOP clause support (e.g., SELECT TOP 10, TOP (10) PERCENT, TOP 5 WITH TIES)
    // T-SQL allows DISTINCT/ALL followed by TOP
    dialect.replace_grammar(
        "SelectClauseModifierSegment",
        AnyNumberOf::new(vec![
            Ref::keyword("DISTINCT").to_matchable(),
            Ref::keyword("ALL").to_matchable(),
            // TOP alone
            Sequence::new(vec![
                // https://docs.microsoft.com/en-us/sql/t-sql/queries/top-transact-sql
                Ref::keyword("TOP").to_matchable(),
                optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                    .to_matchable(),
                Ref::keyword("PERCENT").optional().to_matchable(),
                Ref::keyword("WITH").optional().to_matchable(),
                Ref::keyword("TIES").optional().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL ORDER BY supports OFFSET followed by an optional FETCH clause.
    dialect.add([(
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
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![Ref::new("OffsetClauseSegment").to_matchable()]
                })
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("OffsetClauseSegment").to_matchable(),
                    Ref::new("FetchClauseSegment").optional().to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "OffsetClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::OffsetClause, |_| {
            Sequence::new(vec![
                Ref::keyword("OFFSET").to_matchable(),
                one_of(vec![
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("ROW").to_matchable(),
                    Ref::keyword("ROWS").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL supports CTEs with DML statements (INSERT, UPDATE, DELETE, MERGE)
    // We add these to NonWithSelectableGrammar so WithCompoundStatementSegment can use them
    dialect.add([(
        "NonWithSelectableGrammar".into(),
        one_of(vec![
            Ref::new("SetExpressionSegment").to_matchable(),
            optionally_bracketed(vec![Ref::new("SelectStatementSegment").to_matchable()])
                .to_matchable(),
            Ref::new("NonSetSelectableGrammar").to_matchable(),
            Ref::new("OpenQueryUpdateStatementSegment").to_matchable(),
            Ref::new("UpdateStatementSegment").to_matchable(),
            Ref::new("OpenQueryInsertStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("OpenQueryDeleteStatementSegment").to_matchable(),
            Ref::new("DeleteStatementSegment").to_matchable(),
            Ref::new("MergeStatementSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // Add T-SQL assignment operator segments.
    dialect.add([
        (
            "AssignmentOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::AssignmentOperator, |_| {
                one_of(vec![
                    Ref::new("RawEqualsSegment").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("PlusSegment").to_matchable(),
                            Ref::new("MinusSegment").to_matchable(),
                            Ref::new("DivideSegment").to_matchable(),
                            Ref::new("MultiplySegment").to_matchable(),
                            Ref::new("ModuloSegment").to_matchable(),
                            Ref::new("BitwiseAndSegment").to_matchable(),
                            Ref::new("BitwiseOrSegment").to_matchable(),
                            Ref::new("BitwiseXorSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("RawEqualsSegment").to_matchable(),
                    ])
                    .config(|this| this.allow_gaps = false)
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PlusComparisonSegment".into(),
            StringParser::new("+", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "MinusComparisonSegment".into(),
            StringParser::new("-", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "MultiplyComparisonSegment".into(),
            StringParser::new("*", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "DivideComparisonSegment".into(),
            StringParser::new("/", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "ModuloComparisonSegment".into(),
            StringParser::new("%", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "AdditionAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Sequence::new(vec![
                    Ref::new("PlusComparisonSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SubtractionAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Sequence::new(vec![
                    Ref::new("MinusComparisonSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MultiplicationAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Sequence::new(vec![
                    Ref::new("MultiplyComparisonSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DivisionAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Sequence::new(vec![
                    Ref::new("DivideComparisonSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ModulusAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Sequence::new(vec![
                    Ref::new("ModuloComparisonSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "ArithmeticBinaryOperatorGrammar",
        one_of(vec![
            Ref::new("AdditionAssignmentSegment").to_matchable(),
            Ref::new("SubtractionAssignmentSegment").to_matchable(),
            Ref::new("MultiplicationAssignmentSegment").to_matchable(),
            Ref::new("DivisionAssignmentSegment").to_matchable(),
            Ref::new("ModulusAssignmentSegment").to_matchable(),
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
        .to_matchable(),
    );

    // T-SQL permits adjacent SELECT statements without semicolons. Preserve
    // the existing clauses while removing ANSI's greedy parsing and statement
    // terminators, then add the T-SQL result-formatting clause.
    for (name, grammar) in [
        ("SelectClauseSegment", ansi::select_clause_segment()),
        (
            "UnorderedSelectStatementSegment",
            ansi::get_unordered_select_statement_segment_grammar(),
        ),
    ] {
        dialect.replace_grammar(
            name,
            Sequence::new(grammar.elements().to_vec()).to_matchable(),
        );
    }
    let mut select_elements = ansi::select_statement().elements().to_vec();
    select_elements.push(Ref::new("ForClauseSegment").optional().to_matchable());
    dialect.replace_grammar(
        "SelectStatementSegment",
        Sequence::new(select_elements).to_matchable(),
    );

    dialect.add([
        (
            "ForClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ForClause, |_| {
                let optional_literal = || {
                    Bracketed::new(vec![Ref::new("LiteralGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable()
                };
                let common_xml_directives = AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("BINARY").to_matchable(),
                        Ref::keyword("BASE64").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("TYPE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ROOT").to_matchable(),
                        optional_literal(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable();
                let elements = Sequence::new(vec![
                    Ref::keyword("ELEMENTS").to_matchable(),
                    one_of(vec![
                        Ref::keyword("XSINIL").to_matchable(),
                        Ref::keyword("ABSENT").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable();

                Sequence::new(vec![
                    Ref::keyword("FOR").to_matchable(),
                    one_of(vec![
                        Ref::keyword("BROWSE").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("JSON").to_matchable(),
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::keyword("AUTO").to_matchable(),
                                    Ref::keyword("PATH").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ROOT").to_matchable(),
                                    optional_literal(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                                Ref::keyword("INCLUDE_NULL_VALUES")
                                    .optional()
                                    .to_matchable(),
                                Ref::keyword("WITHOUT_ARRAY_WRAPPER")
                                    .optional()
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("XML").to_matchable(),
                            one_of(vec![
                                Delimited::new(vec![
                                    Sequence::new(vec![
                                        Ref::keyword("PATH").to_matchable(),
                                        optional_literal(),
                                    ])
                                    .to_matchable(),
                                    common_xml_directives.clone(),
                                    elements.clone(),
                                ])
                                .to_matchable(),
                                Delimited::new(vec![
                                    Ref::keyword("EXPLICIT").to_matchable(),
                                    common_xml_directives.clone(),
                                    Ref::keyword("XMLDATA").optional().to_matchable(),
                                ])
                                .to_matchable(),
                                Delimited::new(vec![
                                    one_of(vec![
                                        Ref::keyword("AUTO").to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("RAW").to_matchable(),
                                            optional_literal(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                    common_xml_directives,
                                    elements,
                                    one_of(vec![
                                        Ref::keyword("XMLDATA").to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("XMLSCHEMA").to_matchable(),
                                            optional_literal(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .config(|this| this.optional())
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "JsonFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                one_of(vec![
                    Ref::keyword("JSON_ARRAY").to_matchable(),
                    Ref::keyword("JSON_OBJECT").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "JsonFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_| {
                let null_clause = one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("NULL").to_matchable(),
                        Ref::keyword("ON").to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ABSENT").to_matchable(),
                        Ref::keyword("ON").to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable();
                let key_value = Sequence::new(vec![
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("ParameterNameSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ColonSegment").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("LiteralGrammar").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                            Ref::new("FunctionSegment").to_matchable(),
                            Bracketed::new(vec![Ref::new("SelectStatementSegment").to_matchable()])
                                .to_matchable(),
                            Ref::keyword("NULL").to_matchable(),
                        ])
                        .to_matchable(),
                        null_clause.clone(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable();
                one_of(vec![
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            AnyNumberOf::new(vec![
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                Ref::new("ParameterNameSegment").to_matchable(),
                                Ref::keyword("NULL").to_matchable(),
                                null_clause.clone(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![key_value, null_clause]).to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ReplicateFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                Ref::keyword("REPLICATE").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ReplicateFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_| {
                Bracketed::new(vec![
                    one_of(vec![
                        Ref::new("ExpressionSegment").to_matchable(),
                        Ref::new("HexadecimalLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("CommaSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "FunctionSegment",
        one_of(vec![
            Ref::new("ColumnsExpressionGrammar").to_matchable(),
            Sequence::new(vec![
                Ref::new("DatePartFunctionNameSegment").to_matchable(),
                Ref::new("DateTimeFunctionContentsSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::new("ReplicateFunctionNameSegment").to_matchable(),
                Ref::new("ReplicateFunctionContentsSegment").to_matchable(),
            ])
            .to_matchable(),
            // Try the JSON syntax before generic function arguments, which can
            // otherwise misclassify ON NULL as a typed literal expression.
            Sequence::new(vec![
                Ref::new("JsonFunctionNameSegment").to_matchable(),
                Ref::new("JsonFunctionContentsSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::new("FunctionNameSegment")
                        .exclude(one_of(vec![
                            Ref::new("DatePartFunctionNameSegment").to_matchable(),
                            Ref::new("ColumnsExpressionFunctionNameSegment").to_matchable(),
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
        .to_matchable(),
    );

    dialect.add([(
        "EqualAliasOperatorSegment".into(),
        NodeMatcher::new(SyntaxKind::AliasOperator, |_| {
            Ref::new("RawEqualsSegment").to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override identifier handling for T-SQL identifiers ending in # and
    // temporary table names beginning with # or ##.
    dialect.add([
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({pattern})$");

                RegexParser::new(
                    r"[A-Za-z0-9_\p{L}]*[A-Za-z\p{L}][A-Za-z0-9_\p{L}]*#?",
                    SyntaxKind::NakedIdentifier,
                )
                .anti_template(&anti_template)
                .casefold(CaseFold::Upper)
                .to_matchable()
            })
            .into(),
        ),
        (
            "HashIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::HashIdentifier, SyntaxKind::HashIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "HexadecimalLiteralSegment".into(),
            RegexParser::new(
                r"([xX]'([\da-fA-F][\da-fA-F])+'|0[xX][\da-fA-F]*)",
                SyntaxKind::NumericLiteral,
            )
            .to_matchable()
            .into(),
        ),
        (
            "PercentSegment".into(),
            TypedParser::new(SyntaxKind::Percent, SyntaxKind::Percent)
                .to_matchable()
                .into(),
        ),
        (
            "IntegerLiteralSegment".into(),
            RegexParser::new(r"[0-9]+", SyntaxKind::IntegerLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "BinaryLiteralSegment".into(),
            RegexParser::new(r"0[xX][\da-fA-F]*", SyntaxKind::BinaryLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "SizeLiteralSegment".into(),
            TypedParser::new(SyntaxKind::SizeLiteral, SyntaxKind::SizeLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "ActionParameterSegment".into(),
            RegexParser::new(r"\$ACTION", SyntaxKind::ActionParameter)
                .to_matchable()
                .into(),
        ),
        (
            "NakedOrQuotedIdentifierGrammar".into(),
            one_of(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::new("QuotedIdentifierSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([(
        "SetContextInfoSegment".into(),
        NodeMatcher::new(SyntaxKind::SetContextInfoStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                Ref::keyword("CONTEXT_INFO").to_matchable(),
                one_of(vec![
                    Ref::new("HexadecimalLiteralSegment").to_matchable(),
                    Ref::new("ParameterNameSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "SetLanguageStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::SetLanguageStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                Ref::keyword("LANGUAGE").to_matchable(),
                one_of(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    // T-SQL square-bracket identifiers are lexed as quoted identifiers.
                    Ref::new("QuotedIdentifierSegment").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.replace_grammar(
        "SingleIdentifierGrammar",
        one_of(vec![
            Ref::new("NakedIdentifierSegment").to_matchable(),
            Ref::new("QuotedIdentifierSegment").to_matchable(),
            Ref::new("HashIdentifierSegment").to_matchable(),
        ])
        .config(|this| this.terminators = vec![Ref::new("DotSegment").to_matchable()])
        .to_matchable(),
    );

    // Cursor definitions and cursor statement support.
    dialect.add([
        (
            "CursorNameGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("GLOBAL").optional().to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("ParameterNameSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "CursorDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::CursorDefinition, |_| {
                Sequence::new(vec![
                    Ref::keyword("CURSOR").to_matchable(),
                    one_of(vec![
                        Ref::keyword("LOCAL").to_matchable(),
                        Ref::keyword("GLOBAL").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("FORWARD_ONLY").to_matchable(),
                        Ref::keyword("SCROLL").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("STATIC").to_matchable(),
                        Ref::keyword("KEYSET").to_matchable(),
                        Ref::keyword("DYNAMIC").to_matchable(),
                        Ref::keyword("FAST_FORWARD").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("READ_ONLY").to_matchable(),
                        Ref::keyword("SCROLL_LOCKS").to_matchable(),
                        Ref::keyword("OPTIMISTIC").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("TYPE_WARNING").optional().to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("SelectStatementSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DeclareCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeclareSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Ref::new("CursorNameGrammar").to_matchable(),
                    one_of(vec![
                        Ref::new("CursorDefinitionSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INSENSITIVE").optional().to_matchable(),
                            Ref::keyword("SCROLL").optional().to_matchable(),
                            Ref::keyword("CURSOR").to_matchable(),
                            Ref::keyword("FOR").to_matchable(),
                            Ref::new("SelectStatementSegment").to_matchable(),
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
            "OpenCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OpenCursorStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("OPEN").to_matchable(),
                    Ref::keyword("GLOBAL").optional().to_matchable(),
                    Ref::new("CursorNameGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CloseCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CloseCursorStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CLOSE").to_matchable(),
                    Ref::keyword("GLOBAL").optional().to_matchable(),
                    Ref::new("CursorNameGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DeallocateCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeallocateCursorStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DEALLOCATE").to_matchable(),
                    Ref::keyword("GLOBAL").optional().to_matchable(),
                    Ref::new("CursorNameGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FetchCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::FetchCursorStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("FETCH").to_matchable(),
                    one_of(vec![
                        Ref::keyword("NEXT").to_matchable(),
                        Ref::keyword("PRIOR").to_matchable(),
                        Ref::keyword("FIRST").to_matchable(),
                        Ref::keyword("LAST").to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("ABSOLUTE").to_matchable(),
                                Ref::keyword("RELATIVE").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("SignedSegmentGrammar").optional().to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("FROM").optional().to_matchable(),
                    Ref::new("CursorNameGrammar").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("INTO").to_matchable(),
                        Delimited::new(vec![Ref::new("ParameterNameSegment").to_matchable()])
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
    ]);

    // DECLARE statement for variable declarations
    // Syntax: DECLARE @var1 INT = 10, @var2 VARCHAR(50) = 'text'
    dialect.add([
        (
            "DeclareStatementSegment".into(),
            Ref::new("DeclareStatementGrammar").to_matchable().into(),
        ),
        (
            "DeclareStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("DECLARE").to_matchable(),
                // Multiple variables can be declared with comma separation
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::new("TsqlVariableSegment").to_matchable(),
                        Sequence::new(vec![Ref::keyword("AS").to_matchable()])
                            .config(|this| this.optional())
                            .to_matchable(),
                        one_of(vec![
                            Ref::keyword("CURSOR").to_matchable(),
                            // Regular variable declaration
                            Sequence::new(vec![
                                Ref::new("DatatypeSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("AssignmentOperatorSegment").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            // Table variable declaration
                            Sequence::new(vec![
                                Ref::keyword("TABLE").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Ref::new("TableConstraintSegment").to_matchable(),
                                        Ref::new("ComputedColumnDefinitionSegment").to_matchable(),
                                        Ref::new("ColumnDefinitionSegment").to_matchable(),
                                    ])
                                    .config(|this| this.allow_trailing())
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
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // SET local variable statements, including cursor-valued variables.
    dialect.add([(
        "SetLocalVariableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::SetLocalVariableSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Delimited::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("ParameterNameSegment").to_matchable(),
                            Ref::new("AssignmentOperatorSegment").to_matchable(),
                            one_of(vec![
                                Ref::new("ExpressionSegment").to_matchable(),
                                Ref::new("SelectableGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("ParameterNameSegment").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            one_of(vec![
                                Ref::new("ParameterNameSegment").to_matchable(),
                                Ref::new("NakedIdentifierSegment").to_matchable(),
                                Ref::new("CursorDefinitionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // SET statements support session options.
    dialect.add([
        (
            "SetVariableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetSegment, |_| {
                Ref::new("SetVariableStatementGrammar").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetVariableStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                Delimited::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("TRANSACTION").to_matchable(),
                            Ref::keyword("ISOLATION").to_matchable(),
                            Ref::keyword("LEVEL").to_matchable(),
                            one_of(vec![
                                Ref::keyword("SNAPSHOT").to_matchable(),
                                Ref::keyword("SERIALIZABLE").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("REPEATABLE").to_matchable(),
                                    Ref::keyword("READ").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("READ").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("COMMITTED").to_matchable(),
                                        Ref::keyword("UNCOMMITTED").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::keyword("DATEFIRST").to_matchable(),
                                    Ref::keyword("DATEFORMAT").to_matchable(),
                                    Ref::keyword("DEADLOCK_PRIORITY").to_matchable(),
                                    Ref::keyword("LOCK_TIMEOUT").to_matchable(),
                                    Ref::keyword("CONCAT_NULL_YIELDS_NULL").to_matchable(),
                                    Ref::keyword("CURSOR_CLOSE_ON_COMMIT").to_matchable(),
                                    Ref::keyword("FIPS_FLAGGER").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("IDENTITY_INSERT").to_matchable(),
                                        Ref::new("TableReferenceSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::keyword("LANGUAGE").to_matchable(),
                                    Ref::keyword("OFFSETS").to_matchable(),
                                    Ref::keyword("QUOTED_IDENTIFIER").to_matchable(),
                                    Ref::keyword("ARITHABORT").to_matchable(),
                                    Ref::keyword("ARITHIGNORE").to_matchable(),
                                    Ref::keyword("FMTONLY").to_matchable(),
                                    Ref::keyword("NOCOUNT").to_matchable(),
                                    Ref::keyword("NOEXEC").to_matchable(),
                                    Ref::keyword("NUMERIC_ROUNDABORT").to_matchable(),
                                    Ref::keyword("PARSEONLY").to_matchable(),
                                    Ref::keyword("QUERY_GOVERNOR_COST_LIMIT").to_matchable(),
                                    Ref::keyword("RESULT_SET_CACHING").to_matchable(),
                                    Ref::keyword("ROWCOUNT").to_matchable(),
                                    Ref::keyword("TEXTSIZE").to_matchable(),
                                    Ref::keyword("ANSI_DEFAULTS").to_matchable(),
                                    Ref::keyword("ANSI_NULL_DFLT_OFF").to_matchable(),
                                    Ref::keyword("ANSI_NULL_DFLT_ON").to_matchable(),
                                    Ref::keyword("ANSI_NULLS").to_matchable(),
                                    Ref::keyword("ANSI_PADDING").to_matchable(),
                                    Ref::keyword("ANSI_WARNINGS").to_matchable(),
                                    Ref::keyword("FORCEPLAN").to_matchable(),
                                    Ref::keyword("SHOWPLAN_ALL").to_matchable(),
                                    Ref::keyword("SHOWPLAN_TEXT").to_matchable(),
                                    Ref::keyword("SHOWPLAN_XML").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("STATISTICS").to_matchable(),
                                        one_of(vec![
                                            Ref::keyword("IO").to_matchable(),
                                            Ref::keyword("PROFILE").to_matchable(),
                                            Ref::keyword("TIME").to_matchable(),
                                            Ref::keyword("XML").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::keyword("IMPLICIT_TRANSACTIONS").to_matchable(),
                                    Ref::keyword("REMOTE_PROC_TRANSACTIONS").to_matchable(),
                                    Ref::keyword("XACT_ABORT").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            one_of(vec![
                                Ref::keyword("ON").to_matchable(),
                                Ref::keyword("OFF").to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("EqualsSegment").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("LOW").to_matchable(),
                                Ref::keyword("NORMAL").to_matchable(),
                                Ref::keyword("HIGH").to_matchable(),
                                Ref::new("TsqlVariableSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // PRINT statement
    dialect.add([
        (
            "PrintStatementSegment".into(),
            Ref::new("PrintStatementGrammar").to_matchable().into(),
        ),
        (
            "PrintStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("PRINT").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // RAISERROR is a statement in T-SQL and is commonly used inside trigger
    // bodies.
    dialect.add([(
        "RaiserrorStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::RaiserrorStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("RAISERROR").to_matchable(),
                Bracketed::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("CommaSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::new("CommaSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::new("CommaSegment").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Delimited::new(vec![
                        Ref::keyword("LOG").to_matchable(),
                        Ref::keyword("NOWAIT").to_matchable(),
                        Ref::keyword("SETERROR").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ReturnStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::ReturnSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("RETURN").to_matchable(),
                Ref::new("ExpressionSegment").optional().to_matchable(),
                Ref::new("DelimiterGrammar").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // BEGIN...END blocks for grouping multiple statements
    dialect.add([
        (
            "BeginEndBlockSegment".into(),
            Sequence::new(vec![
                Ref::keyword("BEGIN").to_matchable(),
                MetaSegment::indent().to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("SelectableGrammar").to_matchable(),
                            Ref::new("OpenQueryInsertStatementSegment").to_matchable(),
                            Ref::new("InsertStatementSegment").to_matchable(),
                            Ref::new("OpenQueryUpdateStatementSegment").to_matchable(),
                            Ref::new("UpdateStatementSegment").to_matchable(),
                            Ref::new("OpenQueryDeleteStatementSegment").to_matchable(),
                            Ref::new("DeleteStatementSegment").to_matchable(),
                            Ref::new("CreateTableStatementSegment").to_matchable(),
                            Ref::new("DropTableStatementSegment").to_matchable(),
                            Ref::new("OpenSymmetricKeySegment").to_matchable(),
                            Ref::new("DeclareCursorStatementSegment").to_matchable(),
                            Ref::new("OpenCursorStatementSegment").to_matchable(),
                            Ref::new("FetchCursorStatementSegment").to_matchable(),
                            Ref::new("CloseCursorStatementSegment").to_matchable(),
                            Ref::new("DeallocateCursorStatementSegment").to_matchable(),
                            Ref::new("DeclareStatementSegment").to_matchable(),
                            Ref::new("SetLanguageStatementSegment").to_matchable(),
                            Ref::new("SetVariableStatementSegment").to_matchable(),
                            Ref::new("SetLocalVariableStatementSegment").to_matchable(),
                            Ref::new("WaitForStatementSegment").to_matchable(),
                            Ref::new("ExecuteScriptSegment").to_matchable(),
                            Ref::new("PrintStatementSegment").to_matchable(),
                            Ref::new("RaiserrorStatementSegment").to_matchable(),
                            Ref::new("ReturnStatementSegment").to_matchable(),
                            Ref::new("IfStatementSegment").to_matchable(),
                            Ref::new("WhileStatementSegment").to_matchable(),
                            Ref::new("TryBlockSegment").to_matchable(),
                            Ref::new("GotoStatementSegment").to_matchable(),
                            Ref::new("LabelSegment").to_matchable(),
                            Ref::new("BeginEndBlockSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DelimiterGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![
                        // Terminate on END keyword
                        Ref::keyword("END").to_matchable(),
                        // Also terminate on statement keywords to help with boundary detection
                        Ref::keyword("SELECT").to_matchable(),
                        Ref::keyword("INSERT").to_matchable(),
                        Ref::keyword("UPDATE").to_matchable(),
                        Ref::keyword("DELETE").to_matchable(),
                        Ref::keyword("CREATE").to_matchable(),
                        Ref::keyword("DROP").to_matchable(),
                        Ref::keyword("DECLARE").to_matchable(),
                        Ref::keyword("SET").to_matchable(),
                        Ref::keyword("PRINT").to_matchable(),
                        Ref::keyword("IF").to_matchable(),
                        Ref::keyword("WHILE").to_matchable(),
                        Ref::keyword("BEGIN").to_matchable(),
                        Ref::keyword("GOTO").to_matchable(),
                    ];
                })
                .config(|this| this.min_times(0))
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BeginEndBlockGrammar".into(),
            Ref::new("BeginEndBlockSegment").to_matchable().into(),
        ),
    ]);

    // TRY...CATCH blocks
    dialect.add([(
        "TryBlockSegment".into(),
        Sequence::new(vec![
            Ref::keyword("BEGIN").to_matchable(),
            Ref::keyword("TRY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.terminators = vec![Ref::keyword("END").to_matchable()];
            })
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::keyword("END").to_matchable(),
            Ref::keyword("TRY").to_matchable(),
            Ref::keyword("BEGIN").to_matchable(),
            Ref::keyword("CATCH").to_matchable(),
            MetaSegment::indent().to_matchable(),
            // A CATCH block may be empty.
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.terminators = vec![Ref::keyword("END").to_matchable()];
            })
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::keyword("END").to_matchable(),
            Ref::keyword("CATCH").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // GOTO statement and labels
    dialect.add([
        (
            "GotoStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("GOTO").to_matchable(),
                Ref::new("NakedIdentifierSegment").to_matchable(), // Label name
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LabelSegment".into(),
            Sequence::new(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(), // Label name
                Ref::new("ColonSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // IF...ELSE statement
    dialect.add([
        (
            "IfStatementSegment".into(),
            Ref::new("IfStatementGrammar").to_matchable().into(),
        ),
        (
            "IfStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("IF").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::new("StatementSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ELSE").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // WHILE loop
    dialect.add([
        (
            "WhileStatementSegment".into(),
            Ref::new("WhileStatementGrammar").to_matchable().into(),
        ),
        (
            "WhileStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("WHILE").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::new("StatementSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Pivot aliases must remain inside the pivot expression for rule analysis.
    dialect.add([
        (
            "PivotColumnReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::PivotColumnReference, |_| {
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
            "PivotUnpivotStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::FromPivotExpression, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("PIVOT").to_matchable(),
                            optionally_bracketed(vec![
                                optionally_bracketed(vec![
                                    Ref::new("FunctionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("FOR").to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                Ref::keyword("IN").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Ref::new("PivotColumnReferenceSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("UNPIVOT").to_matchable(),
                            optionally_bracketed(vec![
                                optionally_bracketed(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("FOR").to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                Ref::keyword("IN").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Ref::new("PivotColumnReferenceSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("AS").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Override TransactionStatementSegment to require TRANSACTION/WORK after BEGIN
    // This prevents BEGIN from being parsed as a transaction when it should be a BEGIN...END block
    dialect.replace_grammar(
        "TransactionStatementSegment",
        NodeMatcher::new(SyntaxKind::TransactionStatement, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("START").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("BEGIN").to_matchable(),
                        one_of(vec![
                            Ref::keyword("TRANSACTION").to_matchable(),
                            Ref::keyword("WORK").to_matchable(),
                            Ref::keyword("TRAN").to_matchable(), // T-SQL also supports TRAN
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("COMMIT").to_matchable(),
                    Ref::keyword("ROLLBACK").to_matchable(),
                    Ref::keyword("SAVE").to_matchable(), // T-SQL savepoints
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("TRANSACTION").to_matchable(),
                    Ref::keyword("WORK").to_matchable(),
                    Ref::keyword("TRAN").to_matchable(), // T-SQL abbreviation
                ])
                .config(|this| this.optional())
                .to_matchable(),
                // Optional transaction/savepoint name
                Ref::new("SingleIdentifierGrammar")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // T-SQL files are sequences of batches separated by GO statements.
    dialect.add([
        (
            "BatchSeparatorSegment".into(),
            Ref::new("BatchSeparatorGrammar").to_matchable().into(),
        ),
        (
            "BatchSeparatorGrammar".into(),
            Ref::keyword("GO").to_matchable().into(),
        ),
        (
            "BatchDelimiterGrammar".into(),
            Ref::new("GoStatementSegment").to_matchable().into(),
        ),
        (
            "StatementAndDelimiterGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OneOrMoreStatementsGrammar".into(),
            AnyNumberOf::new(vec![
                Ref::new("StatementAndDelimiterGrammar").to_matchable(),
            ])
            .config(|this| this.min_times(1))
            .to_matchable()
            .into(),
        ),
        (
            "GoStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::GoStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("GO").to_matchable(),
                    Ref::new("IntegerLiteralSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BatchSegment".into(),
            NodeMatcher::new(SyntaxKind::Batch, |_| {
                Sequence::new(vec![
                    AnyNumberOf::new(vec![Ref::new("DelimiterGrammar").to_matchable()])
                        .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                            Ref::new("BatchDelimiterGrammar").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("BatchDelimiterGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("DelimiterGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Override FileSegment to handle T-SQL batch separators (GO statements)
    dialect.replace_grammar(
        "FileSegment",
        AnyNumberOf::new(vec![Ref::new("BatchSegment").to_matchable()]).to_matchable(),
    );

    dialect.add([(
        "ReconfigureStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::ReconfigureStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("RECONFIGURE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("OVERRIDE").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "CreateColumnstoreIndexStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateColumnstoreIndexStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                one_of(vec![
                    Ref::keyword("CLUSTERED").to_matchable(),
                    Ref::keyword("NONCLUSTERED").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("COLUMNSTORE").to_matchable(),
                Ref::keyword("INDEX").to_matchable(),
                Ref::new("IndexReferenceSegment").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("BracketedColumnReferenceListGrammar")
                    .optional()
                    .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("WhereClauseSegment").optional().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("DROP_EXISTING").to_matchable(),
                                Ref::new("EqualsSegment").optional().to_matchable(),
                                one_of(vec![
                                    Ref::keyword("ON").to_matchable(),
                                    Ref::keyword("OFF").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("MAXDOP").to_matchable(),
                                Ref::new("EqualsSegment").optional().to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ONLINE").to_matchable(),
                                Ref::new("EqualsSegment").optional().to_matchable(),
                                one_of(vec![
                                    Ref::keyword("ON").to_matchable(),
                                    Ref::keyword("OFF").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("COMPRESSION_DELAY").to_matchable(),
                                Ref::new("EqualsSegment").optional().to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Ref::keyword("MINUTES").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("DATA_COMPRESSION").to_matchable(),
                                Ref::new("EqualsSegment").optional().to_matchable(),
                                one_of(vec![
                                    Ref::keyword("COLUMNSTORE").to_matchable(),
                                    Ref::keyword("COLUMNSTORE_ARCHIVE").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ON").to_matchable(),
                                    Ref::keyword("PARTITIONS").to_matchable(),
                                    Bracketed::new(vec![
                                        Delimited::new(vec![
                                            Ref::new("NumericLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("TO").to_matchable(),
                                            Ref::new("NumericLiteralSegment").to_matchable(),
                                        ])
                                        .config(|this| this.optional())
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .config(|this| this.optional())
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
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "AlterTableSwitchStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterTableSwitchStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::keyword("SWITCH").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("TO").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    one_of(vec![
                        Bracketed::new(vec![
                            Ref::keyword("WAIT_AT_LOW_PRIORITY").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Sequence::new(vec![
                                        Ref::keyword("MAX_DURATION").to_matchable(),
                                        Ref::new("EqualsSegment").to_matchable(),
                                        Ref::new("NumericLiteralSegment").to_matchable(),
                                        Ref::keyword("MINUTES").optional().to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("ABORT_AFTER_WAIT").to_matchable(),
                                        Ref::new("EqualsSegment").to_matchable(),
                                        one_of(vec![
                                            Ref::keyword("NONE").to_matchable(),
                                            Ref::keyword("SELF").to_matchable(),
                                            Ref::keyword("BLOCKERS").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![
                            Ref::keyword("TRUNCATE_TARGET").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ON").to_matchable(),
                                Ref::keyword("OFF").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("DelimiterGrammar").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Preserve N-prefixed procedure arguments as literal components when the
    // lexer represents the prefix and quoted string as separate tokens.
    dialect.add([(
        "ExecuteUnicodeLiteralExpressionSegment".into(),
        NodeMatcher::new(SyntaxKind::Expression, |_| {
            Sequence::new(vec![
                StringParser::new("N", SyntaxKind::NakedIdentifier).to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ExecuteOptionSegment".into(),
        NodeMatcher::new(SyntaxKind::ExecuteOption, |_| {
            let _result_sets_definition = one_of(vec![
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("COLLATE").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            one_of(vec![
                                Ref::keyword("NULL").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("NOT").to_matchable(),
                                    Ref::keyword("NULL").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    Ref::keyword("OBJECT").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("SingleIdentifierGrammar")
                            .optional()
                            .to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    Ref::keyword("TYPE").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        Ref::new("DotSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::keyword("XML").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable();
            one_of(vec![
                Ref::keyword("RECOMPILE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("RESULT").to_matchable(),
                    Ref::keyword("SETS").to_matchable(),
                    Ref::keyword("UNDEFINED").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("RESULT").to_matchable(),
                    Ref::keyword("SETS").to_matchable(),
                    Ref::keyword("NONE").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("RESULT").to_matchable(),
                    Ref::keyword("SETS").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![_result_sets_definition.clone()]).to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "LoginUserSegment".into(),
        NodeMatcher::new(SyntaxKind::LoginUserSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("AS").to_matchable(),
                one_of(vec![
                    Ref::keyword("LOGIN").to_matchable(),
                    Ref::keyword("USER").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("RawEqualsSegment").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "ExecuteScriptSegment".into(),
        NodeMatcher::new(SyntaxKind::ExecuteScriptStatement, |_| {
            let _execute_stored_procedure_or_function = Sequence::new(vec![
                Sequence::new(vec![
                    Ref::new("ParameterNameSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("SemicolonSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ParameterNameSegment").to_matchable(),
                ])
                .to_matchable(),
                MetaSegment::indent().to_matchable(),
                AnyNumberOf::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Sequence::new(vec![
                                Ref::new("ParameterNameSegment").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            one_of(vec![
                                Ref::new("ExecuteUnicodeLiteralExpressionSegment").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("ParameterNameSegment").to_matchable(),
                                    Sequence::new(vec![Ref::keyword("OUTPUT").to_matchable()])
                                        .config(|this| this.optional())
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("DEFAULT").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::new("ExecuteOptionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable();
            let _execute_a_characters_string = Sequence::new(vec![
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("ParameterNameSegment").to_matchable(),
                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.delimiter(Ref::new("PlusSegment"));
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("LoginUserSegment").optional().to_matchable(),
            ])
            .to_matchable();
            let _execute_pass_through_command = Sequence::new(vec![
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("ParameterNameSegment").to_matchable(),
                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.delimiter(Ref::new("PlusSegment"));
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("CommaSegment").to_matchable(),
                        Delimited::new(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::new("ExpressionSegment").to_matchable(),
                                    Ref::new("ParameterNameSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![Ref::keyword("OUTPUT").to_matchable()])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("LoginUserSegment").optional().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AT").to_matchable(),
                    Sequence::new(vec![Ref::keyword("DATA_SOURCE").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable();
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("EXEC").to_matchable(),
                    Ref::keyword("EXECUTE").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                one_of(vec![
                    _execute_stored_procedure_or_function.clone(),
                    _execute_a_characters_string.clone(),
                    _execute_pass_through_command.clone(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Context required by upstream EXECUTE regression fixtures.
    dialect.add([(
        "InsertStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("INSERT").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("INTO").optional().to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("OpenQuerySegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("PostTableExpressionGrammar")
                    .optional()
                    .to_matchable(),
                Ref::new("BracketedColumnReferenceListGrammar")
                    .optional()
                    .to_matchable(),
                Ref::new("OutputClauseSegment").optional().to_matchable(),
                one_of(vec![
                    Ref::new("SelectableGrammar").to_matchable(),
                    Ref::new("ExecuteScriptSegment").to_matchable(),
                    Ref::new("DefaultValuesGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([
        (
            "TopPercentGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("TOP").to_matchable(),
                optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                    .to_matchable(),
                Ref::keyword("PERCENT").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "MergeIntoLiteralGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("MERGE").to_matchable(),
                Ref::new("TopPercentGrammar").optional().to_matchable(),
                Ref::keyword("INTO").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DELETE").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("TopPercentGrammar").optional().to_matchable(),
                            Ref::keyword("FROM").optional().to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Sequence::new(vec![
                                        Ref::keyword("OPENDATASOURCE").to_matchable(),
                                        Bracketed::new(vec![
                                            Ref::new("QuotedLiteralSegment").to_matchable(),
                                            Ref::new("CommaSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Ref::new("DotSegment").to_matchable(),
                                    ])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                                    Ref::new("TableReferenceSegment").to_matchable(),
                                    Ref::new("PostTableExpressionGrammar")
                                        .optional()
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("OPENQUERY").to_matchable(),
                                    Bracketed::new(vec![
                                        Ref::new("NakedIdentifierSegment").to_matchable(),
                                        Ref::new("CommaSegment").to_matchable(),
                                        Ref::new("QuotedLiteralSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("OpenRowSetSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("OutputClauseSegment").optional().to_matchable(),
                            Ref::new("FromClauseSegment").optional().to_matchable(),
                            one_of(vec![
                                Ref::new("WhereClauseSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("WHERE").to_matchable(),
                                    Ref::keyword("CURRENT").to_matchable(),
                                    Ref::keyword("OF").to_matchable(),
                                    Ref::new("CursorNameGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FROM").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                            Ref::keyword("JOIN").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                            Ref::new("JoinOnConditionSegment").to_matchable(),
                            Ref::new("WhereClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("OpenQuerySegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("OptionClauseSegment").optional().to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
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
                    MetaSegment::indent().to_matchable(),
                    one_of(vec![
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::new("AliasedTableReferenceGrammar").to_matchable(),
                        Ref::new("OpenQuerySegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("PostTableExpressionGrammar")
                        .optional()
                        .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::new("SetClauseListSegment").to_matchable(),
                    Ref::new("OutputClauseSegment").optional().to_matchable(),
                    Ref::new("FromClauseSegment").optional().to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                    Ref::new("OptionClauseSegment").optional().to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable()
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
            "SetClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SetClause, |_| {
                Sequence::new(vec![
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::new("AssignmentOperatorSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeMatchSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeMatch, |_| {
                Sequence::new(vec![
                    AnyNumberOf::new(vec![
                        Ref::new("MergeMatchedClauseSegment").to_matchable(),
                        Ref::new("MergeNotMatchedClauseSegment").to_matchable(),
                    ])
                    .config(|this| this.min_times(1))
                    .to_matchable(),
                    Ref::new("OutputClauseSegment").optional().to_matchable(),
                    Ref::new("OptionClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeNotMatchedClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeWhenNotMatchedClause, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("WHEN").to_matchable(),
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("MATCHED").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("BY").to_matchable(),
                            Ref::keyword("TARGET").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("AND").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::keyword("THEN").to_matchable(),
                        Ref::new("MergeInsertClauseSegment").to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WHEN").to_matchable(),
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("MATCHED").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::keyword("SOURCE").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("AND").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::keyword("THEN").to_matchable(),
                        one_of(vec![
                            Ref::new("MergeUpdateClauseSegment").to_matchable(),
                            Ref::new("MergeDeleteClauseSegment").to_matchable(),
                        ])
                        .to_matchable(),
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
            "OptionClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OptionClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("OPTION").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("QueryHintSegment").to_matchable()])
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
            "QueryHintSegment".into(),
            NodeMatcher::new(SyntaxKind::QueryHintSegment, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("LABEL").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("HASH").to_matchable(),
                            Ref::keyword("ORDER").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("GROUP").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("MERGE").to_matchable(),
                            Ref::keyword("HASH").to_matchable(),
                            Ref::keyword("CONCAT").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("UNION").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("LOOP").to_matchable(),
                            Ref::keyword("MERGE").to_matchable(),
                            Ref::keyword("HASH").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("JOIN").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("EXPAND").to_matchable(),
                        Ref::keyword("VIEWS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("FAST").to_matchable(),
                            Ref::keyword("MAXDOP").to_matchable(),
                            Ref::keyword("MAXRECURSION").to_matchable(),
                            Ref::keyword("QUERYTRACEON").to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("MAX_GRANT_PERCENT").to_matchable(),
                                    Ref::keyword("MIN_GRANT_PERCENT").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FORCE").to_matchable(),
                        Ref::keyword("ORDER").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("FORCE").to_matchable(),
                            Ref::keyword("DISABLE").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("EXTERNALPUSHDOWN").to_matchable(),
                            Ref::keyword("SCALEOUTEXECUTION").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("KEEP").to_matchable(),
                            Ref::keyword("KEEPFIXED").to_matchable(),
                            Ref::keyword("ROBUST").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("PLAN").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("IGNORE_NONCLUSTERED_COLUMNSTORE_INDEX").to_matchable(),
                    Ref::keyword("NO_PERFORMANCE_SPOOL").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OPTIMIZE").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        one_of(vec![
                            Ref::keyword("UNKNOWN").to_matchable(),
                            Bracketed::new(vec![
                                Ref::new("ParameterNameSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("UNKNOWN").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::new("EqualsSegment").to_matchable(),
                                        Ref::new("LiteralGrammar").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                AnyNumberOf::new(vec![
                                    Ref::new("CommaSegment").to_matchable(),
                                    Ref::new("ParameterNameSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("UNKNOWN").to_matchable(),
                                        Sequence::new(vec![
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("LiteralGrammar").to_matchable(),
                                        ])
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
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PARAMETERIZATION").to_matchable(),
                        one_of(vec![
                            Ref::keyword("SIMPLE").to_matchable(),
                            Ref::keyword("FORCED").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("RECOMPILE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USE").to_matchable(),
                        Ref::keyword("HINT").to_matchable(),
                        Bracketed::new(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            AnyNumberOf::new(vec![
                                Ref::new("CommaSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USE").to_matchable(),
                        Ref::keyword("PLAN").to_matchable(),
                        Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TABLE").to_matchable(),
                        Ref::keyword("HINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        Delimited::new(vec![Ref::new("TableHintSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);
    dialect.add([(
        "WaitForStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::WaitforStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("WAITFOR").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("DELAY").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TIME").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("TIMEOUT").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "OutputClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::OutputClause, |_| {
            Sequence::new(vec![
                Ref::keyword("OUTPUT").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Delimited::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("DELETED").to_matchable(),
                                    Ref::keyword("INSERTED").to_matchable(),
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("DotSegment").to_matchable(),
                                one_of(vec![
                                    Ref::new("WildcardIdentifierSegment").to_matchable(),
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("ActionParameterSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("AliasExpressionSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![
                        Ref::keyword("INTO").to_matchable(),
                        Ref::keyword("FROM").to_matchable(),
                    ]
                })
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("INTO").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    one_of(vec![
                        Ref::new("ParameterNameSegment").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    add_database_grammars(&mut dialect);

    // Add T-SQL specific statement types to the statement segment
    dialect.replace_grammar(
        "StatementSegment",
        one_of(vec![
            // T-SQL specific statements (BEGIN...END blocks must come first to avoid transaction conflicts)
            Ref::new("BeginEndBlockGrammar").to_matchable(),
            Ref::new("TryBlockSegment").to_matchable(),
            Ref::new("AtomicBlockSegment").to_matchable(),
            Ref::new("DeclareCursorStatementSegment").to_matchable(),
            Ref::new("OpenCursorStatementSegment").to_matchable(),
            Ref::new("FetchCursorStatementSegment").to_matchable(),
            Ref::new("CloseCursorStatementSegment").to_matchable(),
            Ref::new("DeallocateCursorStatementSegment").to_matchable(),
            Ref::new("DeclareStatementGrammar").to_matchable(),
            Ref::new("SetContextInfoSegment").to_matchable(),
            Ref::new("SetLanguageStatementSegment").to_matchable(),
            Ref::new("CreateSecurityPolicySegment").to_matchable(),
            Ref::new("AlterSecurityPolicySegment").to_matchable(),
            Ref::new("DropSecurityPolicySegment").to_matchable(),
            Ref::new("SetVariableStatementSegment").to_matchable(),
            Ref::new("SetLocalVariableStatementSegment").to_matchable(),
            Ref::new("WaitForStatementSegment").to_matchable(),
            Ref::new("ExecuteScriptSegment").to_matchable(),
            Ref::new("PrintStatementGrammar").to_matchable(),
            Ref::new("RaiserrorStatementSegment").to_matchable(),
            Ref::new("ReturnStatementSegment").to_matchable(),
            Ref::new("IfStatementGrammar").to_matchable(),
            Ref::new("WhileStatementGrammar").to_matchable(),
            Ref::new("GotoStatementSegment").to_matchable(),
            Ref::new("LabelSegment").to_matchable(),
            Ref::new("UseStatementGrammar").to_matchable(),
            // Include all ANSI statement types
            Ref::new("SelectableGrammar").to_matchable(),
            Ref::new("MergeStatementSegment").to_matchable(),
            Ref::new("OpenQueryInsertStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("TransactionStatementSegment").to_matchable(),
            Ref::new("DropTableStatementSegment").to_matchable(),
            Ref::new("DropViewStatementSegment").to_matchable(),
            Ref::new("CreateUserStatementSegment").to_matchable(),
            Ref::new("DropUserStatementSegment").to_matchable(),
            Ref::new("TruncateStatementSegment").to_matchable(),
            Ref::new("AccessStatementSegment").to_matchable(),
            Ref::new("CreateTableGraphStatementSegment").to_matchable(),
            Ref::new("CreateTableStatementSegment").to_matchable(),
            Ref::new("CreateRoleStatementSegment").to_matchable(),
            Ref::new("CreateServerRoleStatementSegment").to_matchable(),
            Ref::new("CreateLoginStatementSegment").to_matchable(),
            Ref::new("DropRoleStatementSegment").to_matchable(),
            Ref::new("AlterTableSwitchStatementSegment").to_matchable(),
            Ref::new("AlterTableStatementSegment").to_matchable(),
            Ref::new("CreateSchemaStatementSegment").to_matchable(),
            Ref::new("SetSchemaStatementSegment").to_matchable(),
            Ref::new("DropSchemaStatementSegment").to_matchable(),
            Ref::new("DropTypeStatementSegment").to_matchable(),
            Ref::new("CreateDatabaseStatementSegment").to_matchable(),
            Ref::new("AlterDatabaseStatementSegment").to_matchable(),
            Ref::new("DropDatabaseStatementSegment").to_matchable(),
            Ref::new("CreateIndexStatementSegment").to_matchable(),
            Ref::new("DropIndexStatementSegment").to_matchable(),
            Ref::new("CreateViewStatementSegment").to_matchable(),
            Ref::new("OpenQueryDeleteStatementSegment").to_matchable(),
            Ref::new("DeleteStatementSegment").to_matchable(),
            Ref::new("OpenQueryUpdateStatementSegment").to_matchable(),
            Ref::new("UpdateStatementSegment").to_matchable(),
            Ref::new("CreateFunctionStatementSegment").to_matchable(),
            Ref::new("DropFunctionStatementSegment").to_matchable(),
            Ref::new("CreateProcedureStatementSegment").to_matchable(),
            Ref::new("DropProcedureStatementSegment").to_matchable(),
            Ref::new("CreateSequenceStatementSegment").to_matchable(),
            Ref::new("AlterSequenceStatementSegment").to_matchable(),
            Ref::new("DropSequenceStatementSegment").to_matchable(),
            Ref::new("CreateTriggerStatementSegment").to_matchable(),
            Ref::new("DropTriggerStatementSegment").to_matchable(),
            Ref::new("CreateDatabaseScopedCredentialStatementSegment").to_matchable(),
            Ref::new("CreateExternalDataSourceStatementSegment").to_matchable(),
            Ref::new("SqlcmdCommandSegment").to_matchable(),
            Ref::new("CreateExternalFileFormat").to_matchable(),
            Ref::new("CreateExternalTableStatementSegment").to_matchable(),
            Ref::new("DropExternalTableStatementSegment").to_matchable(),
            Ref::new("CopyIntoTableStatementSegment").to_matchable(),
            Ref::new("CreateFullTextIndexStatementSegment").to_matchable(),
            Ref::new("CreateColumnstoreIndexStatementSegment").to_matchable(),
            Ref::new("ReconfigureStatementSegment").to_matchable(),
            Ref::new("CreatePartitionFunctionSegment").to_matchable(),
            Ref::new("AlterPartitionFunctionSegment").to_matchable(),
            Ref::new("CreatePartitionSchemeSegment").to_matchable(),
            Ref::new("AlterPartitionSchemeSegment").to_matchable(),
            Ref::new("CreateMasterKeySegment").to_matchable(),
            Ref::new("AlterMasterKeySegment").to_matchable(),
            Ref::new("DropMasterKeySegment").to_matchable(),
            Ref::new("OpenSymmetricKeySegment").to_matchable(),
        ])
        .config(|this| this.terminators = vec![Ref::new("DelimiterGrammar").to_matchable()])
        .to_matchable(),
    );

    // T-SQL CREATE TRIGGER uses ON before the event timing and supports
    // CREATE OR ALTER.
    dialect.replace_grammar(
        "CreateTriggerStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateTrigger, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("OR").to_matchable(),
                    Ref::keyword("ALTER").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("TRIGGER").to_matchable(),
                Ref::new("TriggerReferenceSegment").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                one_of(vec![
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ALL").to_matchable(),
                        Ref::keyword("SERVER").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("DATABASE").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    AnyNumberOf::new(vec![
                        one_of(vec![
                            Ref::keyword("ENCRYPTION").to_matchable(),
                            Ref::keyword("NATIVE_COMPILATION").to_matchable(),
                            Ref::keyword("SCHEMABINDING").to_matchable(),
                            Ref::new("ExecuteAsClauseGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                            .config(|this| this.optional())
                            .to_matchable(),
                    ])
                    .to_matchable(),
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
                    Ref::keyword("UPDATE").to_matchable(),
                    Ref::keyword("DELETE").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("APPEND").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("NOT").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::keyword("REPLICATION").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("ProcedureDefinitionGrammar").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // IDENTITY = '...' [, SECRET = '...'] used by credential-related statements.
    dialect.add([(
        "CredentialGrammar".into(),
        Sequence::new(vec![
            Ref::keyword("IDENTITY").to_matchable(),
            Ref::new("EqualsSegment").to_matchable(),
            Ref::new("QuotedLiteralSegment").to_matchable(),
            Sequence::new(vec![
                Ref::new("CommaSegment").to_matchable(),
                Ref::keyword("SECRET").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // Azure Blob Storage / Data Lake Storage Gen2 external locations for COPY INTO.
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/copy-into-transact-sql#external-locations
    dialect.add([
        (
            "AzureBlobStoragePath".into(),
            RegexParser::new(
                concat!(
                    r"'https://[a-z0-9][a-z0-9-]{1,61}[a-z0-9]\.blob\.core\.windows\.net/[a-z0-9]",
                    r"[a-z0-9\.-]{1,61}[a-z0-9](?:/.+)?'",
                ),
                SyntaxKind::ExternalLocation,
            )
            .to_matchable()
            .into(),
        ),
        (
            "AzureDataLakeStorageGen2Path".into(),
            RegexParser::new(
                concat!(
                    r"'https://[a-z0-9][a-z0-9-]{1,61}[a-z0-9]\.dfs\.core\.windows\.net/[a-z0-9]",
                    r"[a-z0-9\.-]{1,61}[a-z0-9](?:/.+)?'",
                ),
                SyntaxKind::ExternalLocation,
            )
            .to_matchable()
            .into(),
        ),
    ]);

    // CREATE DATABASE SCOPED CREDENTIAL statement
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/create-database-scoped-credential-transact-sql
    dialect.add([(
        "CreateDatabaseScopedCredentialStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateDatabaseScopedCredentialStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("DATABASE").to_matchable(),
                Ref::keyword("SCOPED").to_matchable(),
                Ref::keyword("CREDENTIAL").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                Ref::keyword("WITH").to_matchable(),
                Ref::new("CredentialGrammar").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // A quoted literal optionally prefixed with N (e.g. 'foo' or N'foo').
    // sqruff lexes N'foo' as two tokens, so mirror the existing TSQL pattern.
    dialect.add([(
        "QuotedLiteralSegmentOptWithN".into(),
        one_of(vec![
            Ref::new("QuotedLiteralSegment").to_matchable(),
            Sequence::new(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(), // N prefix
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // PERIOD FOR SYSTEM_TIME (start_col, end_col) for temporal tables (#4654)
    dialect.add([(
        "PeriodSegment".into(),
        NodeMatcher::new(SyntaxKind::PeriodSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("PERIOD").to_matchable(),
                Ref::keyword("FOR").to_matchable(),
                Ref::keyword("SYSTEM_TIME").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // sqlcmd commands `:r` and `:setvar` (#4653)
    // https://learn.microsoft.com/en-us/sql/tools/sqlcmd/sqlcmd-utility#sqlcmd-commands
    dialect.add([(
        "SqlcmdOperatorSegment".into(),
        MultiStringParser::new(
            vec!["r".to_string(), "setvar".to_string()],
            SyntaxKind::SqlcmdOperator,
        )
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "SqlcmdFilePathSegment".into(),
        TypedParser::new(
            SyntaxKind::UnquotedRelativeSqlFilePath,
            SyntaxKind::UnquotedRelativeSqlFilePath,
        )
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "SqlcmdCommandSegment".into(),
        NodeMatcher::new(SyntaxKind::SqlcmdCommandSegment, |_| {
            one_of(vec![
                // :r <relative .sql file path>
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::new("ColonSegment").to_matchable(),
                        Ref::new("SqlcmdOperatorSegment").to_matchable(),
                    ])
                    .config(|this| this.disallow_gaps())
                    .to_matchable(),
                    Ref::new("SqlcmdFilePathSegment").to_matchable(),
                ])
                .to_matchable(),
                // :setvar <name> <value>
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::new("ColonSegment").to_matchable(),
                        Ref::new("SqlcmdOperatorSegment").to_matchable(),
                    ])
                    .config(|this| this.disallow_gaps())
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("QuotedIdentifierSegment").to_matchable(),
                        Ref::new("NakedIdentifierSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CREATE EXTERNAL FILE FORMAT (#4647)
    // Restricted value sets for compression codecs, encodings and SerDe methods.
    dialect.add([(
        "FileCompressionSegment".into(),
        MultiStringParser::new(
            vec![
                "'org.apache.hadoop.io.compress.GzipCodec'".to_string(),
                "'org.apache.hadoop.io.compress.DefaultCodec'".to_string(),
                "'org.apache.hadoop.io.compress.SnappyCodec'".to_string(),
            ],
            SyntaxKind::FileCompression,
        )
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "FileEncodingSegment".into(),
        MultiStringParser::new(
            vec!["'UTF8'".to_string(), "'UTF16'".to_string()],
            SyntaxKind::FileEncoding,
        )
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "SerdeMethodSegment".into(),
        MultiStringParser::new(
            vec![
                "'org.apache.hadoop.hive.serde2.columnar.LazyBinaryColumnarSerDe'".to_string(),
                "'org.apache.hadoop.hive.serde2.columnar.ColumnarSerDe'".to_string(),
            ],
            SyntaxKind::SerdeMethod,
        )
        .to_matchable()
        .into(),
    )]);

    // FORMAT_OPTIONS (...) entries for the delimited-text file format.
    dialect.add([(
        "ExternalFileFormatDelimitedTextFormatOptionClause".into(),
        NodeMatcher::new(
            SyntaxKind::ExternalFileDelimitedTextFormatOptionsClause,
            |_| {
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("FIELD_TERMINATOR").to_matchable(),
                            Ref::keyword("STRING_DELIMITER").to_matchable(),
                            Ref::keyword("DATE_FORMAT").to_matchable(),
                            Ref::keyword("PARSER_VERSION").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FIRST_ROW").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USE_TYPE_DEFAULT").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("BooleanLiteralGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ENCODING").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("FileEncodingSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            },
        )
        .to_matchable()
        .into(),
    )]);

    // Per-format clauses inside `CREATE EXTERNAL FILE FORMAT ... WITH ( ... )`.
    dialect.add([(
        "ExternalFileFormatDelimitedTextClause".into(),
        NodeMatcher::new(SyntaxKind::ExternalFileDelimitedTextClause, |_| {
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::keyword("FORMAT_TYPE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::keyword("DELIMITEDTEXT").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("FORMAT_OPTIONS").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Ref::new("ExternalFileFormatDelimitedTextFormatOptionClause")
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DATA_COMPRESSION").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("FileCompressionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "ExternalFileFormatRcClause".into(),
        NodeMatcher::new(SyntaxKind::ExternalFileRcClause, |_| {
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::keyword("FORMAT_TYPE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::keyword("RCFILE").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SERDE_METHOD").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("SerdeMethodSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DATA_COMPRESSION").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("FileCompressionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "ExternalFileFormatOrcClause".into(),
        NodeMatcher::new(SyntaxKind::ExternalFileOrcClause, |_| {
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::keyword("FORMAT_TYPE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::keyword("ORC").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DATA_COMPRESSION").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("FileCompressionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "ExternalFileFormatParquetClause".into(),
        NodeMatcher::new(SyntaxKind::ExternalFileParquetClause, |_| {
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::keyword("FORMAT_TYPE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::keyword("PARQUET").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DATA_COMPRESSION").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("FileCompressionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "ExternalFileFormatJsonClause".into(),
        NodeMatcher::new(SyntaxKind::ExternalFileJsonClause, |_| {
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::keyword("FORMAT_TYPE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::keyword("JSON").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DATA_COMPRESSION").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("FileCompressionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "ExternalFileFormatDeltaClause".into(),
        NodeMatcher::new(SyntaxKind::ExternalFileDeltaClause, |_| {
            Sequence::new(vec![
                Ref::keyword("FORMAT_TYPE").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::keyword("DELTA").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    dialect.add([(
        "CreateExternalFileFormat".into(),
        NodeMatcher::new(SyntaxKind::CreateExternalFileFormat, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("EXTERNAL").to_matchable(),
                Ref::keyword("FILE").to_matchable(),
                Ref::keyword("FORMAT").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                Ref::keyword("WITH").to_matchable(),
                Bracketed::new(vec![
                    one_of(vec![
                        Ref::new("ExternalFileFormatDelimitedTextClause").to_matchable(),
                        Ref::new("ExternalFileFormatRcClause").to_matchable(),
                        Ref::new("ExternalFileFormatOrcClause").to_matchable(),
                        Ref::new("ExternalFileFormatParquetClause").to_matchable(),
                        Ref::new("ExternalFileFormatJsonClause").to_matchable(),
                        Ref::new("ExternalFileFormatDeltaClause").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // OPENJSON() table-valued function (#4652)
    // https://learn.microsoft.com/en-us/sql/t-sql/functions/openjson-transact-sql
    dialect.add([(
        "OpenJsonWithClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::OpenjsonWithClause, |_| {
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                            // column_path
                            Ref::new("QuotedLiteralSegment").optional().to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("AS").to_matchable(),
                                Ref::keyword("JSON").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
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
    )]);
    dialect.add([(
        "OpenJsonSegment".into(),
        NodeMatcher::new(SyntaxKind::OpenjsonSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("OPENJSON").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::new("ParameterNameSegment").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("OpenJsonWithClauseSegment")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // OPENQUERY() table-valued function (#6640)
    // https://learn.microsoft.com/en-us/sql/t-sql/functions/openquery-transact-sql
    dialect.add([(
        "OpenQuerySegment".into(),
        NodeMatcher::new(SyntaxKind::OpenquerySegment, |_| {
            Sequence::new(vec![
                Ref::keyword("OPENQUERY").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([
        (
            "OpenQueryInsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("INSERT").to_matchable(),
                    Ref::new("OpenQuerySegment").to_matchable(),
                    Ref::new("PostTableExpressionGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
                    one_of(vec![
                        Ref::new("SelectableGrammar").to_matchable(),
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
            "OpenQueryDeleteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DELETE").to_matchable(),
                    Ref::new("OpenQuerySegment").to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "OpenQueryUpdateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UpdateStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("UPDATE").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("OpenQuerySegment").to_matchable(),
                    Ref::new("PostTableExpressionGrammar")
                        .optional()
                        .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::new("SetClauseListSegment").to_matchable(),
                    Ref::new("FromClauseSegment").optional().to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // OPENROWSET() rowset provider and bulk data source (#6584)
    // https://learn.microsoft.com/en-us/sql/t-sql/functions/openrowset-transact-sql
    dialect.add([(
        "OpenRowSetSegment".into(),
        NodeMatcher::new(SyntaxKind::OpenrowsetSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("OPENROWSET").to_matchable(),
                Bracketed::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("CommaSegment").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                    Ref::new("DelimiterGrammar").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                    Ref::new("DelimiterGrammar").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("CommaSegment").to_matchable(),
                            one_of(vec![
                                Ref::new("TableReferenceSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("BULK").to_matchable(),
                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                            Ref::new("CommaSegment").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Sequence::new(vec![
                                        Ref::keyword("FORMATFILE").to_matchable(),
                                        Ref::new("EqualsSegment").to_matchable(),
                                        Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                                        Ref::new("CommaSegment").to_matchable(),
                                    ])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                                    Delimited::new(vec![
                                        Sequence::new(vec![
                                            Ref::keyword("DATASOURCE").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("ERRORFILE").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("ERRORFILE_DATA_SOURCE").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("MAXERRORS").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("NumericLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("FIRSTROW").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("NumericLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("LASTROW").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("NumericLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("CODEPAGE").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("FORMAT").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("FIELDQUOTE").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("FORMATFILE").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("FORMATFILE_DATA_SOURCE").to_matchable(),
                                            Ref::new("EqualsSegment").to_matchable(),
                                            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("SINGLE_BLOB").to_matchable(),
                                Ref::keyword("SINGLE_CLOB").to_matchable(),
                                Ref::keyword("SINGLE_NCLOB").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("OpenRowSetWithClauseSegment")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // WITH clause of an OPENROWSET() segment.
    // https://learn.microsoft.com/en-us/azure/synapse-analytics/sql/develop-openrowset#syntax
    dialect.replace_grammar(
        "CollateGrammar",
        Sequence::new(vec![
            Ref::keyword("COLLATE").to_matchable(),
            Ref::new("CollationReferenceSegment").to_matchable(),
        ])
        .to_matchable(),
    );
    dialect.add([(
        "OpenRowSetWithClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::OpenrowsetWithClause, |_| {
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                            Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                                .config(|this| this.optional())
                                .to_matchable(),
                            Ref::new("CollateGrammar").optional().to_matchable(),
                            one_of(vec![
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
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
    )]);

    // CREATE EXTERNAL TABLE (#4642)
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/create-external-table-transact-sql
    dialect.add([(
        "CreateExternalTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateExternalTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("EXTERNAL").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnDefinitionSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("WITH").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("TableLocationClause").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("DATA_SOURCE").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FILE_FORMAT").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("REJECT_TYPE").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("VALUE").to_matchable(),
                                Ref::keyword("PERCENTAGE").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("REJECT_VALUE").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("REJECT_SAMPLE_VALUE").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("REJECTED_ROW_LOCATION").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
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
    )]);

    // CREATE FULLTEXT INDEX (#5274)
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/create-fulltext-index-transact-sql
    dialect.add([(
        "CreateFullTextIndexStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateFulltextIndexStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("FULLTEXT").to_matchable(),
                Ref::keyword("INDEX").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            AnyNumberOf::new(vec![
                                Sequence::new(vec![
                                    Ref::keyword("TYPE").to_matchable(),
                                    Ref::keyword("COLUMN").to_matchable(),
                                    Ref::new("DatatypeSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("LANGUAGE").to_matchable(),
                                    one_of(vec![
                                        Ref::new("NumericLiteralSegment").to_matchable(),
                                        Ref::new("QuotedLiteralSegment").to_matchable(),
                                    ])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("STATISTICAL_SEMANTICS").to_matchable(),
                            ])
                            .config(|this| this.max_times_per_element = Some(1))
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("KEY").to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    // catalog / filegroup option
                    Sequence::new(vec![
                        Ref::keyword("ON").to_matchable(),
                        Delimited::new(vec![
                            AnyNumberOf::new(vec![
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("FILEGROUP").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.max_times_per_element = Some(1))
                            .to_matchable(),
                        ])
                        .config(|this| this.allow_trailing())
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                // WITH option
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("CHANGE_TRACKING").to_matchable(),
                                Ref::new("EqualsSegment").optional().to_matchable(),
                                one_of(vec![
                                    Ref::keyword("MANUAL").to_matchable(),
                                    Ref::keyword("AUTO").to_matchable(),
                                    Delimited::new(vec![
                                        Ref::keyword("OFF").to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("NO").to_matchable(),
                                            Ref::keyword("POPULATION").to_matchable(),
                                        ])
                                        .config(|this| this.optional())
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("STOPLIST").to_matchable(),
                                Ref::new("EqualsSegment").optional().to_matchable(),
                                one_of(vec![
                                    Ref::keyword("OFF").to_matchable(),
                                    Ref::keyword("SYSTEM").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("SEARCH").to_matchable(),
                                Ref::keyword("PROPERTY").to_matchable(),
                                Ref::keyword("LIST").to_matchable(),
                                Ref::new("EqualsSegment").optional().to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // DROP EXTERNAL TABLE (#4919)
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/drop-external-table-transact-sql
    dialect.add([(
        "DropExternalTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropExternalTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("EXTERNAL").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // A T-SQL external storage location (used as a table expression by COPY INTO).
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/copy-into-transact-sql#external-locations
    dialect.add([(
        "StorageLocationSegment".into(),
        NodeMatcher::new(SyntaxKind::StorageLocation, |_| {
            one_of(vec![
                Ref::new("AzureBlobStoragePath").to_matchable(),
                Ref::new("AzureDataLakeStorageGen2Path").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // COPY INTO <table> statement
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/copy-into-transact-sql
    dialect.add([(
        "CopyIntoTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CopyIntoTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("COPY").to_matchable(),
                Ref::keyword("INTO").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnDefinitionSegment").to_matchable()])
                        .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("FromClauseSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("FILE_TYPE").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("FILE_FORMAT").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CREDENTIAL").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Bracketed::new(vec![Ref::new("CredentialGrammar").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ERRORFILE").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ERRORFILE_CREDENTIAL").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Bracketed::new(vec![Ref::new("CredentialGrammar").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("MAXERRORS").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("COMPRESSION").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("FIELDQUOTE").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("FIELDTERMINATOR").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ROWTERMINATOR").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("FIRSTROW").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("DATEFORMAT").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ENCODING").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("FileEncodingSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("IDENTITY_INSERT").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("AUTO_CREATE_TABLE").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // `LOCATION = ...` clause, used by external tables and external data sources.
    dialect.add([(
        "TableLocationClause".into(),
        NodeMatcher::new(SyntaxKind::TableLocationClause, |_| {
            Sequence::new(vec![
                Ref::keyword("LOCATION").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("USER_DB").to_matchable(), // Azure Synapse Analytics specific
                    Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CREATE EXTERNAL DATA SOURCE statement
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/create-external-data-source-transact-sql
    dialect.add([(
        "CreateExternalDataSourceStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateExternalDataSourceStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("EXTERNAL").to_matchable(),
                Ref::keyword("DATA").to_matchable(),
                Ref::keyword("SOURCE").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                Ref::keyword("WITH").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("TableLocationClause").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CONNECTION_OPTIONS").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            AnyNumberOf::new(vec![
                                Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CREDENTIAL").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("PUSHDOWN").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ON").to_matchable(),
                                Ref::keyword("OFF").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
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
    )]);

    // USE statement for changing database context
    dialect.add([
        (
            "UseStatementSegment".into(),
            Ref::new("UseStatementGrammar").to_matchable().into(),
        ),
        (
            "UseStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("USE").to_matchable(),
                Ref::new("DatabaseReferenceSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Add variable reference support for T-SQL @ and @@ variables
    dialect.add([
        (
            "LeadingDotSegment".into(),
            StringParser::new(".", SyntaxKind::LeadingDot)
                .to_matchable()
                .into(),
        ),
        (
            "TsqlVariableSegment".into(),
            TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable)
                .to_matchable()
                .into(),
        ),
        (
            "ParameterizedSegment".into(),
            NodeMatcher::new(SyntaxKind::ParameterizedExpression, |_| {
                Ref::new("TsqlVariableSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TsqlTableVariableSegment".into(),
            NodeMatcher::new(SyntaxKind::TableReference, |_| {
                Ref::new("TsqlVariableSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Update TableReferenceSegment to support T-SQL table variables.
    // Temp tables are handled by ObjectReferenceSegment via HashIdentifierSegment.
    dialect.replace_grammar(
        "TableReferenceSegment",
        one_of(vec![
            Ref::new("ObjectReferenceSegment").to_matchable(),
            Sequence::new(vec![
                Ref::new("LeadingDotSegment").to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::new("SingleIdentifierGrammar")
                            .optional()
                            .to_matchable(),
                        Ref::new("DotSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.min_times(0);
                    this.max_times(2);
                })
                .to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("TsqlVariableSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL table expressions; PIVOT/UNPIVOT are handled as join-like clauses.
    dialect.replace_grammar(
        "TableExpressionSegment",
        one_of(vec![
            Ref::new("ValuesClauseSegment").to_matchable(),
            Sequence::new(vec![
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("PostTableExpressionGrammar").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("BareFunctionSegment").to_matchable(),
            Ref::new("OpenRowSetSegment").to_matchable(),
            Ref::new("OpenJsonSegment").to_matchable(),
            Ref::new("OpenQuerySegment").to_matchable(),
            Ref::new("FunctionSegment").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("StorageLocationSegment").to_matchable(),
            Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()]).to_matchable(),
            Bracketed::new(vec![Ref::new("MergeStatementSegment").to_matchable()]).to_matchable(),
            Bracketed::new(vec![Ref::new("DeleteStatementSegment").to_matchable()]).to_matchable(),
            Bracketed::new(vec![Ref::new("InsertStatementSegment").to_matchable()]).to_matchable(),
            Bracketed::new(vec![Ref::new("UpdateStatementSegment").to_matchable()]).to_matchable(),
        ])
        .to_matchable(),
    );

    // Table hints support - Examples: SELECT * FROM Users WITH (NOLOCK) and
    // SELECT * FROM Users (NOLOCK)
    dialect.add([
        (
            "TableHintSegment".into(),
            Sequence::new(vec![
                Ref::keyword("WITH").optional().to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("TableHintElement").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TableHintElement".into(),
            one_of(vec![
                // Simple hints (just keywords)
                Ref::keyword("NOLOCK").to_matchable(),
                Ref::keyword("READUNCOMMITTED").to_matchable(),
                Ref::keyword("READCOMMITTED").to_matchable(),
                Ref::keyword("REPEATABLEREAD").to_matchable(),
                Ref::keyword("SERIALIZABLE").to_matchable(),
                Ref::keyword("READPAST").to_matchable(),
                Ref::keyword("PAGLOCK").to_matchable(),
                Ref::keyword("ROWLOCK").to_matchable(),
                Ref::keyword("TABLOCK").to_matchable(),
                Ref::keyword("TABLOCKX").to_matchable(),
                Ref::keyword("UPDLOCK").to_matchable(),
                Ref::keyword("XLOCK").to_matchable(),
                Ref::keyword("NOEXPAND").to_matchable(),
                Ref::keyword("FORCESEEK").to_matchable(),
                Ref::keyword("FORCESCAN").to_matchable(),
                Ref::keyword("HOLDLOCK").to_matchable(),
                Ref::keyword("SNAPSHOT").to_matchable(),
                // INDEX hint with parameter
                Sequence::new(vec![
                    Ref::keyword("INDEX").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("IndexReferenceSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // T-SQL MERGE supports table hints on the target. Excluding USING from the
    // optional alias prevents a target without an alias from consuming it.
    dialect.add([(
        "MergeStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::MergeStatement, |_| {
            Sequence::new(vec![
                Ref::new("MergeIntoLiteralGrammar").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("TableHintSegment").optional().to_matchable(),
                Ref::new("AliasExpressionSegment")
                    .exclude(Ref::keyword("USING"))
                    .optional()
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
    )]);

    // Define PostTableExpressionGrammar to include T-SQL table hints
    dialect.add([(
        "PostTableExpressionGrammar".into(),
        Ref::new("TableHintSegment")
            .optional()
            .to_matchable()
            .into(),
    )]);

    dialect.replace_grammar(
        "TemporalQuerySegment",
        NodeMatcher::new(SyntaxKind::TemporalQuery, |_| {
            Sequence::new(vec![
                Ref::keyword("FOR").to_matchable(),
                Ref::keyword("SYSTEM_TIME").to_matchable(),
                one_of(vec![
                    Ref::keyword("ALL").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::keyword("OF").to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FROM").to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("BETWEEN").to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("AND").to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("ParameterNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CONTAINED").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("QuotedLiteralSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Override FromExpressionElementSegment to ensure table hints are parsed correctly
    // The LookaheadExclude prevents WITH from being parsed as an alias when followed by (
    dialect.replace_grammar(
        "FromExpressionElementSegment",
        Sequence::new(vec![
            Ref::new("PreTableFunctionKeywordsGrammar")
                .optional()
                .to_matchable(),
            optionally_bracketed(vec![Ref::new("TableExpressionSegment").to_matchable()])
                .to_matchable(),
            Ref::new("TemporalQuerySegment").optional().to_matchable(),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec![
                    Ref::new("FromClauseTerminatorGrammar").to_matchable(),
                    Ref::new("SamplingExpressionSegment").to_matchable(),
                    Ref::new("JoinLikeClauseGrammar").to_matchable(),
                    Ref::new("JoinClauseSegment").to_matchable(),
                    LookaheadExclude::new("WITH", "(").to_matchable(), // Prevents WITH from being parsed as alias when followed by (
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
                .to_matchable(), // T-SQL table hints
        ])
        .to_matchable(),
    );

    // Update JoinClauseSegment to handle APPLY syntax properly
    dialect.replace_grammar(
        "JoinClauseSegment",
        one_of(vec![
            // Standard JOIN syntax
            Sequence::new(vec![
                Ref::new("ConditionalJoinKeywordsGrammar")
                    .optional()
                    .to_matchable(),
                Ref::new("JoinKeywordsGrammar").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("JoinTargetGrammar").to_matchable(),
                AnyNumberOf::new(vec![Ref::new("NestedJoinGrammar").to_matchable()]).to_matchable(),
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
            // NATURAL JOIN
            Sequence::new(vec![
                Ref::new("UnconditionalJoinKeywordsGrammar").to_matchable(),
                Ref::new("JoinKeywordsGrammar").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("FromExpressionElementSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable(),
            // T-SQL APPLY syntax
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("CROSS").to_matchable(),
                    Ref::keyword("OUTER").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("APPLY").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("FromExpressionElementSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL specific data type handling for MAX keyword and -1
    // Override BracketedArguments to accept MAX keyword and negative numbers
    dialect.replace_grammar(
        "BracketedArguments",
        Bracketed::new(vec![
            Delimited::new(vec![
                one_of(vec![
                    Ref::new("LiteralGrammar").to_matchable(),
                    Ref::keyword("MAX").to_matchable(),
                    // Support negative numbers like -1 for NVARCHAR(-1)
                    Sequence::new(vec![
                        Ref::new("SignedSegmentGrammar").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.optional();
            })
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL data types have type-specific argument placement. In particular,
    // the VARYING keyword precedes bracketed arguments in forms such as
    // `CHAR VARYING(100)`, so the generic ANSI datatype grammar is not broad
    // enough for the complete T-SQL type family.
    dialect.replace_grammar(
        "DatatypeSegment",
        Sequence::new(vec![
            // Optional schema qualification, including bracketed schemas.
            Sequence::new(vec![
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::new("DotSegment").to_matchable(),
            ])
            .config(|this| {
                this.disallow_gaps();
                this.optional();
            })
            .to_matchable(),
            one_of(vec![
                // Square-bracketed data type identifiers such as [sysname].
                Sequence::new(vec![
                    TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::DataTypeIdentifier)
                        .to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                // Exact numeric types without parameters.
                one_of(vec![
                    Ref::keyword("TINYINT").to_matchable(),
                    Ref::keyword("SMALLINT").to_matchable(),
                    Ref::keyword("INT").to_matchable(),
                    Ref::keyword("BIGINT").to_matchable(),
                    Ref::keyword("BIT").to_matchable(),
                    Ref::keyword("MONEY").to_matchable(),
                    Ref::keyword("SMALLMONEY").to_matchable(),
                ])
                .to_matchable(),
                // Exact numeric types with optional precision and scale.
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("DECIMAL").to_matchable(),
                        Ref::keyword("NUMERIC").to_matchable(),
                        Ref::keyword("DEC").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                // Approximate numeric types.
                Sequence::new(vec![
                    Ref::keyword("FLOAT").to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("REAL").to_matchable(),
                // Date and time types.
                one_of(vec![
                    Ref::keyword("DATE").to_matchable(),
                    Ref::keyword("SMALLDATETIME").to_matchable(),
                    Ref::keyword("DATETIME").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("TIME").to_matchable(),
                        Ref::keyword("DATETIME2").to_matchable(),
                        Ref::keyword("DATETIMEOFFSET").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                // Character string types.
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("CHAR").to_matchable(),
                        Ref::keyword("CHARACTER").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("VARYING").optional().to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("VARCHAR").to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("TEXT").to_matchable(),
                // Unicode character string types.
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("NCHAR").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NATIONAL").to_matchable(),
                            one_of(vec![
                                Ref::keyword("CHAR").to_matchable(),
                                Ref::keyword("CHARACTER").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("VARYING").optional().to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("NVARCHAR").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NATIONAL").to_matchable(),
                            Ref::keyword("CHARACTER").to_matchable(),
                            Ref::keyword("VARYING").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("NTEXT").to_matchable(),
                // Binary string types.
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("BINARY").to_matchable(),
                        Ref::keyword("VARBINARY").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("IMAGE").to_matchable(),
                // Other, spatial, and vector types.
                one_of(vec![
                    Ref::keyword("CURSOR").to_matchable(),
                    Ref::keyword("SQL_VARIANT").to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::keyword("TIMESTAMP").to_matchable(),
                    Ref::keyword("ROWVERSION").to_matchable(),
                    Ref::keyword("UNIQUEIDENTIFIER").to_matchable(),
                    Ref::keyword("XML").to_matchable(),
                    Ref::keyword("JSON").to_matchable(),
                    Ref::keyword("GEOGRAPHY").to_matchable(),
                    Ref::keyword("GEOMETRY").to_matchable(),
                    Ref::keyword("HIERARCHYID").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("VECTOR").to_matchable(),
                    Ref::new("BracketedArguments").optional().to_matchable(),
                ])
                .to_matchable(),
                // User-defined data types.
                Ref::new("DatatypeIdentifierSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("CharCharacterSetGrammar")
                .optional()
                .to_matchable(),
        ])
        .to_matchable(),
    );

    // APPLY clause support (CROSS APPLY and OUTER APPLY)
    // APPLY invokes a table-valued function for each row of the outer table
    // CROSS APPLY: Like INNER JOIN - returns only rows with results
    // OUTER APPLY: Like LEFT JOIN - returns all rows, NULLs when no results
    dialect.add([(
        "ApplyClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::JoinClause,
            |_| // APPLY is classified as a join type
            Sequence::new(vec![
                one_of(vec![Ref::keyword("CROSS").to_matchable(), Ref::keyword("OUTER").to_matchable()]).to_matchable(),
                Ref::keyword("APPLY").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("FromExpressionElementSegment").to_matchable(), // The function or subquery
                MetaSegment::dedent().to_matchable()
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // APPLY and PIVOT/UNPIVOT can follow an aliased table expression.
    dialect.add([(
        "JoinLikeClauseGrammar".into(),
        one_of(vec![
            Ref::new("ApplyClauseSegment").to_matchable(),
            any_set_of(vec![
                Ref::new("PivotUnpivotStatementSegment").to_matchable(),
            ])
            .config(|this| this.min_times(1))
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // WITHIN GROUP support for ordered set aggregate functions
    dialect.add([(
        "WithinGroupClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::WithingroupClause, |_| {
            Sequence::new(vec![
                Ref::keyword("WITHIN").to_matchable(),
                Ref::keyword("GROUP").to_matchable(),
                Bracketed::new(vec![
                    Ref::new("OrderByClauseSegment").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override PostFunctionGrammar to include WITHIN GROUP
    dialect.add([(
        "PostFunctionGrammar".into(),
        AnyNumberOf::new(vec![
            Ref::new("WithinGroupClauseSegment").to_matchable(),
            Ref::new("OverClauseSegment").to_matchable(),
            Ref::new("FilterClauseGrammar").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // Add T-SQL IDENTITY constraint support
    dialect.add([(
        "IdentityConstraintGrammar".into(),
        Sequence::new(vec![
            Ref::keyword("IDENTITY").to_matchable(),
            Bracketed::new(vec![
                Ref::new("NumericLiteralSegment").to_matchable(), // seed
                Ref::new("CommaSegment").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(), // increment
            ])
            .config(|this| this.optional())
            .to_matchable(), // IDENTITY() can be empty
        ])
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "PrimaryKeyGrammar",
        Sequence::new(vec![
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("PRIMARY").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("UNIQUE").to_matchable(),
            ])
            .to_matchable(),
            one_of(vec![
                Ref::keyword("CLUSTERED").to_matchable(),
                Ref::keyword("NONCLUSTERED").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // Extend ColumnConstraintSegment to include T-SQL specific constraints
    dialect.add([(
        "ColumnConstraintSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_| {
            one_of(vec![
                // A primary or foreign key may have a column list and options.
                // Keep this before the shorter column-constraint alternative so
                // the Rust matcher does not stop after PRIMARY KEY CLUSTERED.
                Ref::new("TableConstraintSegment").to_matchable(),
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONSTRAINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        // NOT NULL / NULL
                        Sequence::new(vec![
                            Ref::keyword("NOT").optional().to_matchable(),
                            Ref::keyword("NULL").to_matchable(),
                        ])
                        .to_matchable(),
                        // CHECK constraint
                        Sequence::new(vec![
                            Ref::keyword("CHECK").to_matchable(),
                            Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        // DEFAULT constraint
                        Sequence::new(vec![
                            Ref::keyword("DEFAULT").to_matchable(),
                            optionally_bracketed(vec![
                                one_of(vec![
                                    optionally_bracketed(vec![
                                        Ref::new("LiteralGrammar").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::new("BareFunctionSegment").to_matchable(),
                                    Ref::new("FunctionSegment").to_matchable(),
                                    Ref::new("NextValueSequenceSegment").to_matchable(),
                                    Ref::new("HexadecimalLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Primary key without a column list.
                        Ref::new("PrimaryKeyGrammar").to_matchable(),
                        Ref::new("IdentityConstraintGrammar").to_matchable(), // T-SQL IDENTITY
                        Ref::new("AutoIncrementGrammar").to_matchable(), // Keep ANSI AUTO_INCREMENT
                        // Foreign key without a column list.
                        Ref::new("ForeignKeyGrammar").to_matchable(),
                        Ref::new("ReferencesConstraintGrammar").to_matchable(),
                        Ref::new("CommentClauseSegment").to_matchable(),
                        // COLLATE
                        Sequence::new(vec![
                            Ref::keyword("COLLATE").to_matchable(),
                            Ref::new("CollationReferenceSegment").to_matchable(),
                        ])
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
    )]);

    dialect.add([(
        "BracketedIndexColumnListGrammar".into(),
        NodeMatcher::new(SyntaxKind::BracketedIndexColumnListGrammar, |_| {
            Bracketed::new(vec![
                Delimited::new(vec![
                    Ref::new("IndexColumnDefinitionSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ComputedColumnDefinitionSegment".into(),
        NodeMatcher::new(SyntaxKind::ComputedColumnDefinition, |_| {
            Sequence::new(vec![
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                optionally_bracketed(vec![
                    one_of(vec![
                        Ref::new("FunctionSegment").to_matchable(),
                        Ref::new("BareFunctionSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("PERSISTED").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                AnyNumberOf::new(vec![Ref::new("ColumnConstraintSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ReferencesConstraintGrammar".into(),
        NodeMatcher::new(SyntaxKind::ReferencesConstraintGrammar, |_| {
            Sequence::new(vec![
                Ref::keyword("REFERENCES").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("BracketedColumnReferenceListGrammar")
                    .optional()
                    .to_matchable(),
                any_set_of(vec![
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
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("REPLICATION").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "TableConstraintSegment",
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
                    Ref::new("BracketedIndexColumnListGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("ForeignKeyGrammar").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                    Ref::new("ReferencesConstraintGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    let alter_table_options = dialect.grammar("AlterTableOptionsGrammar").copy(
        Some(vec![
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("CONSTRAINT").to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("CHECK").to_matchable(),
                Ref::keyword("CONSTRAINT").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::keyword("DROP").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("PeriodSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("COLUMN").to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ADD").to_matchable(),
                Delimited::new(vec![
                    one_of(vec![
                        Ref::new("ComputedColumnDefinitionSegment").to_matchable(),
                        Ref::new("ColumnDefinitionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ADD").to_matchable(),
                Ref::new("ColumnConstraintSegment").to_matchable(),
                Ref::keyword("FOR").to_matchable(),
                Ref::new("ColumnReferenceSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("CHECK").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("ADD").to_matchable(),
                Ref::new("TableConstraintSegment").to_matchable(),
            ])
            .to_matchable(),
        ]),
        Some(0),
        None,
        None,
        Vec::new(),
        false,
    );
    dialect.replace_grammar("AlterTableOptionsGrammar", alter_table_options);

    // T-SQL permits DEFAULT as a function argument expression.
    let expression_d_without_brackets =
        dialect.grammar("Expression_D_Potential_Select_Statement_Without_Brackets");
    dialect.replace_grammar(
        "Expression_D_Potential_Select_Statement_Without_Brackets",
        expression_d_without_brackets.copy(
            Some(vec![Ref::keyword("DEFAULT").to_matchable()]),
            Some(0),
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    // T-SQL literal handling distinguishes integers and binary literals while
    // retaining variables as literals.
    dialect.replace_grammar(
        "NumericLiteralSegment",
        one_of(vec![
            RegexParser::new(r"[0-9]+", SyntaxKind::NumericLiteral).to_matchable(),
            TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral).to_matchable(),
        ])
        .to_matchable(),
    );
    dialect.add([(
        "LiteralGrammar".into(),
        one_of(vec![
            Ref::new("QuotedLiteralSegment").to_matchable(),
            Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
            Ref::new("IntegerLiteralSegment").to_matchable(),
            Ref::new("BinaryLiteralSegment").to_matchable(),
            Ref::new("NumericLiteralSegment").to_matchable(),
            Ref::new("BooleanLiteralGrammar").to_matchable(),
            Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
            Ref::new("NullLiteralSegment").to_matchable(),
            Ref::new("DateTimeLiteralGrammar").to_matchable(),
            Ref::new("TypedArrayLiteralSegment").to_matchable(),
            Ref::new("ParameterizedSegment").to_matchable(), // Add T-SQL variables
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL supports CREATE VIEW, ALTER VIEW, and CREATE OR ALTER VIEW.
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/alter-view-transact-sql
    dialect.add([(
        "CreateViewStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateViewStatement, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("ALTER").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CREATE").to_matchable(),
                        Ref::keyword("OR").to_matchable(),
                        Ref::keyword("ALTER").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("VIEW").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("BracketedColumnReferenceListGrammar")
                    .optional()
                    .to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("SelectableGrammar").to_matchable(),
                Ref::new("WithNoSchemaBindingClauseSegment")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL CREATE FUNCTION support
    dialect.replace_grammar(
        "FunctionParameterGrammar",
        Sequence::new(vec![
            Ref::new("ParameterNameSegment").optional().to_matchable(),
            Ref::keyword("AS").optional().to_matchable(),
            Ref::new("DatatypeSegment").to_matchable(),
            Ref::keyword("NULL").optional().to_matchable(),
            Sequence::new(vec![
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "FunctionParameterListGrammar",
        Bracketed::new(vec![
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::new("FunctionParameterGrammar").to_matchable(),
                    Ref::keyword("READONLY").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "CreateFunctionStatementSegment",
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("ALTER").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("OR").to_matchable(),
                    Ref::keyword("ALTER").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("FUNCTION").to_matchable(),
            Ref::new("ObjectReferenceSegment").to_matchable(),
            Ref::new("FunctionParameterListGrammar").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("RETURNS").to_matchable(),
                one_of(vec![
                    Ref::new("DatatypeSegment").to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("ParameterNameSegment").to_matchable(),
                        Ref::keyword("TABLE").to_matchable(),
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
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::new("FunctionOptionSegment").optional().to_matchable(),
            Ref::keyword("AS").optional().to_matchable(),
            Ref::new("ProcedureDefinitionGrammar").to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.add([(
        "FunctionOptionSegment".into(),
        NodeMatcher::new(SyntaxKind::FunctionOptionSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Delimited::new(vec![
                    AnyNumberOf::new(vec![
                        Ref::keyword("ENCRYPTION").to_matchable(),
                        Ref::keyword("SCHEMABINDING").to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("RETURNS").to_matchable(),
                                    Ref::keyword("NULL").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("CALLED").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("NULL").to_matchable(),
                            Ref::keyword("INPUT").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ExecuteAsClauseGrammar").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INLINE").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ON").to_matchable(),
                                Ref::keyword("OFF").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.min_times(1))
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL CREATE PROCEDURE support
    dialect.add([
        (
            "CreateProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateProcedureStatement, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("CREATE").to_matchable(),
                        Ref::keyword("ALTER").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CREATE").to_matchable(),
                            Ref::keyword("OR").to_matchable(),
                            Ref::keyword("ALTER").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("PROC").to_matchable(),
                        Ref::keyword("PROCEDURE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    // Optional version number
                    Sequence::new(vec![
                        Ref::new("SemicolonSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    // Optional parameter list
                    Ref::new("ProcedureParameterListGrammar")
                        .optional()
                        .to_matchable(),
                    // Procedure options
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Delimited::new(vec![
                            Ref::keyword("ENCRYPTION").to_matchable(),
                            Ref::keyword("RECOMPILE").to_matchable(),
                            Ref::keyword("NATIVE_COMPILATION").to_matchable(),
                            Ref::keyword("SCHEMABINDING").to_matchable(),
                            Ref::new("ExecuteAsClauseGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("REPLICATION").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("ProcedureDefinitionGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropProcedureStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    one_of(vec![
                        Ref::keyword("PROC").to_matchable(),
                        Ref::keyword("PROCEDURE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Delimited::new(vec![Ref::new("ObjectReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureParameterListGrammar".into(),
            one_of(vec![
                // Bracketed parameter list: (param1, param2, param3)
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ProcedureParameterGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable(),
                // Unbracketed parameter list: param1, param2, param3
                Delimited::new(vec![Ref::new("ProcedureParameterGrammar").to_matchable()])
                    .config(|this| this.optional())
                    .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureParameterGrammar".into(),
            Sequence::new(vec![
                Ref::new("ParameterNameSegment").to_matchable(),
                Ref::new("DatatypeSegment").to_matchable(),
                // Optional VARYING keyword (for cursors and some special types)
                Ref::keyword("VARYING").optional().to_matchable(),
                // Optional NULL/NOT NULL
                Sequence::new(vec![
                    Ref::keyword("NOT").optional().to_matchable(),
                    Ref::keyword("NULL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                // Optional default value
                Sequence::new(vec![
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("LiteralGrammar").to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                        // Function calls as defaults (e.g., NEWID())
                        Ref::new("FunctionSegment").to_matchable(),
                        // String literal with prefix (e.g., N'foo')
                        Sequence::new(vec![
                            Ref::new("NakedIdentifierSegment").to_matchable(), // N, B, X etc.
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                // Optional parameter modifiers (can appear in any order)
                AnyNumberOf::new(vec![
                    one_of(vec![
                        Ref::keyword("OUT").to_matchable(),
                        Ref::keyword("OUTPUT").to_matchable(),
                        Ref::keyword("READONLY").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ParameterNameSegment".into(),
            Ref::new("TsqlVariableSegment").to_matchable().into(),
        ),
        (
            "ExecuteAsClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("EXECUTE").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                one_of(vec![
                    Ref::keyword("CALLER").to_matchable(),
                    Ref::keyword("SELF").to_matchable(),
                    Ref::keyword("OWNER").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(), // user name
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureDefinitionGrammar".into(),
            NodeMatcher::new(SyntaxKind::ProcedureStatement, |_| {
                one_of(vec![
                    Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                    Ref::new("AtomicBlockSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("EXTERNAL").to_matchable(),
                        Ref::keyword("NAME").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AtomicBlockSegment".into(),
            Sequence::new(vec![
                Ref::keyword("BEGIN").to_matchable(),
                Ref::keyword("ATOMIC").to_matchable(),
                Ref::keyword("WITH").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("AtomicBlockOptionGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                MetaSegment::indent().to_matchable(),
                AnyNumberOf::new(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AtomicBlockOptionGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("LANGUAGE").to_matchable(),
                    Ref::keyword("DATEFIRST").to_matchable(),
                    Ref::keyword("DATEFORMAT").to_matchable(),
                    Ref::keyword("DELAYED_DURABILITY").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TRANSACTION").to_matchable(),
                        Ref::keyword("ISOLATION").to_matchable(),
                        Ref::keyword("LEVEL").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    // N'string' syntax for Unicode strings
                    Sequence::new(vec![
                        Ref::new("NakedIdentifierSegment").to_matchable(), // N prefix
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // Special handling for multi-word isolation levels
                    Sequence::new(vec![
                        Ref::keyword("REPEATABLE").to_matchable(),
                        Ref::keyword("READ").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("SERIALIZABLE").to_matchable(),
                    Ref::keyword("SNAPSHOT").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("OFF").to_matchable(),
                    // Date format values
                    Ref::keyword("MDY").to_matchable(),
                    Ref::keyword("DMY").to_matchable(),
                    Ref::keyword("YMD").to_matchable(),
                    Ref::keyword("YDM").to_matchable(),
                    Ref::keyword("MYD").to_matchable(),
                    Ref::keyword("DYM").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([(
        "SelectVariableAssignmentSegment".into(),
        NodeMatcher::new(SyntaxKind::SelectVariableAssignment, |_| {
            Sequence::new(vec![
                Ref::new("ParameterNameSegment").to_matchable(),
                Ref::new("AssignmentOperatorSegment").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "AltAliasExpressionSegment".into(),
        NodeMatcher::new(SyntaxKind::AliasExpression, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::new("QuotedIdentifierSegment").to_matchable(),
                    Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("EqualAliasOperatorSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL supports alternative alias syntax: AliasName = Expression
    // The parser distinguishes between column references (table1.column1)
    // and alias assignments (AliasName = table1.column1)
    dialect.replace_grammar(
        "SelectClauseElementSegment",
        one_of(vec![
            // T-SQL alias equals pattern: AliasName = Expression
            Sequence::new(vec![
                Ref::new("AltAliasExpressionSegment").to_matchable(),
                one_of(vec![
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::new("BaseExpressionElementGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // Wildcard expressions
            Ref::new("WildcardExpressionSegment").to_matchable(),
            // SELECT @variable = expression (variable assignment)
            Ref::new("SelectVariableAssignmentSegment").to_matchable(),
            // Everything else
            Sequence::new(vec![
                Ref::new("BaseExpressionElementGrammar").to_matchable(),
                Ref::new("AliasExpressionSegment").optional().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // Graph tables and their constraint/index options.
    dialect.add([
        (
            "ConnectionConstraintGrammar".into(),
            NodeMatcher::new(SyntaxKind::ConnectionConstraintGrammar, |_| {
                Sequence::new(vec![
                    Ref::keyword("CONNECTION").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("TableReferenceSegment").to_matchable(),
                                Ref::keyword("TO").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
                            ])
                            .config(|this| {
                                this.optional();
                            })
                            .to_matchable(),
                        ])
                        .config(|this| {
                            this.allow_trailing();
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    any_set_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("DELETE").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("NO").to_matchable(),
                                    Ref::keyword("ACTION").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("CASCADE").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("UPDATE").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("NO").to_matchable(),
                                    Ref::keyword("ACTION").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("CASCADE").to_matchable(),
                            ])
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
            "GraphTableConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::GraphTableConstraint, |_| {
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONSTRAINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("PrimaryKeyGrammar").to_matchable(),
                            Ref::new("BracketedIndexColumnListGrammar").to_matchable(),
                            Ref::new("RelationalIndexOptionsSegment")
                                .optional()
                                .to_matchable(),
                            Ref::new("OnPartitionOrFilegroupOptionSegment")
                                .optional()
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("ForeignKeyGrammar").to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                            Ref::new("ReferencesConstraintGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ConnectionConstraintGrammar")
                            .optional()
                            .to_matchable(),
                        Ref::new("CheckConstraintGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateTableGraphStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableGraphStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Ref::new("GraphTableConstraintSegment").to_matchable(),
                            Ref::new("ComputedColumnDefinitionSegment").to_matchable(),
                            Ref::new("ColumnDefinitionSegment").to_matchable(),
                            Ref::new("TableIndexSegment").to_matchable(),
                            Ref::new("PeriodSegment").to_matchable(),
                        ])
                        .config(|this| {
                            this.allow_trailing();
                        })
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        one_of(vec![
                            Ref::keyword("NODE").to_matchable(),
                            Ref::keyword("EDGE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("OnPartitionOrFilegroupOptionSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CheckConstraintGrammar".into(),
            NodeMatcher::new(SyntaxKind::CheckConstraintGrammar, |_| {
                Sequence::new(vec![
                    Ref::keyword("CHECK").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("REPLICATION").to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RelationalIndexOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::RelationalIndexOptions, |_| {
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    optionally_bracketed(vec![
                        Delimited::new(vec![
                            AnyNumberOf::new(vec![
                                Sequence::new(vec![
                                    one_of(vec![
                                        Ref::keyword("PAD_INDEX").to_matchable(),
                                        Ref::keyword("FILLFACTOR").to_matchable(),
                                        Ref::keyword("SORT_IN_TEMPDB").to_matchable(),
                                        Ref::keyword("IGNORE_DUP_KEY").to_matchable(),
                                        Ref::keyword("STATISTICS_NORECOMPUTE").to_matchable(),
                                        Ref::keyword("STATISTICS_INCREMENTAL").to_matchable(),
                                        Ref::keyword("DROP_EXISTING").to_matchable(),
                                        Ref::keyword("RESUMABLE").to_matchable(),
                                        Ref::keyword("ALLOW_ROW_LOCKS").to_matchable(),
                                        Ref::keyword("ALLOW_PAGE_LOCKS").to_matchable(),
                                        Ref::keyword("OPTIMIZE_FOR_SEQUENTIAL_KEY").to_matchable(),
                                        Ref::keyword("MAXDOP").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("ON").to_matchable(),
                                        Ref::keyword("OFF").to_matchable(),
                                        Ref::new("LiteralGrammar").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("MaxDurationSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ONLINE").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("OFF").to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("ON").to_matchable(),
                                            Bracketed::new(vec![
                                                Sequence::new(vec![
                                                    Ref::keyword("WAIT_AT_LOW_PRIORITY")
                                                        .to_matchable(),
                                                    Bracketed::new(vec![
                                                        Delimited::new(vec![
                                                            Ref::new("MaxDurationSegment")
                                                                .to_matchable(),
                                                            Sequence::new(vec![
                                                                Ref::keyword("ABORT_AFTER_WAIT")
                                                                    .to_matchable(),
                                                                Ref::new("EqualsSegment")
                                                                    .to_matchable(),
                                                                one_of(vec![
                                                                    Ref::keyword("NONE")
                                                                        .to_matchable(),
                                                                    Ref::keyword("SELF")
                                                                        .to_matchable(),
                                                                    Ref::keyword("BLOCKERS")
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
                                                .to_matchable(),
                                            ])
                                            .config(|this| {
                                                this.optional();
                                            })
                                            .to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("COMPRESSION_DELAY").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    Ref::new("NumericLiteralSegment").to_matchable(),
                                    Sequence::new(vec![Ref::keyword("MINUTES").to_matchable()])
                                        .config(|this| {
                                            this.optional();
                                        })
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("DATA_COMPRESSION").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("NONE").to_matchable(),
                                        Ref::keyword("ROW").to_matchable(),
                                        Ref::keyword("PAGE").to_matchable(),
                                        Ref::keyword("COLUMNSTORE").to_matchable(),
                                        Ref::keyword("COLUMNSTORE_ARCHIVE").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::new("OnPartitionsSegment").optional().to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| {
                                this.min_times(1);
                            })
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
            "OnPartitionOrFilegroupOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::OnPartitionOrFilegroupStatement, |_| {
                one_of(vec![
                    Ref::new("PartitionSchemeClause").to_matchable(),
                    Ref::new("FilegroupClause").to_matchable(),
                    Ref::new("LiteralGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableIndexSegment".into(),
            NodeMatcher::new(SyntaxKind::TableIndexSegment, |_| {
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("INDEX").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Sequence::new(vec![Ref::keyword("UNIQUE").to_matchable()])
                                .config(|this| {
                                    this.optional();
                                })
                                .to_matchable(),
                            one_of(vec![
                                Ref::keyword("CLUSTERED").to_matchable(),
                                Ref::keyword("NONCLUSTERED").to_matchable(),
                            ])
                            .config(|this| {
                                this.optional();
                            })
                            .to_matchable(),
                            Ref::new("BracketedIndexColumnListGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CLUSTERED").to_matchable(),
                            Ref::keyword("COLUMNSTORE").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Sequence::new(vec![Ref::keyword("NONCLUSTERED").to_matchable()])
                                .config(|this| {
                                    this.optional();
                                })
                                .to_matchable(),
                            Ref::keyword("COLUMNSTORE").to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("RelationalIndexOptionsSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("OnPartitionOrFilegroupOptionSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("FilestreamOnOptionSegment")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MaxDurationSegment".into(),
            NodeMatcher::new(SyntaxKind::MaxDuration, |_| {
                Sequence::new(vec![
                    Ref::keyword("MAX_DURATION").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Sequence::new(vec![Ref::keyword("MINUTES").to_matchable()])
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
            "OnPartitionsSegment".into(),
            NodeMatcher::new(SyntaxKind::OnPartitionsClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("PARTITIONS").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("NumericLiteralSegment").to_matchable(),
                                    Ref::keyword("TO").to_matchable(),
                                    Ref::new("NumericLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
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
            "PartitionSchemeClause".into(),
            NodeMatcher::new(SyntaxKind::PartitionSchemeClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::new("PartitionSchemeNameSegment").to_matchable(),
                    Bracketed::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FilegroupClause".into(),
            NodeMatcher::new(SyntaxKind::FilegroupClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::new("FilegroupNameSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FilestreamOnOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::FilestreamOnOptionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("FILESTREAM_ON").to_matchable(),
                    one_of(vec![
                        Ref::new("FilegroupNameSegment").to_matchable(),
                        Ref::new("PartitionSchemeNameSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("NULL").to_matchable(),
                            Ref::new("LiteralGrammar").to_matchable(),
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
            "PartitionSchemeNameSegment".into(),
            NodeMatcher::new(SyntaxKind::PartitionSchemeName, |_| {
                Ref::new("SingleIdentifierGrammar").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FilegroupNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FilegroupName, |_| {
                Ref::new("SingleIdentifierGrammar").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // T-SQL CREATE TABLE with Azure Synapse Analytics support
    dialect.replace_grammar(
        "CreateTableStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                one_of(vec![
                    // Regular CREATE TABLE with column definitions
                    Sequence::new(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("TableConstraintSegment").to_matchable(),
                                    Ref::new("ComputedColumnDefinitionSegment").to_matchable(),
                                    Ref::new("ColumnDefinitionSegment").to_matchable(),
                                    Ref::new("TableIndexSegment").to_matchable(),
                                    Ref::new("PeriodSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.allow_trailing())
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        // Azure Synapse table options
                        Sequence::new(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![Ref::new("TableOptionGrammar").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // CREATE TABLE AS SELECT with optional WITH clause before AS
                    Sequence::new(vec![
                        // Azure Synapse table options (required for CTAS)
                        Sequence::new(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![Ref::new("TableOptionGrammar").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                        optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // T-SQL CREATE USER statement.
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/create-user-transact-sql
    let allow_encrypted_value = Sequence::new(vec![
        Ref::keyword("ALLOW_ENCRYPTED_VALUE_MODIFICATIONS").to_matchable(),
        Ref::new("EqualsSegment").to_matchable(),
        one_of(vec![
            Ref::keyword("ON").to_matchable(),
            Ref::keyword("OFF").to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable();
    let default_schema = Sequence::new(vec![
        Ref::keyword("DEFAULT_SCHEMA").to_matchable(),
        Ref::new("EqualsSegment").to_matchable(),
        Ref::new("ObjectReferenceSegment").to_matchable(),
    ])
    .to_matchable();
    let default_language = Sequence::new(vec![
        Ref::keyword("DEFAULT_LANGUAGE").to_matchable(),
        Ref::new("EqualsSegment").to_matchable(),
        Ref::new("ObjectReferenceSegment").to_matchable(),
    ])
    .to_matchable();
    let limited_option_list = Sequence::new(vec![
        Ref::keyword("WITH").to_matchable(),
        Delimited::new(vec![
            default_schema.clone(),
            default_language.clone(),
            allow_encrypted_value.clone(),
        ])
        .to_matchable(),
    ])
    .config(|this| this.optional())
    .to_matchable();
    let options_list = Delimited::new(vec![
        default_schema,
        default_language,
        Sequence::new(vec![
            Ref::keyword("SID").to_matchable(),
            Ref::new("EqualsSegment").to_matchable(),
            Ref::new("HexadecimalLiteralSegment").to_matchable(),
        ])
        .to_matchable(),
        allow_encrypted_value,
        Sequence::new(vec![
            Ref::keyword("PASSWORD").to_matchable(),
            Ref::new("EqualsSegment").to_matchable(),
            Ref::new("QuotedLiteralSegment").to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable();
    let external_provider = Sequence::new(vec![
        Ref::keyword("FROM").to_matchable(),
        Ref::keyword("EXTERNAL").to_matchable(),
        Ref::keyword("PROVIDER").to_matchable(),
        Sequence::new(vec![
            Ref::keyword("WITH").to_matchable(),
            Ref::keyword("OBJECT_ID").to_matchable(),
            Ref::new("EqualsSegment").to_matchable(),
            Ref::new("QuotedLiteralSegment").to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
    ])
    .to_matchable();
    dialect.replace_grammar(
        "CreateUserStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::keyword("USER").to_matchable(),
            Ref::new("RoleReferenceSegment").to_matchable(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![Ref::keyword("WITH").to_matchable(), options_list])
                    .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FROM").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("LOGIN").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    limited_option_list.clone(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FROM").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("CERTIFICATE").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ASYMMETRIC").to_matchable(),
                            Ref::keyword("KEY").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITHOUT").to_matchable(),
                    Ref::keyword("LOGIN").to_matchable(),
                    limited_option_list,
                ])
                .to_matchable(),
                external_provider,
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL CREATE ROLE with optional AUTHORIZATION clause
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/create-role-transact-sql
    dialect.replace_grammar(
        "CreateRoleStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateRoleStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("ROLE").to_matchable(),
                Ref::new("RoleReferenceSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AUTHORIZATION").to_matchable(),
                    Ref::new("RoleReferenceSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // T-SQL CREATE SERVER ROLE with optional AUTHORIZATION clause
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/create-server-role-transact-sql
    dialect.add([(
        "CreateServerRoleStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateServerRoleStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("SERVER").to_matchable(),
                Ref::keyword("ROLE").to_matchable(),
                Ref::new("RoleReferenceSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AUTHORIZATION").to_matchable(),
                    Ref::new("RoleReferenceSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL CREATE LOGIN statement.
    // https://learn.microsoft.com/en-us/sql/t-sql/statements/create-login-transact-sql
    dialect.add([(
        "CreateLoginStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateLoginStatement, |_| {
            let default_database = Sequence::new(vec![
                Ref::keyword("DEFAULT_DATABASE").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable();
            let default_language = Sequence::new(vec![
                Ref::keyword("DEFAULT_LANGUAGE").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable();
            let secondary_option = one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("SID").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("HexadecimalLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                default_database,
                default_language,
                Sequence::new(vec![
                    Ref::keyword("CHECK_EXPIRATION").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ON").to_matchable(),
                        Ref::keyword("OFF").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CHECK_POLICY").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ON").to_matchable(),
                        Ref::keyword("OFF").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CREDENTIAL").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable();
            let password_options = Sequence::new(vec![
                Ref::keyword("PASSWORD").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
                Ref::keyword("MUST_CHANGE").optional().to_matchable(),
                Ref::new("CommaSegment").optional().to_matchable(),
                Delimited::new(vec![secondary_option])
                    .config(|this| this.optional())
                    .to_matchable(),
            ])
            .to_matchable();
            let sources = one_of(vec![
                Ref::keyword("WINDOWS").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("EXTERNAL").to_matchable(),
                    Ref::keyword("PROVIDER").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CERTIFICATE").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ASYMMETRIC").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable();

            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("LOGIN").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![Ref::keyword("FROM").to_matchable(), sources])
                        .to_matchable(),
                    Sequence::new(vec![Ref::keyword("WITH").to_matchable(), password_options])
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "TableOptionGrammar".into(),
        one_of(vec![
            // Azure Synapse distribution options
            Sequence::new(vec![
                Ref::keyword("DISTRIBUTION").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("ROUND_ROBIN").to_matchable(),
                    Ref::keyword("REPLICATE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("HASH").to_matchable(),
                        Bracketed::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // Azure Synapse index options
            one_of(vec![
                Ref::keyword("HEAP").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CLUSTERED").to_matchable(),
                    Ref::keyword("COLUMNSTORE").to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CLUSTERED").to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("ASC").to_matchable(),
                                    Ref::keyword("DESC").to_matchable(),
                                ])
                                .config(|this| this.optional())
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
            .to_matchable(),
            // Other table options
            Sequence::new(vec![
                Ref::keyword("SYSTEM_VERSIONING").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("HISTORY_TABLE").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("HISTORY_RETENTION_PERIOD").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("INFINITE").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::new("NumericLiteralSegment").optional().to_matchable(),
                                        one_of(vec![
                                            Ref::keyword("DAYS").to_matchable(),
                                            Ref::keyword("WEEKS").to_matchable(),
                                            Ref::keyword("MONTHS").to_matchable(),
                                            Ref::keyword("YEARS").to_matchable(),
                                        ])
                                        .config(|this| this.optional())
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::new("CommaSegment").to_matchable(),
                                Ref::keyword("DATA_CONSISTENCY_CHECK").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("ON").to_matchable(),
                                    Ref::keyword("OFF").to_matchable(),
                                ])
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
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("PARTITION").to_matchable(),
                Bracketed::new(vec![
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::keyword("RANGE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("LEFT").to_matchable(),
                        Ref::keyword("RIGHT").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::keyword("VALUES").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL uses + for both arithmetic and string concatenation
    dialect.add([(
        "StringBinaryOperatorGrammar".into(),
        one_of(vec![
            Ref::new("ConcatSegment").to_matchable(), // Standard || operator
            Ref::new("PlusSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL specific data type identifier - allows case-insensitive user-defined types
    dialect.add([(
        "DatatypeIdentifierSegment".into(),
        SegmentGenerator::new(|dialect| {
            // Future-reserved words remain valid data type identifiers in T-SQL.
            let reserved_keywords = dialect.sets("reserved_keywords");
            let pattern = reserved_keywords.iter().join("|");
            let anti_template = format!("^({pattern})$");

            one_of(vec![
                // Case-insensitive pattern for T-SQL data type identifiers (including UDTs)
                RegexParser::new("[A-Za-z_][A-Za-z0-9_]*", SyntaxKind::DataTypeIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable(),
                Ref::new("SingleIdentifierGrammar")
                    .exclude(Ref::new("NakedIdentifierSegment"))
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .into(),
    )]);

    dialect.add([
        (
            "CreatePartitionFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::CreatePartitionFunctionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::keyword("FUNCTION").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()]).to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::keyword("RANGE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("LEFT").to_matchable(),
                        Ref::keyword("RIGHT").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::keyword("VALUES").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("LiteralGrammar").to_matchable()])
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
            "AlterPartitionFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterPartitionFunctionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::keyword("FUNCTION").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Bracketed::new(vec![]).to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("SPLIT").to_matchable(),
                            Ref::keyword("RANGE").to_matchable(),
                            Bracketed::new(vec![Ref::new("LiteralGrammar").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("MERGE").to_matchable(),
                            Ref::keyword("RANGE").to_matchable(),
                            Bracketed::new(vec![Ref::new("LiteralGrammar").to_matchable()])
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
            "CreatePartitionSchemeSegment".into(),
            NodeMatcher::new(SyntaxKind::CreatePartitionSchemeStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::keyword("SCHEME").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::keyword("ALL").optional().to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                                Ref::keyword("PRIMARY").to_matchable(),
                            ])
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
            "AlterPartitionSchemeSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterPartitionSchemeStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::keyword("SCHEME").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::keyword("NEXT").to_matchable(),
                    Ref::keyword("USED").to_matchable(),
                    Ref::new("ObjectReferenceSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([
        (
            "CreateMasterKeySegment".into(),
            NodeMatcher::new(SyntaxKind::CreateMasterKeyStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("MASTER").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ENCRYPTION").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::keyword("PASSWORD").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
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
            "MasterKeyEncryptionSegment".into(),
            NodeMatcher::new(SyntaxKind::MasterKeyEncryptionOption, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("SERVICE").to_matchable(),
                        Ref::keyword("MASTER").to_matchable(),
                        Ref::keyword("KEY").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PASSWORD").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
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
            "AlterMasterKeySegment".into(),
            NodeMatcher::new(SyntaxKind::AlterMasterKeyStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("MASTER").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("FORCE").optional().to_matchable(),
                            Ref::keyword("REGENERATE").to_matchable(),
                            Ref::keyword("WITH").to_matchable(),
                            Ref::keyword("ENCRYPTION").to_matchable(),
                            Ref::keyword("BY").to_matchable(),
                            Ref::new("MasterKeyEncryptionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("ADD").to_matchable(),
                                Ref::keyword("DROP").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("ENCRYPTION").to_matchable(),
                            Ref::keyword("BY").to_matchable(),
                            Ref::new("MasterKeyEncryptionSegment").to_matchable(),
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
            "DropMasterKeySegment".into(),
            NodeMatcher::new(SyntaxKind::DropMasterKeyStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("MASTER").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateSecurityPolicySegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSecurityPolicyStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("SECURITY").to_matchable(),
                    Ref::keyword("POLICY").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("ADD").to_matchable(),
                            one_of(vec![
                                Ref::keyword("FILTER").to_matchable(),
                                Ref::keyword("BLOCK").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::keyword("PREDICATE").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("ON").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("AFTER").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("INSERT").to_matchable(),
                                        Ref::keyword("UPDATE").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("BEFORE").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("UPDATE").to_matchable(),
                                        Ref::keyword("DELETE").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::keyword("STATE").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("ON").to_matchable(),
                                        Ref::keyword("OFF").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("SCHEMABINDING").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("ON").to_matchable(),
                                        Ref::keyword("OFF").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("REPLICATION").to_matchable(),
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
            "AlterSecurityPolicySegment".into(),
            NodeMatcher::new(SyntaxKind::AlterSecurityPolicyStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("SECURITY").to_matchable(),
                    Ref::keyword("POLICY").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("ADD").to_matchable(),
                                Ref::keyword("ALTER").to_matchable(),
                            ])
                            .to_matchable(),
                            one_of(vec![
                                Ref::keyword("FILTER").to_matchable(),
                                Ref::keyword("BLOCK").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::keyword("PREDICATE").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("ON").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("AFTER").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("INSERT").to_matchable(),
                                        Ref::keyword("UPDATE").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("BEFORE").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("UPDATE").to_matchable(),
                                        Ref::keyword("DELETE").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("DROP").to_matchable(),
                            one_of(vec![
                                Ref::keyword("FILTER").to_matchable(),
                                Ref::keyword("BLOCK").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::keyword("PREDICATE").to_matchable(),
                            Ref::keyword("ON").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::keyword("STATE").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("ON").to_matchable(),
                                        Ref::keyword("OFF").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("SCHEMABINDING").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("ON").to_matchable(),
                                        Ref::keyword("OFF").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("REPLICATION").to_matchable(),
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
            "DropSecurityPolicySegment".into(),
            NodeMatcher::new(SyntaxKind::DropSecurityPolicy, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("SECURITY").to_matchable(),
                    Ref::keyword("POLICY").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("IF").to_matchable(),
                        Ref::keyword("EXISTS").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "OpenSymmetricKeySegment".into(),
            NodeMatcher::new(SyntaxKind::OpenSymmetricKeyStatement, |_| {
                let with_password = Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("PASSWORD").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable();

                let decryption_mechanism = one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CERTIFICATE").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        with_password.clone(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ASYMMETRIC").to_matchable(),
                        Ref::keyword("KEY").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        with_password,
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("SYMMETRIC").to_matchable(),
                        Ref::keyword("KEY").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PASSWORD").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ]);

                Sequence::new(vec![
                    Ref::keyword("OPEN").to_matchable(),
                    Ref::keyword("SYMMETRIC").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::keyword("DECRYPTION").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    decryption_mechanism.to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NextValueSequenceSegment".into(),
            NodeMatcher::new(SyntaxKind::SequenceNextValue, |_| {
                Sequence::new(vec![
                    Ref::keyword("NEXT").to_matchable(),
                    Ref::keyword("VALUE").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::Expression, |_| {
                one_of(vec![
                    Ref::new("Expression_A_Grammar").to_matchable(),
                    Ref::new("NextValueSequenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // expand() must be called after all grammar modifications

    dialect
}

fn add_database_grammars(dialect: &mut Dialect) {
    dialect.replace_grammar(
        "CollationReferenceSegment",
        one_of(vec![
            Ref::new("QuotedLiteralSegment").to_matchable(),
            Ref::new("NakedIdentifierSegment").to_matchable(),
            Ref::keyword("DATABASE_DEFAULT").to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "LogicalFileNameSegment".into(),
            NodeMatcher::new(SyntaxKind::LogicalFileName, |_| {
                Sequence::new(vec![
                    Ref::keyword("NAME").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("NakedIdentifierSegment").to_matchable(),
                        Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FileSpecFileNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecFileName, |_| {
                Sequence::new(vec![
                    Ref::new("CommaSegment").optional().to_matchable(),
                    Ref::keyword("FILENAME").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FileSpecNewNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecNewName, |_| {
                Sequence::new(vec![
                    Ref::new("CommaSegment").to_matchable(),
                    Ref::keyword("NEWNAME").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegmentOptWithN").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FileSpecSizeSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecSize, |_| {
                Sequence::new(vec![
                    Ref::new("CommaSegment").to_matchable(),
                    Ref::keyword("SIZE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("SizeLiteralSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("KB").to_matchable(),
                                Ref::keyword("MB").to_matchable(),
                                Ref::keyword("GB").to_matchable(),
                                Ref::keyword("TB").to_matchable(),
                            ])
                            .config(|this| this.optional())
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
            "FileSpecMaxSizeSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecMaxSize, |_| {
                Sequence::new(vec![
                    Ref::new("CommaSegment").optional().to_matchable(),
                    Ref::keyword("MAXSIZE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("SizeLiteralSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("KB").to_matchable(),
                                Ref::keyword("MB").to_matchable(),
                                Ref::keyword("GB").to_matchable(),
                                Ref::keyword("TB").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("UNLIMITED").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FileSpecFileGrowthSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecFileGrowth, |_| {
                Sequence::new(vec![
                    Ref::new("CommaSegment").to_matchable(),
                    Ref::keyword("FILEGROWTH").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("SizeLiteralSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("KB").to_matchable(),
                                Ref::keyword("MB").to_matchable(),
                                Ref::keyword("GB").to_matchable(),
                                Ref::keyword("TB").to_matchable(),
                                Ref::new("PercentSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
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
            "UnbracketedFileSpecSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecWithoutBracket, |_| {
                Sequence::new(vec![
                    Ref::new("LogicalFileNameSegment").optional().to_matchable(),
                    Ref::new("FileSpecFileNameSegment").to_matchable(),
                    Ref::new("FileSpecSizeSegment").optional().to_matchable(),
                    Ref::new("FileSpecMaxSizeSegment").optional().to_matchable(),
                    Ref::new("FileSpecFileGrowthSegment")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FileSpecSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpec, |_| {
                Bracketed::new(vec![Ref::new("UnbracketedFileSpecSegment").to_matchable()])
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FileSpecSegmentInAlterDatabase".into(),
            NodeMatcher::new(SyntaxKind::FileSpec, |_| {
                Bracketed::new(vec![
                    Sequence::new(vec![
                        Ref::new("LogicalFileNameSegment").optional().to_matchable(),
                        Ref::new("FileSpecNewNameSegment").optional().to_matchable(),
                        Ref::new("FileSpecFileNameSegment")
                            .optional()
                            .to_matchable(),
                        Ref::new("FileSpecSizeSegment").optional().to_matchable(),
                        Ref::new("FileSpecMaxSizeSegment").optional().to_matchable(),
                        Ref::new("FileSpecFileGrowthSegment")
                            .optional()
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
            "CompatibilityLevelSegment".into(),
            NodeMatcher::new(SyntaxKind::CompatibilityLevel, |_| {
                Sequence::new(vec![
                    Ref::keyword("COMPATIBILITY_LEVEL").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AutoOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::AutoOption, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("AUTO_CLOSE").to_matchable(),
                            Ref::keyword("AUTO_SHRINK").to_matchable(),
                            Ref::keyword("AUTO_UPDATE_STATISTICS").to_matchable(),
                            Ref::keyword("AUTO_UPDATE_STATISTICS_ASYNC").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("OFF").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AUTO_CREATE_STATISTICS").to_matchable(),
                        one_of(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("OFF").to_matchable(),
                            Bracketed::new(vec![
                                Sequence::new(vec![
                                    Ref::keyword("INCREMENTAL").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("ON").to_matchable(),
                                        Ref::keyword("OFF").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                            ])
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
            "ServiceObjectiveSegment".into(),
            NodeMatcher::new(SyntaxKind::ServiceObjective, |_| {
                Sequence::new(vec![
                    Ref::keyword("SERVICE_OBJECTIVE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ELASTIC_POOL").to_matchable(),
                            Bracketed::new(vec![
                                Ref::keyword("NAME").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("NakedOrQuotedIdentifierGrammar").to_matchable(),
                            ])
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
            "EditionSegment".into(),
            NodeMatcher::new(SyntaxKind::Edition, |_| {
                Sequence::new(vec![
                    Ref::keyword("EDITION").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AllowConnectionsSegment".into(),
            NodeMatcher::new(SyntaxKind::AllowConnections, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALLOW_CONNECTIONS").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ALL").to_matchable(),
                        Ref::keyword("NO").to_matchable(),
                        Ref::keyword("READ_ONLY").to_matchable(),
                        Ref::keyword("READ_WRITE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BackupStorageRedundancySegment".into(),
            NodeMatcher::new(SyntaxKind::BackupStorageRedundancy, |_| {
                Sequence::new(vec![
                    Ref::keyword("BACKUP_STORAGE_REDUNDANCY").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar("CreateDatabaseStatementSegment", {
        let file_group = Sequence::new(vec![
            Ref::keyword("FILEGROUP").to_matchable(),
            Ref::new("NakedOrQuotedIdentifierGrammar").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONTAINS").to_matchable(),
                        Ref::keyword("FILESTREAM").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("DEFAULT").optional().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CONTAINS").to_matchable(),
                    Ref::keyword("MEMORY_OPTIMIZED_DATA").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Delimited::new(vec![Ref::new("FileSpecSegment").to_matchable()]).to_matchable(),
        ])
        .to_matchable();

        let filestream_option = one_of(vec![
            Sequence::new(vec![
                Ref::keyword("NON_TRANSACTED_ACCESS").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("OFF").to_matchable(),
                    Ref::keyword("READ_ONLY").to_matchable(),
                    Ref::keyword("FULL").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("DIRECTORY_NAME").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable();

        let language_option = |keyword| {
            Sequence::new(vec![
                Ref::keyword(keyword).to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        };
        let on_off_option = |keyword| {
            Sequence::new(vec![
                Ref::keyword(keyword).to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("OFF").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        };
        let create_database_option = one_of(vec![
            Sequence::new(vec![
                Ref::keyword("FILESTREAM").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![filestream_option])
                        .config(|this| this.min_delimiters = 1)
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            language_option("DEFAULT_FULLTEXT_LANGUAGE"),
            language_option("DEFAULT_LANGUAGE"),
            on_off_option("NESTED_TRIGGERS"),
            on_off_option("TRANSFORM_NOISE_WORDS"),
            Sequence::new(vec![
                Ref::keyword("TWO_DIGIT_YEAR_CUTOFF").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("DB_CHAINING").to_matchable(),
                one_of(vec![
                    Ref::keyword("OFF").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("TRUSTWORTHY").to_matchable(),
                one_of(vec![
                    Ref::keyword("OFF").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("PERSISTENT_LOG_BUFFER").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                Bracketed::new(vec![
                    Ref::keyword("DIRECTORY_NAME").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("LEDGER").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("OFF").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("CATALOG_COLLATION").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("CollationReferenceSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable();

        let normal = Sequence::new(vec![
            Sequence::new(vec![
                Ref::keyword("CONTAINMENT").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("NONE").to_matchable(),
                    Ref::keyword("PARTIAL").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("PRIMARY").optional().to_matchable(),
                Delimited::new(vec![Ref::new("FileSpecSegment").to_matchable()]).to_matchable(),
                Sequence::new(vec![
                    Ref::new("CommaSegment").to_matchable(),
                    Delimited::new(vec![file_group])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LOG").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Delimited::new(vec![Ref::new("FileSpecSegment").to_matchable()]).to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("COLLATE").to_matchable(),
                Ref::new("CollationReferenceSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Delimited::new(vec![create_database_option]).to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable();

        let attach_option = one_of(vec![
            Ref::keyword("ENABLE_BROKER").to_matchable(),
            Ref::keyword("NEW_BROKER").to_matchable(),
            Ref::keyword("ERROR_BROKER_CONVERSATIONS").to_matchable(),
            Ref::keyword("RESTRICTED_USER").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("FILESTREAM").to_matchable(),
                Bracketed::new(vec![
                    Ref::keyword("DIRECTORY_NAME").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable();
        let attach = Sequence::new(vec![
            Ref::keyword("ON").to_matchable(),
            Delimited::new(vec![Ref::new("FileSpecSegment").to_matchable()]).to_matchable(),
            Ref::keyword("FOR").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("ATTACH").to_matchable(),
                    Sequence::new(vec![Ref::keyword("WITH").to_matchable(), attach_option])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("ATTACH_REBUILD_LOG").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable();
        let snapshot = Sequence::new(vec![
            Ref::keyword("ON").to_matchable(),
            Delimited::new(vec![
                Bracketed::new(vec![
                    Ref::new("LogicalFileNameSegment").optional().to_matchable(),
                    Ref::new("FileSpecFileNameSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.min_delimiters = 1)
            .to_matchable(),
            Ref::keyword("AS").to_matchable(),
            Ref::keyword("SNAPSHOT").to_matchable(),
            Ref::keyword("OF").to_matchable(),
            Ref::new("NakedIdentifierSegment").to_matchable(),
        ])
        .to_matchable();

        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::keyword("DATABASE").to_matchable(),
            Ref::new("DatabaseReferenceSegment").to_matchable(),
            one_of(vec![normal, attach, snapshot])
                .config(|this| this.optional())
                .to_matchable(),
        ])
        .to_matchable()
    });

    dialect.add([(
        "AlterDatabaseStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterDatabaseStatement, |_| {
            let modify_name = Sequence::new(vec![
                Ref::keyword("MODIFY").to_matchable(),
                Ref::keyword("NAME").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("DatabaseReferenceSegment").to_matchable(),
            ])
            .to_matchable();
            let add_or_modify_files = one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::keyword("FILE").to_matchable(),
                    Ref::new("FileSpecSegmentInAlterDatabase").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TO").to_matchable(),
                        Ref::keyword("FILEGROUP").to_matchable(),
                        Ref::new("NakedOrQuotedIdentifierGrammar")
                            .optional()
                            .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::keyword("LOG").to_matchable(),
                    Ref::keyword("FILE").to_matchable(),
                    Delimited::new(vec![
                        Ref::new("FileSpecSegmentInAlterDatabase").to_matchable(),
                    ])
                    .config(|this| this.min_delimiters = 1)
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("REMOVE").to_matchable(),
                    Ref::keyword("FILE").to_matchable(),
                    Ref::new("LiteralGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("MODIFY").to_matchable(),
                    Ref::keyword("FILE").to_matchable(),
                    Ref::new("FileSpecSegmentInAlterDatabase").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable();
            let filegroups = Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::keyword("REMOVE").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("FILEGROUP").to_matchable(),
            ])
            .to_matchable();
            let accelerated_recovery = Sequence::new(vec![
                Ref::keyword("ACCELERATED_DATABASE_RECOVERY").to_matchable(),
                one_of(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("OFF").to_matchable(),
                ])
                .to_matchable(),
                Bracketed::new(vec![
                    Ref::keyword("PERSISTENT_VERSION_STORE_FILEGROUP").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("NakedOrQuotedIdentifierGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable();
            let recovery_options = Sequence::new(vec![
                Ref::keyword("RECOVERY").to_matchable(),
                one_of(vec![
                    Ref::keyword("FULL").to_matchable(),
                    Ref::keyword("SIMPLE").to_matchable(),
                    Ref::keyword("BULK_LOGGED").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable();
            let set_option = Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                one_of(vec![
                    optionally_bracketed(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("CompatibilityLevelSegment").to_matchable(),
                                Ref::new("AutoOptionSegment").to_matchable(),
                                accelerated_recovery,
                                Sequence::new(vec![
                                    Ref::new("NakedIdentifierSegment").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("ON").to_matchable(),
                                        Ref::keyword("OFF").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("NakedIdentifierSegment").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    Ref::new("NumericLiteralSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("KB").to_matchable(),
                                        Ref::keyword("MB").to_matchable(),
                                        Ref::keyword("GB").to_matchable(),
                                        Ref::keyword("TB").to_matchable(),
                                    ])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    recovery_options,
                ])
                .to_matchable(),
            ])
            .to_matchable();
            let secondary = Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::keyword("REMOVE").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("SECONDARY").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("SERVER").to_matchable(),
                Ref::new("NakedOrQuotedIdentifierGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("AllowConnectionsSegment").to_matchable(),
                                Ref::new("ServiceObjectiveSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable();
            let modify_options = Sequence::new(vec![
                Ref::keyword("MODIFY").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("FileSpecMaxSizeSegment").to_matchable(),
                            Ref::new("EditionSegment").to_matchable(),
                            Ref::new("ServiceObjectiveSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("MANUAL_CUTOVER").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable();
            let modify_backup = Sequence::new(vec![
                Ref::keyword("MODIFY").to_matchable(),
                Ref::new("BackupStorageRedundancySegment").to_matchable(),
            ])
            .to_matchable();

            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                Ref::keyword("DATABASE").to_matchable(),
                one_of(vec![
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                    Ref::keyword("CURRENT").to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    modify_name,
                    modify_backup,
                    add_or_modify_files,
                    filegroups,
                    modify_options,
                    Ref::new("CollateGrammar").to_matchable(),
                    set_option,
                    secondary,
                    Ref::keyword("PERFORM_CUTOVER").to_matchable(),
                    Ref::keyword("FAILOVER").to_matchable(),
                    Ref::keyword("FORCE_FAILOVER_ALLOW_DATA_LOSS").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
}
