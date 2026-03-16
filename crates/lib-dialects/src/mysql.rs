use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{one_of, optionally_bracketed, AnyNumberOf};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::TypedParser;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use super::ansi;
use super::mysql_keywords::{MYSQL_RESERVED_KEYWORDS, MYSQL_UNRESERVED_KEYWORDS};
use sqruff_lib_core::dialects::init::DialectConfig;
use sqruff_lib_core::value::Value;

sqruff_lib_core::dialect_config!(MySQLDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: MySQLDialectConfig = config
        .map(MySQLDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut mysql = ansi::raw_dialect();
    mysql.name = DialectKind::Mysql;

    // Set MySQL keywords (matching SQLFluff's approach):
    // - Do not clear inherited unreserved ANSI keywords, just extend them
    // - Clear and replace reserved keywords with MySQL-specific list
    mysql.update_keywords_set_from_multiline_string(
        "unreserved_keywords",
        MYSQL_UNRESERVED_KEYWORDS,
    );
    mysql.sets_mut("reserved_keywords").clear();
    mysql.update_keywords_set_from_multiline_string(
        "reserved_keywords",
        MYSQL_RESERVED_KEYWORDS,
    );

    // MySQL uses # for inline comments in addition to --
    mysql.patch_lexer_matchers(vec![Matcher::regex(
        "inline_comment",
        r"(^--|-- |#)[^\n]*",
        SyntaxKind::InlineComment,
    )]);

    // MySQL session variables: @var and @@global_var
    mysql.insert_lexer_matchers(
        vec![Matcher::regex(
            "at_sign_literal",
            r"@@?[a-zA-Z_][a-zA-Z0-9_]*",
            SyntaxKind::AtSignLiteral,
        )],
        "equals",
    );

    // MySQL 8.0+ supports CTEs with DML statements (INSERT, UPDATE, DELETE)
    mysql.add([(
        "NonWithSelectableGrammar".into(),
        one_of(vec![
            Ref::new("SetExpressionSegment").to_matchable(),
            optionally_bracketed(vec![Ref::new("SelectStatementSegment").to_matchable()])
                .to_matchable(),
            Ref::new("NonSetSelectableGrammar").to_matchable(),
            Ref::new("UpdateStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("DeleteStatementSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // DIV operator
    mysql.add([
        (
            "DivBinaryOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Ref::keyword("DIV").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
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
                Ref::new("DivBinaryOperatorSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Session variable segment (@var, @@var)
    mysql.add([(
        "SessionVariableSegment".into(),
        TypedParser::new(SyntaxKind::AtSignLiteral, SyntaxKind::AtSignLiteral)
            .to_matchable()
            .into(),
    )]);

    // Session variables can be used as column references in expressions
    mysql.add([(
        "ColumnReferenceSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnReference, |_| {
            one_of(vec![
                Ref::new("SessionVariableSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    AnyNumberOf::new(vec![Sequence::new(vec![
                        Ref::new("DotSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable()])
                    .to_matchable(),
                ])
                .terminators(vec![
                    Ref::new("CastOperatorSegment").to_matchable(),
                    Ref::new("StartSquareBracketSegment").to_matchable(),
                    Ref::new("CreateCastStatementSegment").to_matchable(),
                ])
                .config(|this| {
                    this.parse_mode(ParseMode::GreedyOnceStarted);
                })
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- CALL statement ----
    mysql.add([(
        "CallStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CallStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CALL").to_matchable(),
                Ref::new("FunctionNameSegment").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .config(|this| this.optional())
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

    // ---- DECLARE statements ----
    mysql.add([(
        "DeclareStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DeclareSegment, |_| {
            one_of(vec![
                // DECLARE variable datatype [DEFAULT value]
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Delimited::new(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DatatypeSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                // DECLARE cursor_name CURSOR FOR select_statement
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("CURSOR").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("SelectStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                // DECLARE condition_name CONDITION FOR value
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("CONDITION").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    one_of(vec![
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SQLSTATE").to_matchable(),
                            Ref::keyword("VALUE").optional().to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                // DECLARE [CONTINUE|EXIT|UNDO] HANDLER FOR condition statement
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("CONTINUE").to_matchable(),
                        Ref::keyword("EXIT").to_matchable(),
                        Ref::keyword("UNDO").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("HANDLER").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Delimited::new(vec![one_of(vec![
                        Ref::keyword("SQLEXCEPTION").to_matchable(),
                        Ref::keyword("SQLWARNING").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NOT").to_matchable(),
                            Ref::keyword("FOUND").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SQLSTATE").to_matchable(),
                            Ref::keyword("VALUE").optional().to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable()])
                    .to_matchable(),
                    // Handler body - can be a simple statement or BEGIN...END block
                    one_of(vec![
                        Ref::new("BeginEndSegment").to_matchable(),
                        Ref::new("StatementSegment").to_matchable(),
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

    // ---- Multi-statement containers for control flow ----

    // Statements inside IF blocks
    mysql.add([(
        "IfStatementsSegment".into(),
        NodeMatcher::new(SyntaxKind::IfStatements, |_| {
            AnyNumberOf::new(vec![Sequence::new(vec![
                one_of(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("MultiStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
            ])
            .to_matchable()])
            .config(|this| {
                this.terminators = vec![
                    Ref::keyword("ELSE").to_matchable(),
                    Ref::keyword("ELSEIF").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("END").to_matchable(),
                        Ref::keyword("IF").to_matchable(),
                    ])
                    .to_matchable(),
                ];
                this.parse_mode = ParseMode::Greedy;
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // IF statement
    mysql.add([(
        "IfStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::IfStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("IF").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::keyword("THEN").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("IfStatementsSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
                AnyNumberOf::new(vec![Sequence::new(vec![
                    Ref::keyword("ELSEIF").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("IfStatementsSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ELSE").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("IfStatementsSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("END").to_matchable(),
                Ref::keyword("IF").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Statements inside WHILE blocks
    mysql.add([(
        "WhileStatementsSegment".into(),
        NodeMatcher::new(SyntaxKind::WhileStatements, |_| {
            AnyNumberOf::new(vec![Sequence::new(vec![
                one_of(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("MultiStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
            ])
            .to_matchable()])
            .config(|this| {
                this.terminators = vec![Sequence::new(vec![
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("WHILE").to_matchable(),
                ])
                .to_matchable()];
                this.parse_mode = ParseMode::Greedy;
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // WHILE statement: [label:] WHILE condition DO statements END WHILE [label]
    mysql.add([(
        "WhileStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::WhileStatement, |_| {
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("ColonSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("WHILE").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::keyword("DO").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("WhileStatementsSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
                Ref::keyword("WHILE").to_matchable(),
                Ref::new("SingleIdentifierGrammar")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Statements inside LOOP blocks
    mysql.add([(
        "LoopStatementsSegment".into(),
        NodeMatcher::new(SyntaxKind::LoopStatements, |_| {
            AnyNumberOf::new(vec![Sequence::new(vec![
                one_of(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("MultiStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
            ])
            .to_matchable()])
            .config(|this| {
                this.terminators = vec![Sequence::new(vec![
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("LOOP").to_matchable(),
                ])
                .to_matchable()];
                this.parse_mode = ParseMode::Greedy;
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // LOOP statement: [label:] LOOP statements END LOOP [label]
    mysql.add([(
        "LoopStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::LoopStatement, |_| {
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("ColonSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("LOOP").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("LoopStatementsSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
                Ref::keyword("LOOP").to_matchable(),
                Ref::new("SingleIdentifierGrammar")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Statements inside REPEAT blocks
    mysql.add([(
        "RepeatStatementsSegment".into(),
        NodeMatcher::new(SyntaxKind::RepeatStatements, |_| {
            AnyNumberOf::new(vec![Sequence::new(vec![
                one_of(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("MultiStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
            ])
            .to_matchable()])
            .config(|this| {
                this.terminators = vec![Ref::keyword("UNTIL").to_matchable()];
                this.parse_mode = ParseMode::Greedy;
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // REPEAT statement: [label:] REPEAT statements UNTIL condition END REPEAT [label]
    mysql.add([(
        "RepeatStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::RepeatStatement, |_| {
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("ColonSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("REPEAT").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("RepeatStatementsSegment").to_matchable(),
                Ref::keyword("UNTIL").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
                Ref::keyword("REPEAT").to_matchable(),
                Ref::new("SingleIdentifierGrammar")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- BEGIN...END block ----
    // Statements inside BEGIN...END blocks
    mysql.add([(
        "ProcedureStatements".into(),
        NodeMatcher::new(SyntaxKind::ProcedureStatements, |_| {
            AnyNumberOf::new(vec![Sequence::new(vec![
                one_of(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("MultiStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
            ])
            .to_matchable()])
            .config(|this| {
                this.terminators = vec![Ref::keyword("END").to_matchable()];
                this.parse_mode = ParseMode::Greedy;
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // BEGIN...END block with optional label
    mysql.add([(
        "BeginEndSegment".into(),
        NodeMatcher::new(SyntaxKind::BeginEndBlock, |_| {
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("ColonSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("BEGIN").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("ProcedureStatements").to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
                Ref::new("SingleIdentifierGrammar")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // MultiStatementSegment - container for compound statements
    mysql.add([(
        "MultiStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::MultiStatementSegment, |_| {
            one_of(vec![
                Ref::new("IfStatementSegment").to_matchable(),
                Ref::new("WhileStatementSegment").to_matchable(),
                Ref::new("LoopStatementSegment").to_matchable(),
                Ref::new("RepeatStatementSegment").to_matchable(),
                Ref::new("BeginEndSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- SELECT clause modifier (MySQL-specific keywords) ----
    mysql.replace_grammar(
        "SelectClauseModifierSegment",
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("DISTINCT").to_matchable(),
                Ref::keyword("ALL").to_matchable(),
                Ref::keyword("DISTINCTROW").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::keyword("HIGH_PRIORITY").optional().to_matchable(),
            Ref::keyword("STRAIGHT_JOIN").optional().to_matchable(),
            Ref::keyword("SQL_SMALL_RESULT").optional().to_matchable(),
            Ref::keyword("SQL_BIG_RESULT").optional().to_matchable(),
            Ref::keyword("SQL_BUFFER_RESULT")
                .optional()
                .to_matchable(),
            Ref::keyword("SQL_CACHE").optional().to_matchable(),
            Ref::keyword("SQL_NO_CACHE").optional().to_matchable(),
            Ref::keyword("SQL_CALC_FOUND_ROWS")
                .optional()
                .to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- FOR UPDATE / FOR SHARE / LOCK IN SHARE MODE ----
    mysql.add([(
        "ForClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::ForClause, |_| {
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("FOR").to_matchable(),
                    one_of(vec![
                        Ref::keyword("UPDATE").to_matchable(),
                        Ref::keyword("SHARE").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OF").to_matchable(),
                        Delimited::new(vec![
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("NOWAIT").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SKIP").to_matchable(),
                            Ref::keyword("LOCKED").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LOCK").to_matchable(),
                    Ref::keyword("IN").to_matchable(),
                    Ref::keyword("SHARE").to_matchable(),
                    Ref::keyword("MODE").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override SelectStatementSegment to include FOR UPDATE / LOCK IN SHARE MODE
    mysql.replace_grammar(
        "SelectStatementSegment",
        ansi::select_statement()
            .copy(
                Some(vec![Ref::new("ForClauseSegment")
                    .optional()
                    .to_matchable()]),
                None,
                None,
                None,
                Vec::new(),
                false,
            ),
    );

    // ---- SELECT INTO clause ----
    // MySQL supports: SELECT ... INTO OUTFILE/DUMPFILE/@var FROM ...
    // We override the UnorderedSelectStatementSegment to include INTO clause
    mysql.replace_grammar(
        "UnorderedSelectStatementSegment",
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").to_matchable(),
            MetaSegment::dedent().to_matchable(),
            // INTO clause can appear here (before FROM)
            Ref::new("SelectIntoClauseSegment")
                .optional()
                .to_matchable(),
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
        .to_matchable(),
    );

    // SELECT INTO clause
    mysql.add([(
        "SelectIntoClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::IntoOutfileClause, |_| {
            Sequence::new(vec![
                Ref::keyword("INTO").to_matchable(),
                one_of(vec![
                    // INTO OUTFILE with options
                    Sequence::new(vec![
                        Ref::keyword("OUTFILE").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        // FIELDS/COLUMNS clause
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("FIELDS").to_matchable(),
                                Ref::keyword("COLUMNS").to_matchable(),
                            ])
                            .to_matchable(),
                            AnyNumberOf::new(vec![
                                Sequence::new(vec![
                                    Ref::keyword("TERMINATED").to_matchable(),
                                    Ref::keyword("BY").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("OPTIONALLY").optional().to_matchable(),
                                    Ref::keyword("ENCLOSED").to_matchable(),
                                    Ref::keyword("BY").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ESCAPED").to_matchable(),
                                    Ref::keyword("BY").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        // LINES clause
                        Sequence::new(vec![
                            Ref::keyword("LINES").to_matchable(),
                            AnyNumberOf::new(vec![
                                Sequence::new(vec![
                                    Ref::keyword("STARTING").to_matchable(),
                                    Ref::keyword("BY").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("TERMINATED").to_matchable(),
                                    Ref::keyword("BY").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // INTO DUMPFILE
                    Sequence::new(vec![
                        Ref::keyword("DUMPFILE").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // INTO @var, @var2, ...
                    Delimited::new(vec![one_of(vec![
                        Ref::new("SessionVariableSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable()])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- PARTITION clause ----
    mysql.add([(
        "PartitionClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::PartitionClause, |_| {
            Sequence::new(vec![
                Ref::keyword("PARTITION").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
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

    // ---- Index hints (USE/FORCE/IGNORE INDEX/KEY) ----
    mysql.add([(
        "IndexHintClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::IndexHintClause, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("USE").to_matchable(),
                    Ref::keyword("IGNORE").to_matchable(),
                    Ref::keyword("FORCE").to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("FOR").to_matchable(),
                    one_of(vec![
                        Ref::keyword("JOIN").to_matchable(),
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
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // PostTableExpressionGrammar - allows PARTITION and index hints after table references
    mysql.add([(
        "PostTableExpressionGrammar".into(),
        Sequence::new(vec![
            Ref::new("PartitionClauseSegment")
                .optional()
                .to_matchable(),
            AnyNumberOf::new(vec![
                Ref::new("IndexHintClauseSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // ---- PREPARE / EXECUTE / DEALLOCATE ----
    mysql.add([
        (
            "PrepareStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::PrepareSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("PREPARE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("SessionVariableSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExecuteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ExecuteSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("EXECUTE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Delimited::new(vec![one_of(vec![
                            Ref::new("SessionVariableSegment").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                        ])
                        .to_matchable()])
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
            "DeallocateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeallocateSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("DEALLOCATE").to_matchable(),
                        Ref::keyword("DROP").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("PREPARE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // ---- SIGNAL / RESIGNAL ----
    mysql.add([
        (
            "SignalStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ResignalSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("SIGNAL").to_matchable(),
                        Ref::keyword("RESIGNAL").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("SQLSTATE").to_matchable(),
                            Ref::keyword("VALUE").optional().to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("SET").to_matchable(),
                        Delimited::new(vec![Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("CLASS_ORIGIN").to_matchable(),
                                Ref::keyword("SUBCLASS_ORIGIN").to_matchable(),
                                Ref::keyword("RETURNED_SQLSTATE").to_matchable(),
                                Ref::keyword("MESSAGE_TEXT").to_matchable(),
                                Ref::keyword("MYSQL_ERRNO").to_matchable(),
                                Ref::keyword("CONSTRAINT_CATALOG").to_matchable(),
                                Ref::keyword("CONSTRAINT_SCHEMA").to_matchable(),
                                Ref::keyword("CONSTRAINT_NAME").to_matchable(),
                                Ref::keyword("CATALOG_NAME").to_matchable(),
                                Ref::keyword("SCHEMA_NAME").to_matchable(),
                                Ref::keyword("TABLE_NAME").to_matchable(),
                                Ref::keyword("COLUMN_NAME").to_matchable(),
                                Ref::keyword("CURSOR_NAME").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable()])
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

    // ---- GET DIAGNOSTICS ----
    mysql.add([(
        "GetDiagnosticsStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::GetDiagnosticsSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("GET").to_matchable(),
                one_of(vec![
                    Ref::keyword("CURRENT").to_matchable(),
                    Ref::keyword("STACKED").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("DIAGNOSTICS").to_matchable(),
                // Statement info or condition info
                one_of(vec![
                    // Statement diagnostics: GET DIAGNOSTICS @var = ROW_COUNT
                    Sequence::new(vec![
                        Delimited::new(vec![Sequence::new(vec![
                            one_of(vec![
                                Ref::new("SessionVariableSegment").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("NUMBER").to_matchable(),
                                Ref::keyword("ROW_COUNT").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable()])
                        .to_matchable(),
                        // Optional CONDITION clause after statement info
                        Sequence::new(vec![
                            Ref::keyword("CONDITION").to_matchable(),
                            one_of(vec![
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Ref::new("SessionVariableSegment").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // Condition diagnostics: GET DIAGNOSTICS CONDITION n @var = item
                    Sequence::new(vec![
                        Ref::keyword("CONDITION").to_matchable(),
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("SessionVariableSegment").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        // Condition info items (optional)
                        Delimited::new(vec![Sequence::new(vec![
                            one_of(vec![
                                Ref::new("SessionVariableSegment").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("CLASS_ORIGIN").to_matchable(),
                                Ref::keyword("SUBCLASS_ORIGIN").to_matchable(),
                                Ref::keyword("RETURNED_SQLSTATE").to_matchable(),
                                Ref::keyword("MESSAGE_TEXT").to_matchable(),
                                Ref::keyword("MYSQL_ERRNO").to_matchable(),
                                Ref::keyword("CONSTRAINT_CATALOG").to_matchable(),
                                Ref::keyword("CONSTRAINT_SCHEMA").to_matchable(),
                                Ref::keyword("CONSTRAINT_NAME").to_matchable(),
                                Ref::keyword("CATALOG_NAME").to_matchable(),
                                Ref::keyword("SCHEMA_NAME").to_matchable(),
                                Ref::keyword("TABLE_NAME").to_matchable(),
                                Ref::keyword("COLUMN_NAME").to_matchable(),
                                Ref::keyword("CURSOR_NAME").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable()])
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
    )]);

    // ---- OPEN / CLOSE cursor ----
    mysql.add([(
        "CursorOpenCloseSegment".into(),
        NodeMatcher::new(SyntaxKind::CursorOpenCloseSegment, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("OPEN").to_matchable(),
                    Ref::keyword("CLOSE").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- FETCH cursor ----
    mysql.add([(
        "CursorFetchSegment".into(),
        NodeMatcher::new(SyntaxKind::CursorFetchSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("FETCH").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("NEXT").optional().to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::keyword("INTO").to_matchable(),
                Delimited::new(vec![one_of(vec![
                    Ref::new("SessionVariableSegment").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable()])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- SET statement (MySQL session variables) ----
    mysql.add([(
        "SetStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::SetStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                Delimited::new(vec![Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("GLOBAL").to_matchable(),
                        Ref::keyword("SESSION").to_matchable(),
                        Ref::keyword("LOCAL").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Ref::new("SessionVariableSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- DROP PROCEDURE / DROP FUNCTION ----
    mysql.add([
        (
            "DropProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropProcedureStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("PROCEDURE").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Override DropFunctionStatementSegment to support backtick-quoted names
    mysql.replace_grammar(
        "DropFunctionStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("FUNCTION").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("FunctionNameSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- Override FileSegment to include MultiStatementSegment ----
    mysql.replace_grammar(
        "FileSegment",
        Sequence::new(vec![
            Sequence::new(vec![one_of(vec![
                Ref::new("MultiStatementSegment").to_matchable(),
                Ref::new("StatementSegment").to_matchable(),
            ])
            .to_matchable()])
            .to_matchable(),
            AnyNumberOf::new(vec![
                Ref::new("DelimiterGrammar").to_matchable(),
                one_of(vec![
                    Ref::new("MultiStatementSegment").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("DelimiterGrammar").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- Override StatementSegment to include MySQL-specific statements ----
    mysql.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec![
                Ref::new("CallStatementSegment").to_matchable(),
                Ref::new("DeclareStatementSegment").to_matchable(),
                Ref::new("SetStatementSegment").to_matchable(),
                Ref::new("PrepareStatementSegment").to_matchable(),
                Ref::new("ExecuteStatementSegment").to_matchable(),
                Ref::new("DeallocateStatementSegment").to_matchable(),
                Ref::new("SignalStatementSegment").to_matchable(),
                Ref::new("GetDiagnosticsStatementSegment").to_matchable(),
                Ref::new("CursorOpenCloseSegment").to_matchable(),
                Ref::new("CursorFetchSegment").to_matchable(),
                Ref::new("DropProcedureStatementSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    mysql
}
