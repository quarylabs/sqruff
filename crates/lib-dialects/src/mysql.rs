use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, any_set_of, one_of};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use super::ansi;
use crate::mysql_keywords::{
    MYSQL_RESERVED_KEYWORDS, MYSQL_RESERVED_KEYWORDS_REMOVE, MYSQL_UNRESERVED_KEYWORDS,
};
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

    // ============================================================
    // Lexer matchers
    // ============================================================

    // Patch inline comments to support # syntax, and single/double quotes for MySQL escaping.
    mysql.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r"(--|#)[^\n]*", SyntaxKind::InlineComment),
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
    ]);

    // Hexadecimal and bit value literals before numeric_literal.
    mysql.insert_lexer_matchers(
        vec![
            Matcher::regex(
                "hexadecimal_literal",
                r"([xX]'([\da-fA-F][\da-fA-F])+'|0x[\da-fA-F]+)",
                SyntaxKind::NumericLiteral,
            ),
            Matcher::regex(
                "bit_value_literal",
                r"([bB]'[01]+'|0b[01]+)",
                SyntaxKind::NumericLiteral,
            ),
        ],
        "numeric_literal",
    );

    // @ sign variables (session and system variables).
    mysql.insert_lexer_matchers(
        vec![Matcher::regex(
            "at_sign",
            r"@@?[a-zA-Z0-9_$]*(\.[a-zA-Z0-9_$]+)?",
            SyntaxKind::AtSignLiteral,
        )],
        "word",
    );

    // && operator before &.
    mysql.insert_lexer_matchers(
        vec![Matcher::string(
            "double_ampersand",
            "&&",
            SyntaxKind::DoubleAmpersand,
        )],
        "ampersand",
    );

    // || operator before |.
    mysql.insert_lexer_matchers(
        vec![Matcher::string(
            "double_vertical_bar",
            "||",
            SyntaxKind::DoubleVerticalBar,
        )],
        "vertical_bar",
    );

    // := walrus operator before =.
    mysql.insert_lexer_matchers(
        vec![Matcher::string(
            "walrus_operator",
            ":=",
            SyntaxKind::WalrusOperator,
        )],
        "equals",
    );

    // JSON path operators ->> and -> before >.
    mysql.insert_lexer_matchers(
        vec![
            Matcher::string(
                "inline_path_operator",
                "->>",
                SyntaxKind::InlinePathOperator,
            ),
            Matcher::string("column_path_operator", "->", SyntaxKind::ColumnPathOperator),
        ],
        "greater_than",
    );

    // ============================================================
    // Keywords
    // ============================================================

    // Add MySQL unreserved keywords (don't clear inherited ANSI unreserved keywords).
    for kw in MYSQL_UNRESERVED_KEYWORDS.lines() {
        let kw = kw.trim();
        if !kw.is_empty() {
            mysql.sets_mut("unreserved_keywords").insert(
                // SAFETY: We're leaking static strings from our keywords module.
                // This is the same pattern used by other dialects.
                kw,
            );
        }
    }

    // Replace reserved keywords entirely.
    mysql.sets_mut("reserved_keywords").clear();
    for kw in MYSQL_RESERVED_KEYWORDS.lines() {
        let kw = kw.trim();
        if !kw.is_empty() {
            mysql.sets_mut("reserved_keywords").insert(kw);
        }
    }

    // Remove some reserved keywords to avoid parsing issues.
    for kw in MYSQL_RESERVED_KEYWORDS_REMOVE {
        mysql.sets_mut("reserved_keywords").remove(kw);
    }

    // ============================================================
    // Grammar replacements (overriding ANSI)
    // ============================================================

    // Double-quoted literal segment.
    mysql.add([(
        "DoubleQuotedLiteralSegment".into(),
        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedLiteral)
            .to_matchable()
            .into(),
    )]);

    // AtSignLiteralSegment.
    mysql.add([(
        "AtSignLiteralSegment".into(),
        TypedParser::new(SyntaxKind::AtSignLiteral, SyntaxKind::AtSignLiteral)
            .to_matchable()
            .into(),
    )]);

    // SystemVariableSegment - @@session.var or @@global.var.
    mysql.add([(
        "SystemVariableSegment".into(),
        RegexParser::new(
            r"@@(session|global)\.[A-Za-z0-9_]+",
            SyntaxKind::SystemVariable,
        )
        .to_matchable()
        .into(),
    )]);

    // DoubleQuotedJSONPath.
    mysql.add([(
        "DoubleQuotedJSONPath".into(),
        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::JsonPath)
            .to_matchable()
            .into(),
    )]);

    // SingleQuotedJSONPath.
    mysql.add([(
        "SingleQuotedJSONPath".into(),
        TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::JsonPath)
            .to_matchable()
            .into(),
    )]);

    // Parameter direction segments.
    mysql.add([
        (
            "OutputParameterSegment".into(),
            StringParser::new("OUT", SyntaxKind::ParameterDirection)
                .to_matchable()
                .into(),
        ),
        (
            "InputParameterSegment".into(),
            StringParser::new("IN", SyntaxKind::ParameterDirection)
                .to_matchable()
                .into(),
        ),
        (
            "InputOutputParameterSegment".into(),
            StringParser::new("INOUT", SyntaxKind::ParameterDirection)
                .to_matchable()
                .into(),
        ),
    ]);

    // ProcedureParameterGrammar.
    mysql.add([(
        "ProcedureParameterGrammar".into(),
        one_of(vec![
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("OutputParameterSegment").to_matchable(),
                    Ref::new("InputParameterSegment").to_matchable(),
                    Ref::new("InputOutputParameterSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("ParameterNameSegment").optional().to_matchable(),
                Ref::new("DatatypeSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("DatatypeSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // LocalVariableNameSegment.
    mysql.add([(
        "LocalVariableNameSegment".into(),
        RegexParser::new(r"`?[a-zA-Z0-9_$]*`?", SyntaxKind::Variable)
            .to_matchable()
            .into(),
    )]);

    // SessionVariableNameSegment - @name.
    mysql.add([(
        "SessionVariableNameSegment".into(),
        RegexParser::new(r"[@][a-zA-Z0-9_$]*", SyntaxKind::Variable)
            .to_matchable()
            .into(),
    )]);

    // WalrusOperatorSegment - :=.
    mysql.add([(
        "WalrusOperatorSegment".into(),
        StringParser::new(":=", SyntaxKind::AssignmentOperator)
            .to_matchable()
            .into(),
    )]);

    // VariableAssignmentSegment.
    mysql.add([(
        "VariableAssignmentSegment".into(),
        Sequence::new(vec![
            Ref::new("SessionVariableNameSegment").to_matchable(),
            Ref::new("WalrusOperatorSegment").to_matchable(),
            Ref::new("BaseExpressionElementGrammar").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // JSON column path operators.
    mysql.add([
        (
            "ColumnPathOperatorSegment".into(),
            StringParser::new("->", SyntaxKind::ColumnPathOperator)
                .to_matchable()
                .into(),
        ),
        (
            "InlinePathOperatorSegment".into(),
            StringParser::new("->>", SyntaxKind::InlinePathOperator)
                .to_matchable()
                .into(),
        ),
    ]);

    // BooleanDynamicSystemVariablesGrammar.
    mysql.add([(
        "BooleanDynamicSystemVariablesGrammar".into(),
        one_of(vec![
            one_of(vec![
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("OFF").to_matchable(),
            ])
            .to_matchable(),
            one_of(vec![
                Ref::keyword("TRUE").to_matchable(),
                Ref::keyword("FALSE").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // BracketedKeyPartListGrammar - (key_part, ...).
    mysql.add([(
        "BracketedKeyPartListGrammar".into(),
        Bracketed::new(vec![
            Delimited::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
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
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // TildeSegment for delimiter grammar.
    mysql.add([(
        "TildeSegment".into(),
        StringParser::new("~", SyntaxKind::StatementTerminator)
            .to_matchable()
            .into(),
    )]);

    // ============================================================
    // Grammar replacements
    // ============================================================

    // QuotedIdentifierSegment - MySQL uses backticks.
    mysql.replace_grammar(
        "QuotedIdentifierSegment",
        TypedParser::new(SyntaxKind::BackQuote, SyntaxKind::QuotedIdentifier).to_matchable(),
    );

    // LiteralGrammar - add double-quoted literals and system variables.
    let literal_grammar = mysql.grammar("LiteralGrammar");
    mysql.replace_grammar(
        "LiteralGrammar",
        literal_grammar.copy(
            Some(vec![
                Ref::new("DoubleQuotedLiteralSegment").to_matchable(),
                Ref::new("SystemVariableSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    // FromClauseTerminatorGrammar - add index hints, partition, FOR, INTO.
    let from_clause_terminator = mysql.grammar("FromClauseTerminatorGrammar");
    mysql.replace_grammar(
        "FromClauseTerminatorGrammar",
        from_clause_terminator.copy(
            Some(vec![
                Ref::new("IndexHintClauseSegment").to_matchable(),
                Ref::new("SelectPartitionClauseSegment").to_matchable(),
                Ref::new("ForClauseSegment").to_matchable(),
                Ref::new("SetOperatorSegment").to_matchable(),
                Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
                Ref::new("IntoClauseSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    // WhereClauseTerminatorGrammar - add INTO clause.
    let where_clause_terminator = mysql.grammar("WhereClauseTerminatorGrammar");
    mysql.replace_grammar(
        "WhereClauseTerminatorGrammar",
        where_clause_terminator.copy(
            Some(vec![Ref::new("IntoClauseSegment").to_matchable()]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    // BaseExpressionElementGrammar - add session and local variables.
    let base_expr = mysql.grammar("BaseExpressionElementGrammar");
    mysql.replace_grammar(
        "BaseExpressionElementGrammar",
        base_expr.copy(
            Some(vec![
                Ref::new("SessionVariableNameSegment").to_matchable(),
                Ref::new("LocalVariableNameSegment").to_matchable(),
                Ref::new("VariableAssignmentSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    // DateTimeLiteralGrammar - MySQL allows optional keyword.
    mysql.replace_grammar(
        "DateTimeLiteralGrammar",
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("DATE").to_matchable(),
                Ref::keyword("TIME").to_matchable(),
                Ref::keyword("TIMESTAMP").to_matchable(),
                Ref::keyword("DATETIME").to_matchable(),
                Ref::keyword("INTERVAL").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            one_of(vec![
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral)
                    .to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // QuotedLiteralSegment - MySQL allows whitespace-concatenated string literals.
    mysql.replace_grammar(
        "QuotedLiteralSegment",
        AnyNumberOf::new(vec![
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral).to_matchable(),
            Ref::new("DoubleQuotedLiteralSegment").to_matchable(),
        ])
        .config(|this| this.min_times = 1)
        .to_matchable(),
    );

    // UniqueKeyGrammar.
    mysql.replace_grammar(
        "UniqueKeyGrammar",
        Sequence::new(vec![
            Ref::keyword("UNIQUE").to_matchable(),
            Ref::keyword("KEY").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    // CharCharacterSetGrammar.
    mysql.replace_grammar(
        "CharCharacterSetGrammar",
        Ref::keyword("BINARY").to_matchable(),
    );

    // DelimiterGrammar - semicolon or tilde.
    mysql.replace_grammar(
        "DelimiterGrammar",
        one_of(vec![
            Ref::new("SemicolonSegment").to_matchable(),
            Ref::new("TildeSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // ParameterNameSegment.
    mysql.replace_grammar(
        "ParameterNameSegment",
        RegexParser::new(r"`?[A-Za-z0-9_]*`?", SyntaxKind::Parameter).to_matchable(),
    );

    // SingleIdentifierGrammar - add session variables.
    let single_id_grammar = mysql.grammar("SingleIdentifierGrammar");
    mysql.replace_grammar(
        "SingleIdentifierGrammar",
        single_id_grammar.copy(
            Some(vec![Ref::new("SessionVariableNameSegment").to_matchable()]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    // AndOperatorGrammar - AND and &&.
    mysql.replace_grammar(
        "AndOperatorGrammar",
        one_of(vec![
            StringParser::new("AND", SyntaxKind::BinaryOperator).to_matchable(),
            StringParser::new("&&", SyntaxKind::BinaryOperator).to_matchable(),
        ])
        .to_matchable(),
    );

    // OrOperatorGrammar - OR, ||, and XOR.
    mysql.replace_grammar(
        "OrOperatorGrammar",
        one_of(vec![
            StringParser::new("OR", SyntaxKind::BinaryOperator).to_matchable(),
            StringParser::new("||", SyntaxKind::BinaryOperator).to_matchable(),
            StringParser::new("XOR", SyntaxKind::BinaryOperator).to_matchable(),
        ])
        .to_matchable(),
    );

    // NotOperatorGrammar - NOT and !.
    mysql.replace_grammar(
        "NotOperatorGrammar",
        one_of(vec![
            StringParser::new("NOT", SyntaxKind::Keyword).to_matchable(),
            StringParser::new("!", SyntaxKind::NotOperator).to_matchable(),
        ])
        .to_matchable(),
    );

    // Expression_C_Grammar - add optional session variable assignment.
    let expr_c = mysql.grammar("Expression_C_Grammar");
    mysql.replace_grammar(
        "Expression_C_Grammar",
        Sequence::new(vec![
            Sequence::new(vec![
                Ref::new("SessionVariableNameSegment").to_matchable(),
                Ref::new("WalrusOperatorSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            expr_c,
        ])
        .to_matchable(),
    );

    // ============================================================
    // MySQL-specific arithmetic operators (DIV, bitwise ops)
    // ============================================================

    mysql.add([(
        "DivBinaryOperatorSegment".into(),
        NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
            Ref::keyword("DIV").to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    mysql.replace_grammar(
        "ArithmeticBinaryOperatorGrammar",
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
        .to_matchable(),
    );

    // MySQL 8.0+ supports CTEs with DML statements.
    mysql.replace_grammar(
        "NonWithSelectableGrammar",
        one_of(vec![
            Ref::new("SetExpressionSegment").to_matchable(),
            Ref::new("SelectStatementSegment").to_matchable(),
            Ref::new("NonSetSelectableGrammar").to_matchable(),
            Ref::new("UpdateStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("DeleteStatementSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // ============================================================
    // Segment definitions
    // ============================================================

    // ColumnDefinitionSegment.
    mysql.replace_grammar(
        "ColumnDefinitionSegment",
        Sequence::new(vec![
            Ref::new("SingleIdentifierGrammar").to_matchable(),
            one_of(vec![
                Ref::new("DatatypeSegment")
                    .exclude(one_of(vec![
                        Ref::keyword("DATETIME").to_matchable(),
                        Ref::keyword("TIMESTAMP").to_matchable(),
                    ]))
                    .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("DATETIME").to_matchable(),
                        Ref::keyword("TIMESTAMP").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                            .config(|this| this.optional())
                            .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Sequence::new(vec![Ref::keyword("NOT").to_matchable()])
                            .config(|this| this.optional())
                            .to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("CURRENT_TIMESTAMP").to_matchable(),
                                    Ref::keyword("NOW").to_matchable(),
                                ])
                                .to_matchable(),
                                Bracketed::new(vec![
                                    Ref::new("NumericLiteralSegment").optional().to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("UPDATE").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::keyword("CURRENT_TIMESTAMP").to_matchable(),
                        Sequence::new(vec![
                            Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Bracketed::new(vec![
                Ref::new("ExpressionSegment").optional().to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            AnyNumberOf::new(vec![
                Ref::new("ColumnConstraintSegment")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // CreateTableStatementSegment - ANSI grammar plus MySQL table options.
    mysql.replace_grammar(
        "CreateTableStatementSegment",
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
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("SelectableGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LIKE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("TableEndClauseSegment").optional().to_matchable(),
            // MySQL table options
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::keyword("DEFAULT").optional().to_matchable(),
                    Ref::new("ParameterNameSegment").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::new("LiteralGrammar").to_matchable(),
                        Ref::new("ParameterNameSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // CreateUserStatementSegment.
    mysql.replace_grammar(
        "CreateUserStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::keyword("USER").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::new("RoleReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("IDENTIFIED").to_matchable(),
                                one_of(vec![
                                    Sequence::new(vec![
                                        Ref::keyword("BY").to_matchable(),
                                        one_of(vec![
                                            Sequence::new(vec![
                                                Ref::keyword("RANDOM").to_matchable(),
                                                Ref::keyword("PASSWORD").to_matchable(),
                                            ])
                                            .to_matchable(),
                                            Ref::new("QuotedLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("WITH").to_matchable(),
                                        Ref::new("ObjectReferenceSegment").to_matchable(),
                                        Sequence::new(vec![
                                            one_of(vec![
                                                Sequence::new(vec![
                                                    Ref::keyword("BY").to_matchable(),
                                                    one_of(vec![
                                                        Sequence::new(vec![
                                                            Ref::keyword("RANDOM").to_matchable(),
                                                            Ref::keyword("PASSWORD").to_matchable(),
                                                        ])
                                                        .to_matchable(),
                                                        Ref::new("QuotedLiteralSegment")
                                                            .to_matchable(),
                                                    ])
                                                    .to_matchable(),
                                                ])
                                                .to_matchable(),
                                                Sequence::new(vec![
                                                    Ref::keyword("AS").to_matchable(),
                                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                                ])
                                                .to_matchable(),
                                                Sequence::new(vec![
                                                    Ref::keyword("INITIAL").to_matchable(),
                                                    Ref::keyword("AUTHENTICATION").to_matchable(),
                                                    Ref::keyword("IDENTIFIED").to_matchable(),
                                                    one_of(vec![
                                                        Sequence::new(vec![
                                                            Ref::keyword("BY").to_matchable(),
                                                            one_of(vec![
                                                                Sequence::new(vec![
                                                                    Ref::keyword("RANDOM")
                                                                        .to_matchable(),
                                                                    Ref::keyword("PASSWORD")
                                                                        .to_matchable(),
                                                                ])
                                                                .to_matchable(),
                                                                Ref::new("QuotedLiteralSegment")
                                                                    .to_matchable(),
                                                            ])
                                                            .to_matchable(),
                                                        ])
                                                        .to_matchable(),
                                                        Sequence::new(vec![
                                                            Ref::keyword("WITH").to_matchable(),
                                                            Ref::new("ObjectReferenceSegment")
                                                                .to_matchable(),
                                                            Ref::keyword("AS").to_matchable(),
                                                            Ref::new("QuotedLiteralSegment")
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
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.delimiter(Ref::keyword("AND")))
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("DEFAULT").to_matchable(),
                Ref::keyword("ROLE").to_matchable(),
                Delimited::new(vec![Ref::new("RoleReferenceSegment").to_matchable()])
                    .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("REQUIRE").to_matchable(),
                one_of(vec![
                    Ref::keyword("NONE").to_matchable(),
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::keyword("SSL").to_matchable(),
                            Ref::keyword("X509").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CIPHER").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ISSUER").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("SUBJECT").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.delimiter(Ref::keyword("AND")))
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("MAX_QUERIES_PER_HOUR").to_matchable(),
                            Ref::keyword("MAX_UPDATES_PER_HOUR").to_matchable(),
                            Ref::keyword("MAX_CONNECTIONS_PER_HOUR").to_matchable(),
                            Ref::keyword("MAX_USER_CONNECTIONS").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::keyword("PASSWORD").to_matchable(),
                    Ref::keyword("EXPIRE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("NEVER").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INTERVAL").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::keyword("DAY").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("PASSWORD").to_matchable(),
                    Ref::keyword("HISTORY").to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("PASSWORD").to_matchable(),
                    Ref::keyword("REUSE").to_matchable(),
                    Ref::keyword("INTERVAL").to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::keyword("DAY").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("PASSWORD").to_matchable(),
                    Ref::keyword("REQUIRE").to_matchable(),
                    Ref::keyword("CURRENT").to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("OPTIONAL").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("FAILED_LOGIN_ATTEMPTS").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("PASSWORD_LOCK_TIME").to_matchable(),
                    one_of(vec![
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::keyword("UNBOUNDED").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ACCOUNT").to_matchable(),
                one_of(vec![
                    Ref::keyword("UNLOCK").to_matchable(),
                    Ref::keyword("LOCK").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("COMMENT").to_matchable(),
                    Ref::keyword("ATTRIBUTE").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // ColumnConstraintSegment - add CHARACTER SET and COLLATE.
    // We inline the ANSI grammar here rather than using mysql.grammar() which would
    // embed a NodeMatcher directly (NodeMatcher doesn't support cache_key).
    mysql.replace_grammar(
        "ColumnConstraintSegment",
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
                Ref::new("UniqueKeyGrammar").to_matchable(),
                Ref::new("AutoIncrementGrammar").to_matchable(),
                Ref::new("ReferenceDefinitionGrammar").to_matchable(),
                Ref::new("CommentClauseSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("COLLATE").to_matchable(),
                    Ref::new("CollationReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                // MySQL-specific: CHARACTER SET and COLLATE with NakedIdentifier
                Sequence::new(vec![
                    Ref::keyword("CHARACTER").to_matchable(),
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("COLLATE").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // IndexTypeGrammar.
    mysql.add([(
        "IndexTypeGrammar".into(),
        Sequence::new(vec![
            Ref::keyword("USING").to_matchable(),
            one_of(vec![
                Ref::keyword("BTREE").to_matchable(),
                Ref::keyword("HASH").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // IndexOptionsSegment.
    mysql.add([(
        "IndexOptionsSegment".into(),
        any_set_of(vec![
            Sequence::new(vec![
                Ref::keyword("KEY_BLOCK_SIZE").to_matchable(),
                Ref::new("EqualsSegment").optional().to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("IndexTypeGrammar").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Ref::keyword("PARSER").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("CommentClauseSegment").to_matchable(),
            one_of(vec![
                Ref::keyword("VISIBLE").to_matchable(),
                Ref::keyword("INVISIBLE").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ENGINE_ATTRIBUTE").to_matchable(),
                Ref::new("EqualsSegment").optional().to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("SECONDARY_ENGINE_ATTRIBUTE").to_matchable(),
                Ref::new("EqualsSegment").optional().to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // TableConstraintSegment.
    mysql.replace_grammar(
        "TableConstraintSegment",
        one_of(vec![
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::keyword("CONSTRAINT").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                one_of(vec![
                    // UNIQUE [INDEX | KEY]
                    Sequence::new(vec![
                        Ref::keyword("UNIQUE").to_matchable(),
                        one_of(vec![
                            Ref::keyword("INDEX").to_matchable(),
                            Ref::keyword("KEY").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("IndexReferenceSegment").optional().to_matchable(),
                        Ref::new("IndexTypeGrammar").optional().to_matchable(),
                        Ref::new("BracketedKeyPartListGrammar").to_matchable(),
                        Ref::new("IndexOptionsSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // PRIMARY KEY
                    Sequence::new(vec![
                        Ref::new("PrimaryKeyGrammar").to_matchable(),
                        Ref::new("IndexTypeGrammar").optional().to_matchable(),
                        Ref::new("BracketedKeyPartListGrammar").to_matchable(),
                        Ref::new("IndexOptionsSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // FOREIGN KEY
                    Sequence::new(vec![
                        Ref::new("ForeignKeyGrammar").to_matchable(),
                        Ref::new("IndexReferenceSegment").optional().to_matchable(),
                        Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                        Ref::keyword("REFERENCES").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("ON").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("DELETE").to_matchable(),
                                    Ref::keyword("UPDATE").to_matchable(),
                                ])
                                .to_matchable(),
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
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // CHECK
                    Sequence::new(vec![
                        Ref::keyword("CHECK").to_matchable(),
                        Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                        one_of(vec![
                            Ref::keyword("ENFORCED").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("NOT").to_matchable(),
                                Ref::keyword("ENFORCED").to_matchable(),
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
            // {INDEX | KEY}
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("IndexReferenceSegment").optional().to_matchable(),
                Ref::new("IndexTypeGrammar").optional().to_matchable(),
                Ref::new("BracketedKeyPartListGrammar").to_matchable(),
                Ref::new("IndexOptionsSegment").optional().to_matchable(),
            ])
            .to_matchable(),
            // {FULLTEXT | SPATIAL}
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("FULLTEXT").to_matchable(),
                    Ref::keyword("SPATIAL").to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("IndexReferenceSegment").optional().to_matchable(),
                Ref::new("BracketedKeyPartListGrammar").to_matchable(),
                Ref::new("IndexOptionsSegment").optional().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // CreateIndexStatementSegment.
    mysql.replace_grammar(
        "CreateIndexStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            one_of(vec![
                Ref::keyword("UNIQUE").to_matchable(),
                Ref::keyword("FULLTEXT").to_matchable(),
                Ref::keyword("SPATIAL").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::keyword("INDEX").to_matchable(),
            Ref::new("IndexReferenceSegment").to_matchable(),
            Ref::new("IndexTypeGrammar").optional().to_matchable(),
            Ref::keyword("ON").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("BracketedKeyPartListGrammar").to_matchable(),
            Ref::new("IndexOptionsSegment").optional().to_matchable(),
            any_set_of(vec![
                Sequence::new(vec![
                    Ref::keyword("ALGORITHM").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("INPLACE").to_matchable(),
                        Ref::keyword("COPY").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LOCK").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("NONE").to_matchable(),
                        Ref::keyword("SHARED").to_matchable(),
                        Ref::keyword("EXCLUSIVE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // IntervalExpressionSegment.
    mysql.replace_grammar(
        "IntervalExpressionSegment",
        Sequence::new(vec![
            Ref::keyword("INTERVAL").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("ExpressionSegment").to_matchable(),
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
        .to_matchable(),
    );

    // RoleReferenceSegment.
    mysql.replace_grammar(
        "RoleReferenceSegment",
        one_of(vec![
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::new("QuotedIdentifierSegment").to_matchable(),
                    Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                    Ref::new("DoubleQuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("AtSignLiteralSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("NakedIdentifierSegment").to_matchable(),
                        Ref::new("QuotedIdentifierSegment").to_matchable(),
                        Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                        Ref::new("DoubleQuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("CURRENT_USER").to_matchable(),
        ])
        .to_matchable(),
    );

    // UpsertClauseListSegment - ON DUPLICATE KEY UPDATE.
    mysql.add([(
        "UpsertClauseListSegment".into(),
        NodeMatcher::new(SyntaxKind::UpsertClauseList, |_| {
            Sequence::new(vec![
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("DUPLICATE").to_matchable(),
                Ref::keyword("KEY").to_matchable(),
                Ref::keyword("UPDATE").to_matchable(),
                Delimited::new(vec![Ref::new("SetClauseSegment").to_matchable()]).to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // InsertRowAliasSegment.
    mysql.add([(
        "InsertRowAliasSegment".into(),
        NodeMatcher::new(SyntaxKind::InsertRowAlias, |_| {
            Sequence::new(vec![
                Ref::keyword("AS").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Bracketed::new(vec![Ref::new("SingleIdentifierListSegment").to_matchable()])
                    .config(|this| this.optional())
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // InsertStatementSegment.
    mysql.replace_grammar(
        "InsertStatementSegment",
        Sequence::new(vec![
            Ref::keyword("INSERT").to_matchable(),
            one_of(vec![
                Ref::keyword("LOW_PRIORITY").to_matchable(),
                Ref::keyword("DELAYED").to_matchable(),
                Ref::keyword("HIGH_PRIORITY").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::keyword("IGNORE").optional().to_matchable(),
            Ref::keyword("INTO").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("PARTITION").to_matchable(),
                Bracketed::new(vec![Ref::new("SingleIdentifierListSegment").to_matchable()])
                    .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::new("BracketedColumnReferenceListGrammar")
                .optional()
                .to_matchable(),
            any_set_of(vec![
                one_of(vec![
                    Ref::new("ValuesClauseSegment").to_matchable(),
                    Ref::new("SetClauseListSegment").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("SelectableGrammar").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("TABLE").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("InsertRowAliasSegment").optional().to_matchable(),
                Ref::new("UpsertClauseListSegment")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // DeleteStatementSegment.
    mysql.replace_grammar(
        "DeleteStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DELETE").to_matchable(),
            Ref::keyword("LOW_PRIORITY").optional().to_matchable(),
            Ref::keyword("QUICK").optional().to_matchable(),
            Ref::keyword("IGNORE").optional().to_matchable(),
            one_of(vec![
                // DELETE FROM ... USING ...
                Sequence::new(vec![
                    Ref::keyword("FROM").to_matchable(),
                    Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                        .config(|this| {
                            this.base.terminators = vec![Ref::keyword("USING").to_matchable()]
                        })
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Delimited::new(vec![Ref::new("FromExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                ])
                .to_matchable(),
                // DELETE ... FROM ...
                Sequence::new(vec![
                    Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                        .config(|this| {
                            this.base.terminators = vec![Ref::keyword("FROM").to_matchable()]
                        })
                        .to_matchable(),
                    Ref::new("FromClauseSegment").to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                ])
                .to_matchable(),
                // Simple DELETE FROM ...
                Sequence::new(vec![
                    Ref::new("FromClauseSegment").to_matchable(),
                    Ref::new("SelectPartitionClauseSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                    Ref::new("OrderByClauseSegment").optional().to_matchable(),
                    Ref::new("LimitClauseSegment").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // DeclareStatement.
    mysql.add([(
        "DeclareStatement".into(),
        NodeMatcher::new(SyntaxKind::DeclareStatement, |_| {
            one_of(vec![
                // DECLARE cursor
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::keyword("CURSOR").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .to_matchable(),
                // DECLARE handler
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
                    one_of(vec![
                        Ref::keyword("SQLEXCEPTION").to_matchable(),
                        Ref::keyword("SQLWARNING").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NOT").to_matchable(),
                            Ref::keyword("FOUND").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SQLSTATE").to_matchable(),
                            Ref::keyword("VALUE").optional().to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("NakedIdentifierSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .to_matchable(),
                // DECLARE condition
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::keyword("CONDITION").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                // DECLARE local variable
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Ref::new("LocalVariableNameSegment").to_matchable(),
                    Ref::new("DatatypeSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("FunctionSegment").to_matchable(),
                        ])
                        .to_matchable(),
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

    // DelimiterStatement.
    mysql.add([(
        "DelimiterStatement".into(),
        NodeMatcher::new(SyntaxKind::DelimiterStatement, |_| {
            Ref::keyword("DELIMITER").to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // DefinerSegment.
    mysql.add([(
        "DefinerSegment".into(),
        NodeMatcher::new(SyntaxKind::DefinerSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("DEFINER").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                Ref::new("RoleReferenceSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CharacteristicStatement.
    mysql.add([(
        "CharacteristicStatement".into(),
        NodeMatcher::new(SyntaxKind::CharacteristicStatement, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("DETERMINISTIC").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("DETERMINISTIC").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LANGUAGE").to_matchable(),
                    Ref::keyword("SQL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONTAINS").to_matchable(),
                        Ref::keyword("SQL").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NO").to_matchable(),
                        Ref::keyword("SQL").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("READS").to_matchable(),
                        Ref::keyword("SQL").to_matchable(),
                        Ref::keyword("DATA").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("MODIFIES").to_matchable(),
                        Ref::keyword("SQL").to_matchable(),
                        Ref::keyword("DATA").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SQL").to_matchable(),
                    Ref::keyword("SECURITY").to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFINER").to_matchable(),
                        Ref::keyword("INVOKER").to_matchable(),
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

    // FunctionDefinitionGrammar (body of CREATE FUNCTION/PROCEDURE).
    mysql.add([(
        "FunctionDefinitionGrammar".into(),
        NodeMatcher::new(SyntaxKind::FunctionDefinition, |_| {
            Ref::new("TransactionStatementSegment").to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CreateProcedureStatementSegment.
    mysql.add([(
        "CreateProcedureStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateProcedureStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::new("DefinerSegment").optional().to_matchable(),
                Ref::keyword("PROCEDURE").to_matchable(),
                Ref::new("FunctionNameSegment").to_matchable(),
                Ref::new("ProcedureParameterListGrammar")
                    .optional()
                    .to_matchable(),
                Ref::new("CommentClauseSegment").optional().to_matchable(),
                Ref::new("CharacteristicStatement")
                    .optional()
                    .to_matchable(),
                Ref::new("FunctionDefinitionGrammar").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CreateFunctionStatementSegment.
    mysql.replace_grammar(
        "CreateFunctionStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("DefinerSegment").optional().to_matchable(),
            Ref::keyword("FUNCTION").to_matchable(),
            Ref::new("FunctionNameSegment").to_matchable(),
            Ref::new("FunctionParameterListGrammar")
                .optional()
                .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("RETURNS").to_matchable(),
                Ref::new("DatatypeSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("CommentClauseSegment").optional().to_matchable(),
            Ref::new("CharacteristicStatement").to_matchable(),
            Ref::new("FunctionDefinitionGrammar").to_matchable(),
        ])
        .to_matchable(),
    );

    // ProcedureParameterListGrammar.
    mysql.add([(
        "ProcedureParameterListGrammar".into(),
        NodeMatcher::new(SyntaxKind::ProcedureParameterList, |_| {
            Bracketed::new(vec![
                Delimited::new(vec![Ref::new("ProcedureParameterGrammar").to_matchable()])
                    .config(|this| this.optional())
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // TransactionStatementSegment.
    mysql.replace_grammar(
        "TransactionStatementSegment",
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("START").to_matchable(),
                Ref::keyword("TRANSACTION").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("ColonSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("BEGIN").to_matchable(),
                    Ref::keyword("WORK").optional().to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("LEAVE").to_matchable(),
                Ref::new("SingleIdentifierGrammar")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("COMMIT").to_matchable(),
                Ref::keyword("WORK").optional().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AND").to_matchable(),
                    Ref::keyword("NO").optional().to_matchable(),
                    Ref::keyword("CHAIN").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ROLLBACK").to_matchable(),
                Ref::keyword("WORK").optional().to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("END").to_matchable(),
                Ref::new("SingleIdentifierGrammar")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // SetAssignmentStatementSegment.
    mysql.add([(
        "SetAssignmentStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::SetStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                Delimited::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("SessionVariableNameSegment").to_matchable(),
                            Ref::new("LocalVariableNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("WalrusOperatorSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        AnyNumberOf::new(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("DoubleQuotedLiteralSegment").to_matchable(),
                            Ref::new("SessionVariableNameSegment").to_matchable(),
                            Ref::new("BooleanDynamicSystemVariablesGrammar").to_matchable(),
                            Ref::new("LocalVariableNameSegment").to_matchable(),
                            Ref::new("FunctionSegment").to_matchable(),
                            Ref::new("ArithmeticBinaryOperatorGrammar").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
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

    // IfExpressionStatement.
    mysql.add([(
        "IfExpressionStatement".into(),
        NodeMatcher::new(SyntaxKind::IfThenStatement, |_| {
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::keyword("IF").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ELSEIF").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ELSE").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("IF").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // SelectClauseModifierSegment.
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
            Ref::keyword("SQL_BUFFER_RESULT").optional().to_matchable(),
            Ref::keyword("SQL_CACHE").optional().to_matchable(),
            Ref::keyword("SQL_NO_CACHE").optional().to_matchable(),
            Ref::keyword("SQL_CALC_FOUND_ROWS")
                .optional()
                .to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
    );

    // IntoClauseSegment.
    mysql.add([(
        "IntoClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::IntoClause, |_| {
            Sequence::new(vec![
                Ref::keyword("INTO").to_matchable(),
                one_of(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("SessionVariableNameSegment").to_matchable(),
                            Ref::new("NakedIdentifierSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DUMPFILE").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OUTFILE").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CHARACTER").to_matchable(),
                            Ref::keyword("SET").to_matchable(),
                            Ref::new("NakedIdentifierSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("FIELDS").to_matchable(),
                                Ref::keyword("COLUMNS").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("TERMINATED").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("OPTIONALLY").optional().to_matchable(),
                                Ref::keyword("ENCLOSED").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ESCAPED").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("LINES").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("STARTING").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("TERMINATED").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
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
    )]);

    // ForClauseSegment.
    mysql.add([(
        "ForClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::ForClause, |_| {
            one_of(vec![
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        one_of(vec![
                            Ref::keyword("UPDATE").to_matchable(),
                            Ref::keyword("SHARE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OF").to_matchable(),
                        Delimited::new(vec![Ref::new("NakedIdentifierSegment").to_matchable()])
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
            .config(|this| this.optional())
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // IndexHintClauseSegment.
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
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Bracketed::new(vec![Ref::new("ObjectReferenceSegment").to_matchable()])
                    .to_matchable(),
                Ref::new("JoinOnConditionSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CallStoredProcedureSegment.
    mysql.add([(
        "CallStoredProcedureSegment".into(),
        NodeMatcher::new(SyntaxKind::CallSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("CALL").to_matchable(),
                one_of(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("QuotedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                Bracketed::new(vec![
                    AnyNumberOf::new(vec![
                        Delimited::new(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("DoubleQuotedLiteralSegment").to_matchable(),
                            Ref::new("SessionVariableNameSegment").to_matchable(),
                            Ref::new("LocalVariableNameSegment").to_matchable(),
                            Ref::new("FunctionSegment").to_matchable(),
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

    // SelectPartitionClauseSegment.
    mysql.add([(
        "SelectPartitionClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::PartitionClause, |_| {
            Sequence::new(vec![
                Ref::keyword("PARTITION").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ObjectReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // WhileStatementSegment.
    mysql.add([(
        "WhileStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::WhileStatement, |_| {
            one_of(vec![
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ColonSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WHILE").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                        Ref::keyword("DO").to_matchable(),
                        AnyNumberOf::new(vec![Ref::new("StatementSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("WHILE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // PrepareSegment.
    mysql.add([(
        "PrepareSegment".into(),
        NodeMatcher::new(SyntaxKind::PrepareSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("PREPARE").to_matchable(),
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::keyword("FROM").to_matchable(),
                one_of(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("SessionVariableNameSegment").to_matchable(),
                    Ref::new("LocalVariableNameSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // GetDiagnosticsSegment.
    mysql.add([(
        "GetDiagnosticsSegment".into(),
        NodeMatcher::new(SyntaxKind::GetDiagnosticsSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("GET").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CURRENT").to_matchable(),
                    Ref::keyword("STACKED").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("DIAGNOSTICS").to_matchable(),
                Delimited::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("SessionVariableNameSegment").to_matchable(),
                            Ref::new("LocalVariableNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("NUMBER").to_matchable(),
                            Ref::keyword("ROW_COUNT").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("CONDITION").to_matchable(),
                one_of(vec![
                    Ref::new("SessionVariableNameSegment").to_matchable(),
                    Ref::new("LocalVariableNameSegment").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                Delimited::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("SessionVariableNameSegment").to_matchable(),
                            Ref::new("LocalVariableNameSegment").to_matchable(),
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

    // LoopStatementSegment.
    mysql.add([(
        "LoopStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::LoopStatement, |_| {
            one_of(vec![
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ColonSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("LOOP").to_matchable(),
                    Delimited::new(vec![Ref::new("StatementSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("LOOP").to_matchable(),
                    Ref::new("SingleIdentifierGrammar")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CursorOpenCloseSegment.
    mysql.add([(
        "CursorOpenCloseSegment".into(),
        NodeMatcher::new(SyntaxKind::CursorOpenCloseSegment, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("CLOSE").to_matchable(),
                    Ref::keyword("OPEN").to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("QuotedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // IterateStatementSegment.
    mysql.add([(
        "IterateStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::IterateStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ITERATE").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ExecuteSegment.
    mysql.add([(
        "ExecuteSegment".into(),
        NodeMatcher::new(SyntaxKind::ExecuteSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("EXECUTE").to_matchable(),
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("USING").to_matchable(),
                    Delimited::new(vec![Ref::new("SessionVariableNameSegment").to_matchable()])
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

    // RepeatStatementSegment.
    mysql.add([(
        "RepeatStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::RepeatStatement, |_| {
            one_of(vec![
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ColonSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("REPEAT").to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("StatementSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("UNTIL").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("END").to_matchable(),
                        Ref::keyword("REPEAT").to_matchable(),
                        Ref::new("SingleIdentifierGrammar")
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
    )]);

    // DeallocateSegment.
    mysql.add([(
        "DeallocateSegment".into(),
        NodeMatcher::new(SyntaxKind::DeallocateSegment, |_| {
            Sequence::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("DEALLOCATE").to_matchable(),
                        Ref::keyword("DROP").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("PREPARE").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("NakedIdentifierSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ResignalSegment (handles both SIGNAL and RESIGNAL).
    mysql.add([(
        "ResignalSegment".into(),
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
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
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
                            one_of(vec![
                                Ref::new("SessionVariableNameSegment").to_matchable(),
                                Ref::new("LocalVariableNameSegment").to_matchable(),
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

    // CursorFetchSegment.
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
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::keyword("INTO").to_matchable(),
                Delimited::new(vec![
                    Ref::new("SessionVariableNameSegment").to_matchable(),
                    Ref::new("LocalVariableNameSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // DropIndexStatementSegment.
    mysql.replace_grammar(
        "DropIndexStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("INDEX").to_matchable(),
            Ref::new("IndexReferenceSegment").to_matchable(),
            Ref::keyword("ON").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("ALGORITHM").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("INPLACE").to_matchable(),
                        Ref::keyword("COPY").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LOCK").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("NONE").to_matchable(),
                        Ref::keyword("SHARED").to_matchable(),
                        Ref::keyword("EXCLUSIVE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // DropProcedureStatementSegment.
    mysql.add([(
        "DropProcedureStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropProcedureStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                one_of(vec![
                    Ref::keyword("PROCEDURE").to_matchable(),
                    Ref::keyword("FUNCTION").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // AlterTableStatementSegment.
    mysql.replace_grammar(
        "AlterTableStatementSegment",
        Sequence::new(vec![
            Ref::keyword("ALTER").to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Delimited::new(vec![
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
                    // ADD/MODIFY column
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
                                ])
                                .to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // ADD constraint
                    Sequence::new(vec![
                        Ref::keyword("ADD").to_matchable(),
                        Ref::new("TableConstraintSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // CHANGE column
                    Sequence::new(vec![
                        Ref::keyword("CHANGE").to_matchable(),
                        Ref::keyword("COLUMN").optional().to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::new("ColumnDefinitionSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("FIRST").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("AFTER").to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // DROP
                    Sequence::new(vec![
                        Ref::keyword("DROP").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("COLUMN").optional().to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("INDEX").to_matchable(),
                                    Ref::keyword("KEY").to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                                Ref::new("IndexReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("PrimaryKeyGrammar").to_matchable(),
                            Sequence::new(vec![
                                Ref::new("ForeignKeyGrammar").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("CHECK").to_matchable(),
                                    Ref::keyword("CONSTRAINT").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // ALTER CHECK/CONSTRAINT
                    Sequence::new(vec![
                        Ref::keyword("ALTER").to_matchable(),
                        one_of(vec![
                            Ref::keyword("CHECK").to_matchable(),
                            Ref::keyword("CONSTRAINT").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("ENFORCED").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("NOT").to_matchable(),
                                Ref::keyword("ENFORCED").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // ALTER INDEX
                    Sequence::new(vec![
                        Ref::keyword("ALTER").to_matchable(),
                        Ref::keyword("INDEX").to_matchable(),
                        Ref::new("IndexReferenceSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("VISIBLE").to_matchable(),
                            Ref::keyword("INVISIBLE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // RENAME
                    Sequence::new(vec![
                        Ref::keyword("RENAME").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
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
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // ENABLE/DISABLE KEYS
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("DISABLE").to_matchable(),
                            Ref::keyword("ENABLE").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("KEYS").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // WithCheckOptionSegment.
    mysql.add([(
        "WithCheckOptionSegment".into(),
        NodeMatcher::new(SyntaxKind::WithCheckOption, |_| {
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                one_of(vec![
                    Ref::keyword("CASCADED").to_matchable(),
                    Ref::keyword("LOCAL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("CHECK").to_matchable(),
                Ref::keyword("OPTION").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // AlterViewStatementSegment.
    mysql.add([(
        "AlterViewStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterViewStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ALGORITHM").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("UNDEFINED").to_matchable(),
                        Ref::keyword("MERGE").to_matchable(),
                        Ref::keyword("TEMPTABLE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("DefinerSegment").optional().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SQL").to_matchable(),
                    Ref::keyword("SECURITY").to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFINER").to_matchable(),
                        Ref::keyword("INVOKER").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("VIEW").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("BracketedColumnReferenceListGrammar")
                    .optional()
                    .to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("SelectStatementSegment").to_matchable(),
                Ref::new("WithCheckOptionSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CreateViewStatementSegment.
    mysql.replace_grammar(
        "CreateViewStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("OrReplaceGrammar").optional().to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ALGORITHM").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("UNDEFINED").to_matchable(),
                    Ref::keyword("MERGE").to_matchable(),
                    Ref::keyword("TEMPTABLE").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::new("DefinerSegment").optional().to_matchable(),
            Sequence::new(vec![
                Ref::keyword("SQL").to_matchable(),
                Ref::keyword("SECURITY").to_matchable(),
                one_of(vec![
                    Ref::keyword("DEFINER").to_matchable(),
                    Ref::keyword("INVOKER").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::keyword("VIEW").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("BracketedColumnReferenceListGrammar")
                .optional()
                .to_matchable(),
            Ref::keyword("AS").to_matchable(),
            Ref::new("SelectStatementSegment").to_matchable(),
            Ref::new("WithCheckOptionSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    // RenameTableStatementSegment.
    mysql.add([(
        "RenameTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::RenameTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("RENAME").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
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

    // ResetMasterStatementSegment.
    mysql.add([(
        "ResetMasterStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::ResetMasterStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("RESET").to_matchable(),
                Ref::keyword("MASTER").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("TO").to_matchable(),
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

    // PurgeBinaryLogsStatementSegment.
    mysql.add([(
        "PurgeBinaryLogsStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::PurgeBinaryLogsStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("PURGE").to_matchable(),
                one_of(vec![
                    Ref::keyword("BINARY").to_matchable(),
                    Ref::keyword("MASTER").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("LOGS").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("BEFORE").to_matchable(),
                        Ref::new("DateTimeLiteralGrammar").to_matchable(),
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

    // HelpStatementSegment.
    mysql.add([(
        "HelpStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::HelpStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("HELP").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CheckTableStatementSegment.
    mysql.add([(
        "CheckTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CheckTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CHECK").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                    .to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("UPGRADE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("QUICK").to_matchable(),
                    Ref::keyword("FAST").to_matchable(),
                    Ref::keyword("MEDIUM").to_matchable(),
                    Ref::keyword("EXTENDED").to_matchable(),
                    Ref::keyword("CHANGED").to_matchable(),
                ])
                .config(|this| this.min_times = 1)
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ChecksumTableStatementSegment.
    mysql.add([(
        "ChecksumTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::ChecksumTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CHECKSUM").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                    .to_matchable(),
                one_of(vec![
                    Ref::keyword("QUICK").to_matchable(),
                    Ref::keyword("EXTENDED").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // AnalyzeTableStatementSegment.
    mysql.add([(
        "AnalyzeTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AnalyzeTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ANALYZE").to_matchable(),
                one_of(vec![
                    Ref::keyword("NO_WRITE_TO_BINLOG").to_matchable(),
                    Ref::keyword("LOCAL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                one_of(vec![
                    Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::keyword("UPDATE").to_matchable(),
                        Ref::keyword("HISTOGRAM").to_matchable(),
                        Ref::keyword("ON").to_matchable(),
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::keyword("BUCKETS").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::keyword("DROP").to_matchable(),
                        Ref::keyword("HISTOGRAM").to_matchable(),
                        Ref::keyword("ON").to_matchable(),
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
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

    // RepairTableStatementSegment.
    mysql.add([(
        "RepairTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::RepairTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("REPAIR").to_matchable(),
                one_of(vec![
                    Ref::keyword("NO_WRITE_TO_BINLOG").to_matchable(),
                    Ref::keyword("LOCAL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                    .to_matchable(),
                AnyNumberOf::new(vec![
                    Ref::keyword("QUICK").to_matchable(),
                    Ref::keyword("EXTENDED").to_matchable(),
                    Ref::keyword("USE_FRM").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // OptimizeTableStatementSegment.
    mysql.add([(
        "OptimizeTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OptimizeTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("OPTIMIZE").to_matchable(),
                one_of(vec![
                    Ref::keyword("NO_WRITE_TO_BINLOG").to_matchable(),
                    Ref::keyword("LOCAL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // UpdateStatementSegment.
    mysql.replace_grammar(
        "UpdateStatementSegment",
        Sequence::new(vec![
            Ref::keyword("UPDATE").to_matchable(),
            Ref::keyword("LOW_PRIORITY").optional().to_matchable(),
            Ref::keyword("IGNORE").optional().to_matchable(),
            Delimited::new(vec![
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("FromExpressionSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("SetClauseListSegment").to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("OrderByClauseSegment").optional().to_matchable(),
            Ref::new("LimitClauseSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    // FlushStatementSegment.
    mysql.add([(
        "FlushStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::FlushStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("FLUSH").to_matchable(),
                one_of(vec![
                    Ref::keyword("NO_WRITE_TO_BINLOG").to_matchable(),
                    Ref::keyword("LOCAL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                one_of(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("BINARY").to_matchable(),
                            Ref::keyword("LOGS").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ENGINE").to_matchable(),
                            Ref::keyword("LOGS").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ERROR").to_matchable(),
                            Ref::keyword("LOGS").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("GENERAL").to_matchable(),
                            Ref::keyword("LOGS").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("HOSTS").to_matchable(),
                        Ref::keyword("LOGS").to_matchable(),
                        Ref::keyword("PRIVILEGES").to_matchable(),
                        Ref::keyword("OPTIMIZER_COSTS").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("RELAY").to_matchable(),
                            Ref::keyword("LOGS").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("FOR").to_matchable(),
                                Ref::keyword("CHANNEL").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SLOW").to_matchable(),
                            Ref::keyword("LOGS").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("STATUS").to_matchable(),
                        Ref::keyword("USER_RESOURCES").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TABLES").to_matchable(),
                        Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                            .config(|this| {
                                this.optional();
                                this.base.terminators = vec![Ref::keyword("WITH").to_matchable()];
                            })
                            .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Ref::keyword("READ").to_matchable(),
                            Ref::keyword("LOCK").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TABLES").to_matchable(),
                        Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                            .config(|this| {
                                this.base.terminators = vec![Ref::keyword("FOR").to_matchable()];
                            })
                            .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FOR").to_matchable(),
                            Ref::keyword("EXPORT").to_matchable(),
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
    )]);

    // LoadDataSegment.
    mysql.add([(
        "LoadDataSegment".into(),
        NodeMatcher::new(SyntaxKind::LoadDataStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("LOAD").to_matchable(),
                Ref::keyword("DATA").to_matchable(),
                one_of(vec![
                    Ref::keyword("LOW_PRIORITY").to_matchable(),
                    Ref::keyword("CONCURRENT").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("LOCAL").optional().to_matchable(),
                Ref::keyword("INFILE").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("REPLACE").to_matchable(),
                    Ref::keyword("IGNORE").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("INTO").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("SelectPartitionClauseSegment")
                    .optional()
                    .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CHARACTER").to_matchable(),
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FIELDS").to_matchable(),
                        Ref::keyword("COLUMNS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TERMINATED").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OPTIONALLY").optional().to_matchable(),
                        Ref::keyword("ENCLOSED").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ESCAPED").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LINES").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("STARTING").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TERMINATED").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("IGNORE").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("LINES").to_matchable(),
                        Ref::keyword("ROWS").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("Expression_B_Grammar").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ReplaceSegment.
    mysql.add([(
        "ReplaceSegment".into(),
        NodeMatcher::new(SyntaxKind::ReplaceStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("REPLACE").to_matchable(),
                one_of(vec![
                    Ref::keyword("LOW_PRIORITY").to_matchable(),
                    Ref::keyword("DELAYED").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("INTO").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("SelectPartitionClauseSegment")
                    .optional()
                    .to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("BracketedColumnReferenceListGrammar")
                            .optional()
                            .to_matchable(),
                        Ref::new("ValuesClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("SetClauseListSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("BracketedColumnReferenceListGrammar")
                            .optional()
                            .to_matchable(),
                        one_of(vec![
                            Ref::new("SelectableGrammar").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("TABLE").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
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

    // CreateTriggerStatementSegment.
    mysql.replace_grammar(
        "CreateTriggerStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("DefinerSegment").optional().to_matchable(),
            Ref::keyword("TRIGGER").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("TriggerReferenceSegment").to_matchable(),
            one_of(vec![
                Ref::keyword("BEFORE").to_matchable(),
                Ref::keyword("AFTER").to_matchable(),
            ])
            .to_matchable(),
            one_of(vec![
                Ref::keyword("INSERT").to_matchable(),
                Ref::keyword("UPDATE").to_matchable(),
                Ref::keyword("DELETE").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("ON").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("FOR").to_matchable(),
                Ref::keyword("EACH").to_matchable(),
                Ref::keyword("ROW").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("FOLLOWS").to_matchable(),
                    Ref::keyword("PRECEDES").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            one_of(vec![
                Ref::new("StatementSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("BEGIN").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::keyword("END").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // DropTriggerStatementSegment.
    mysql.replace_grammar(
        "DropTriggerStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("TRIGGER").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("TriggerReferenceSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // ColumnReferenceSegment - add JSON path operators.
    // Base is a delimited list of identifiers (ANSI), plus optional JSON path.
    let base_col_ref = Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
        .config(|this| this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar")))
        .to_matchable();
    mysql.replace_grammar(
        "ColumnReferenceSegment",
        one_of(vec![
            Sequence::new(vec![
                base_col_ref.clone(),
                one_of(vec![
                    Ref::new("ColumnPathOperatorSegment").to_matchable(),
                    Ref::new("InlinePathOperatorSegment").to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::new("DoubleQuotedJSONPath").to_matchable(),
                    Ref::new("SingleQuotedJSONPath").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            base_col_ref,
        ])
        .to_matchable(),
    );

    // CreateDatabaseStatementSegment.
    mysql.replace_grammar(
        "CreateDatabaseStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            one_of(vec![
                Ref::keyword("DATABASE").to_matchable(),
                Ref::keyword("SCHEMA").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("DatabaseReferenceSegment").to_matchable(),
            AnyNumberOf::new(vec![Ref::new("CreateOptionSegment").to_matchable()]).to_matchable(),
        ])
        .to_matchable(),
    );

    // CreateOptionSegment.
    mysql.add([(
        "CreateOptionSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateOptionSegment, |_| {
            Sequence::new(vec![
                Ref::keyword("DEFAULT").optional().to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CHARACTER").to_matchable(),
                        Ref::keyword("SET").to_matchable(),
                        Ref::new("EqualsSegment").optional().to_matchable(),
                        Ref::new("NakedIdentifierSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("COLLATE").to_matchable(),
                        Ref::new("EqualsSegment").optional().to_matchable(),
                        Ref::new("NakedIdentifierSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ENCRYPTION").to_matchable(),
                        Ref::new("EqualsSegment").optional().to_matchable(),
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

    // AlterDatabaseStatementSegment.
    mysql.add([(
        "AlterDatabaseStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterDatabaseStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                one_of(vec![
                    Ref::keyword("DATABASE").to_matchable(),
                    Ref::keyword("SCHEMA").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DatabaseReferenceSegment")
                    .optional()
                    .to_matchable(),
                AnyNumberOf::new(vec![Ref::new("AlterOptionSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // AlterOptionSegment.
    mysql.add([(
        "AlterOptionSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterOptionSegment, |_| {
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("DEFAULT").optional().to_matchable(),
                    Ref::keyword("CHARACTER").to_matchable(),
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DEFAULT").optional().to_matchable(),
                    Ref::keyword("COLLATE").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DEFAULT").optional().to_matchable(),
                    Ref::keyword("ENCRYPTION").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("READ").to_matchable(),
                    Ref::keyword("ONLY").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
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

    // GrantStatementSegment - MySQL GRANT with additional features.
    mysql.replace_grammar(
        "AccessStatementSegment",
        one_of(vec![
            // GRANT
            Sequence::new(vec![
                Ref::keyword("GRANT").to_matchable(),
                Delimited::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ALL").to_matchable(),
                            Ref::keyword("PRIVILEGES").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("SELECT").to_matchable(),
                                Ref::keyword("INSERT").to_matchable(),
                                Ref::keyword("UPDATE").to_matchable(),
                                Ref::keyword("DELETE").to_matchable(),
                                Ref::keyword("CREATE").to_matchable(),
                                Ref::keyword("DROP").to_matchable(),
                                Ref::keyword("ALTER").to_matchable(),
                                Ref::keyword("INDEX").to_matchable(),
                                Ref::keyword("EXECUTE").to_matchable(),
                                Ref::keyword("REFERENCES").to_matchable(),
                                Ref::keyword("RELOAD").to_matchable(),
                                Ref::keyword("PROCESS").to_matchable(),
                                Ref::keyword("SUPER").to_matchable(),
                                Ref::keyword("USAGE").to_matchable(),
                                Ref::keyword("TRIGGER").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("CREATE").to_matchable(),
                                    Ref::keyword("VIEW").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("SHOW").to_matchable(),
                                    Ref::keyword("VIEW").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
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
                Ref::keyword("ON").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("TABLE").to_matchable(),
                            Ref::keyword("FUNCTION").to_matchable(),
                            Ref::keyword("PROCEDURE").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("TO").to_matchable(),
                Delimited::new(vec![Ref::new("RoleReferenceSegment").to_matchable()])
                    .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("GRANT").to_matchable(),
                    Ref::keyword("OPTION").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            // REVOKE
            Sequence::new(vec![
                Ref::keyword("REVOKE").to_matchable(),
                Delimited::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ALL").to_matchable(),
                            Ref::keyword("PRIVILEGES").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("SELECT").to_matchable(),
                        Ref::keyword("INSERT").to_matchable(),
                        Ref::keyword("UPDATE").to_matchable(),
                        Ref::keyword("DELETE").to_matchable(),
                        Ref::keyword("USAGE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("ON").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("TABLE").to_matchable(),
                            Ref::keyword("FUNCTION").to_matchable(),
                            Ref::keyword("PROCEDURE").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("FROM").to_matchable(),
                Delimited::new(vec![Ref::new("RoleReferenceSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // ValuesStatementSegment - MySQL VALUES statement.
    mysql.add([(
        "ValuesStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::ValuesClause, |_| {
            Sequence::new(vec![
                Ref::keyword("VALUES").to_matchable(),
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("ROW").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LIMIT").to_matchable(),
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

    // ============================================================
    // Select clause/statement overrides
    // ============================================================

    // Add INTO to SelectClauseTerminatorGrammar so SELECT clause stops before INTO.
    let select_clause_term = mysql.grammar("SelectClauseTerminatorGrammar");
    mysql.replace_grammar(
        "SelectClauseTerminatorGrammar",
        select_clause_term.copy(
            Some(vec![Ref::keyword("INTO").to_matchable()]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    // UnorderedSelectStatementSegment - add INTO, FOR, index hint, partition clauses.
    mysql.replace_grammar(
        "UnorderedSelectStatementSegment",
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::new("IntoClauseSegment").optional().to_matchable(),
            Ref::new("FromClauseSegment").optional().to_matchable(),
            Ref::new("SelectPartitionClauseSegment")
                .optional()
                .to_matchable(),
            Ref::new("IndexHintClauseSegment").optional().to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("GroupByClauseSegment").optional().to_matchable(),
            Ref::new("HavingClauseSegment").optional().to_matchable(),
            Ref::new("OverlapsClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
            Ref::new("ForClauseSegment").optional().to_matchable(),
        ])
        .terminators(vec![
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
            Ref::new("OrderByClauseSegment").to_matchable(),
            Ref::new("LimitClauseSegment").to_matchable(),
            Ref::new("IntoClauseSegment").to_matchable(),
            Ref::new("ForClauseSegment").to_matchable(),
            Ref::new("IndexHintClauseSegment").to_matchable(),
            Ref::new("SelectPartitionClauseSegment").to_matchable(),
            Ref::new("UpsertClauseListSegment").to_matchable(),
        ])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable(),
    );

    // SelectStatementSegment - add ORDER BY, LIMIT, named window, INTO, FOR.
    mysql.replace_grammar(
        "SelectStatementSegment",
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::new("IntoClauseSegment").optional().to_matchable(),
            Ref::new("FromClauseSegment").optional().to_matchable(),
            Ref::new("SelectPartitionClauseSegment")
                .optional()
                .to_matchable(),
            Ref::new("IndexHintClauseSegment").optional().to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("GroupByClauseSegment").optional().to_matchable(),
            Ref::new("HavingClauseSegment").optional().to_matchable(),
            Ref::new("OverlapsClauseSegment").optional().to_matchable(),
            Ref::new("OrderByClauseSegment").optional().to_matchable(),
            Ref::new("LimitClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
            Ref::new("IntoClauseSegment").optional().to_matchable(),
            Ref::new("ForClauseSegment").optional().to_matchable(),
        ])
        .terminators(vec![
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
            Ref::new("UpsertClauseListSegment").to_matchable(),
            Ref::new("WithCheckOptionSegment").to_matchable(),
        ])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable(),
    );

    // ============================================================
    // StatementSegment - override to add MySQL-specific statements
    // ============================================================

    mysql.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec![
                Ref::new("DelimiterStatement").to_matchable(),
                Ref::new("CreateProcedureStatementSegment").to_matchable(),
                Ref::new("DeclareStatement").to_matchable(),
                Ref::new("SetAssignmentStatementSegment").to_matchable(),
                Ref::new("IfExpressionStatement").to_matchable(),
                Ref::new("WhileStatementSegment").to_matchable(),
                Ref::new("IterateStatementSegment").to_matchable(),
                Ref::new("RepeatStatementSegment").to_matchable(),
                Ref::new("LoopStatementSegment").to_matchable(),
                Ref::new("CallStoredProcedureSegment").to_matchable(),
                Ref::new("PrepareSegment").to_matchable(),
                Ref::new("ExecuteSegment").to_matchable(),
                Ref::new("DeallocateSegment").to_matchable(),
                Ref::new("GetDiagnosticsSegment").to_matchable(),
                Ref::new("ResignalSegment").to_matchable(),
                Ref::new("CursorOpenCloseSegment").to_matchable(),
                Ref::new("CursorFetchSegment").to_matchable(),
                Ref::new("DropProcedureStatementSegment").to_matchable(),
                Ref::new("AlterTableStatementSegment").to_matchable(),
                Ref::new("AlterViewStatementSegment").to_matchable(),
                Ref::new("CreateViewStatementSegment").to_matchable(),
                Ref::new("RenameTableStatementSegment").to_matchable(),
                Ref::new("ResetMasterStatementSegment").to_matchable(),
                Ref::new("PurgeBinaryLogsStatementSegment").to_matchable(),
                Ref::new("HelpStatementSegment").to_matchable(),
                Ref::new("CheckTableStatementSegment").to_matchable(),
                Ref::new("ChecksumTableStatementSegment").to_matchable(),
                Ref::new("AnalyzeTableStatementSegment").to_matchable(),
                Ref::new("RepairTableStatementSegment").to_matchable(),
                Ref::new("OptimizeTableStatementSegment").to_matchable(),
                Ref::new("UpsertClauseListSegment").to_matchable(),
                Ref::new("InsertRowAliasSegment").to_matchable(),
                Ref::new("FlushStatementSegment").to_matchable(),
                Ref::new("LoadDataSegment").to_matchable(),
                Ref::new("ReplaceSegment").to_matchable(),
                Ref::new("AlterDatabaseStatementSegment").to_matchable(),
            ]),
            None,
            None,
            Some(vec![
                Ref::new("CreateSchemaStatementSegment").to_matchable(),
            ]),
            vec![],
            false,
        ),
    );

    mysql
}
