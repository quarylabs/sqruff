use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::base::{Nothing, Ref};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::vec_of_erased;

use crate::{ansi, tsql_keywords};

pub fn dialect() -> Dialect {
    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut dialect = ansi::raw_dialect();
    dialect.name = DialectKind::Tsql;

    // Set T-SQL specific keywords
    dialect.sets_mut("reserved_keywords").clear();
    dialect
        .sets_mut("reserved_keywords")
        .extend(tsql_keywords::tsql_reserved_keywords());
    dialect.sets_mut("unreserved_keywords").clear();
    dialect
        .sets_mut("unreserved_keywords")
        .extend(tsql_keywords::tsql_unreserved_keywords());

    // T-SQL specific operators
    dialect.sets_mut("operator_symbols").extend([
        "%=", "&=", "*=", "+=", "-=", "/=", "^=", "|=", "!<", "!>",
    ]);

    // T-SQL supports square brackets for identifiers
    dialect.patch_lexer_matchers(vec![
        Matcher::regex("tsql_square_bracket_identifier", r"\[[^\]]*\]", SyntaxKind::DoubleQuote),
    ]);

    // Add T-SQL specific bare functions
    dialect.sets_mut("bare_functions").extend([
        "CURRENT_TIMESTAMP",
        "CURRENT_USER",
        "SESSION_USER",
        "SYSTEM_USER",
        "USER",
    ]);

    // T-SQL datetime units
    dialect.sets_mut("datetime_units").extend([
        "YEAR", "YY", "YYYY",
        "QUARTER", "QQ", "Q",
        "MONTH", "MM", "M",
        "DAYOFYEAR", "DY", "Y",
        "DAY", "DD", "D",
        "WEEK", "WK", "WW",
        "WEEKDAY", "DW",
        "HOUR", "HH",
        "MINUTE", "MI", "N",
        "SECOND", "SS", "S",
        "MILLISECOND", "MS",
        "MICROSECOND", "MCS",
        "NANOSECOND", "NS",
    ]);

    // Add T-SQL specific date functions
    dialect.sets_mut("date_part_function_name").extend([
        "DATEADD",
        "DATEDIFF",
        "DATENAME",
        "DATEPART",
        "DAY",
        "MONTH",
        "YEAR",
        "GETDATE",
        "GETUTCDATE",
        "SYSDATETIME",
        "SYSUTCDATETIME",
        "SYSDATETIMEOFFSET",
    ]);

    // T-SQL specific value table functions
    dialect.sets_mut("value_table_functions").extend([
        "OPENROWSET",
        "OPENQUERY",
        "OPENDATASOURCE",
        "OPENXML",
    ]);

    // Add T-SQL specific grammar
    
    // TOP clause support
    dialect.add([
        (
            "TopClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClauseModifier, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "TopClauseGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TOP"),
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")])
                ]),
                Ref::keyword("PERCENT").optional(),
                Ref::keyword("WITH").optional(),
                Ref::keyword("TIES").optional()
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // DECLARE statement
    dialect.add([
        (
            "DeclareStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "DeclareStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DECLARE"),
                Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("DatatypeSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("EqualsSegment"),
                            Ref::new("ExpressionSegment")
                        ])
                        .config(|this| this.optional())
                    ])
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // SET statement for variables
    dialect.add([
        (
            "SetVariableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "SetVariableStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("EqualsSegment"),
                Ref::new("ExpressionSegment")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // PRINT statement
    dialect.add([
        (
            "PrintStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "PrintStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("PRINT"),
                Ref::new("ExpressionSegment")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // BEGIN...END blocks
    dialect.add([
        (
            "BeginEndBlockSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "BeginEndBlockGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("BEGIN"),
                AnyNumberOf::new(vec_of_erased![Ref::new("StatementSegment")])
                    .config(|this| this.min_times(1)),
                Ref::keyword("END")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // IF...ELSE statement
    dialect.add([
        (
            "IfStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "IfStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::new("ExpressionSegment"),
                Ref::new("StatementSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ELSE"),
                    Ref::new("StatementSegment")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // WHILE loop
    dialect.add([
        (
            "WhileStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "WhileStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WHILE"),
                Ref::new("ExpressionSegment"),
                Ref::new("StatementSegment")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // PIVOT and UNPIVOT support
    dialect.add([
        (
            "PivotUnpivotSegment".into(),
            NodeMatcher::new(SyntaxKind::TableExpression, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "PivotUnpivotGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("PIVOT"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("FunctionSegment"),
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("LiteralGrammar")])
                        ])
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNPIVOT"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")])
                        ])
                    ])
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // GO batch separator (special handling as it's not really a statement)
    dialect.add([
        (
            "BatchSeparatorSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "BatchSeparatorGrammar".into(),
            Ref::keyword("GO").to_matchable().into(),
        ),
    ]);

    // Add T-SQL specific statement types to the statement segment
    dialect.replace_grammar(
        "StatementSegment",
        one_of(vec_of_erased![
            Ref::new("DeclareStatementGrammar"),
            Ref::new("SetVariableStatementGrammar"),
            Ref::new("PrintStatementGrammar"),
            Ref::new("BeginEndBlockGrammar"),
            Ref::new("IfStatementGrammar"),
            Ref::new("WhileStatementGrammar"),
            Ref::new("BatchSeparatorGrammar"),
            Ref::new("UseStatementGrammar"),
            // Include base statement types that exist in ANSI
            Ref::new("SelectableGrammar"),
            Ref::new("InsertStatementSegment"),
            Ref::new("UpdateStatementSegment"),
            Ref::new("DeleteStatementSegment"),
            Ref::new("CreateTableStatementSegment"),
            Ref::new("AlterTableStatementSegment"),
            Ref::new("DropTableStatementSegment")
        ])
        .to_matchable()
        .into(),
    );

    // Update SELECT to include TOP clause
    dialect.replace_grammar(
        "SelectClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("SELECT"),
            Ref::new("SelectClauseModifierSegment").optional(),
            Ref::new("TopClauseGrammar").optional(),
            Ref::new("SelectClauseElementSegment"),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("CommaSegment"),
                    Ref::new("SelectClauseElementSegment")
                ])
            ])
        ])
        .to_matchable()
        .into(),
    );

    // Update table expressions to include PIVOT/UNPIVOT
    dialect.replace_grammar(
        "TableExpressionSegment",
        one_of(vec_of_erased![
            Ref::new("ValuesClauseSegment"),
            Ref::new("BareFunctionSegment"),
            Ref::new("FunctionSegment"),
            Ref::new("TableReferenceSegment"),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
            Sequence::new(vec_of_erased![
                Ref::new("TableReferenceSegment"),
                Ref::new("PivotUnpivotGrammar")
            ])
        ])
        .to_matchable()
        .into(),
    );

    // USE statement for changing database context
    dialect.add([
        (
            "UseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "UseStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("USE"),
                Ref::new("DatabaseReferenceSegment")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    dialect
}