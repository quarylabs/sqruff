use std::sync::Arc;

use crate::core::dialects::base::Dialect;
use crate::core::dialects::init::DialectKind;
use crate::core::parser::grammar::anyof::{one_of, AnyNumberOf};
use crate::core::parser::grammar::base::{Nothing, Ref};
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::parsers::TypedParser;
use crate::dialects::ansi::NodeMatcher;
use crate::dialects::trino_keywords::{RESERVED_WORDS, UNRESERVED_WORDS};
use crate::dialects::{ansi, SyntaxKind};
use crate::helpers::{Config, ToMatchable};
use crate::vec_of_erased;

pub fn dialect() -> Dialect {
    let ansi_dialect = ansi::dialect();
    let mut trino_dialect = ansi::raw_dialect();
    trino_dialect.name = DialectKind::Trino;

    // Set the bare functions: https://trino.io/docs/current/functions/datetime.html
    trino_dialect.sets_mut("bare_functions").clear();
    trino_dialect.sets_mut("bare_functions").extend([
        "current_date",
        "current_time",
        "current_timestamp",
        "localtime",
        "localtimestamp",
    ]);

    // Set the reserved keywords
    trino_dialect.sets_mut("unreserved_keywords").clear();
    trino_dialect.update_keywords_set_from_multiline_string("reserved_keywords", UNRESERVED_WORDS);

    trino_dialect.sets_mut("reserved_keywords").clear();
    trino_dialect.update_keywords_set_from_multiline_string("reserved_keywords", RESERVED_WORDS);

    trino_dialect.replace_grammar(
        "DateTimeLiteralGrammar",
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("DATE"),
                Ref::keyword("TIME"),
                Ref::keyword("TIMESTAMP")
            ]),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::Literal),
            Ref::new("IntervalExpressionSegment"),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "LikeGrammar",
        Sequence::new(vec_of_erased![Ref::keyword("LIKE")]).to_matchable(),
    );

    trino_dialect.replace_grammar("MLTableExpressionSegment", Nothing::new().to_matchable());

    trino_dialect.replace_grammar(
        "FromClauseTerminatorGrammar",
        one_of(vec_of_erased![
            Ref::keyword("WHERE"),
            Ref::keyword("LIMIT"),
            Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY")]),
            Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
            Ref::keyword("HAVING"),
            Ref::keyword("WINDOW"),
            Ref::new("SetOperatorSegment"),
            Ref::new("WithNoSchemaBindingClauseSegment"),
            Ref::new("WithDataClauseSegment"),
            Ref::keyword("FETCH"),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "OrderByClauseTerminators",
        one_of(vec_of_erased![
            Ref::keyword("LIMIT"),
            Ref::keyword("HAVING"),
            Ref::keyword("WINDOW"),
            Ref::new("FrameClauseUnitGrammar"),
            Ref::keyword("FETCH"),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "SelectClauseTerminatorGrammar",
        one_of(vec_of_erased![
            Ref::keyword("FROM"),
            Ref::keyword("WHERE"),
            Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
            Ref::keyword("LIMIT"),
            Ref::new("SetOperatorSegment"),
            Ref::keyword("FETCH"),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "WhereClauseTerminatorGrammar",
        one_of(vec_of_erased![
            Ref::keyword("LIMIT"),
            Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY")]),
            Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
            Ref::keyword("HAVING"),
            Ref::keyword("WINDOW"),
            Ref::keyword("FETCH"),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "HavingClauseTerminatorGrammar",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
            Ref::keyword("LIMIT"),
            Ref::keyword("WINDOW"),
            Ref::keyword("FETCH"),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "GroupByClauseTerminatorGrammar",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
            Ref::keyword("LIMIT"),
            Ref::keyword("HAVING"),
            Ref::keyword("WINDOW"),
            Ref::keyword("FETCH"),
        ])
        .to_matchable(),
    );

    // NOTE: This block was copy/pasted from dialect_ansi.py with these changes made
    //  - "PRIOR" keyword removed
    trino_dialect.replace_grammar(
        "Expression_A_Unary_Operator_Grammar",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("SignedSegmentGrammar"),
                Sequence::new(vec_of_erased![Ref::new("QualifiedNumericLiteralSegment").exclude(
                    Sequence::new(vec_of_erased![Ref::new("QualifiedNumericLiteralSegment")])
                )]),
            ]),
            Ref::new("TildeSegment"),
            Ref::new("NotOperatorGrammar"),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "PostFunctionGrammar",
        ansi_dialect.grammar("PostFunctionGrammar").copy(
            Some(vec_of_erased![Ref::new("WithinGroupClauseSegment"),]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    trino_dialect.replace_grammar(
        "FunctionContentsGrammar",
        AnyNumberOf::new(vec_of_erased!(
            Ref::new("ExpressionSegment"),
            // A cast-like function
            Sequence::new(vec_of_erased![
                Ref::new("ExpressionSegment"),
                Ref::keyword("AS"),
                Ref::new("DatatypeSegment"),
            ]),
            // Trim function
            Sequence::new(vec_of_erased![
                Ref::new("TrimParametersGrammar"),
                Ref::new("ExpressionSegment").optional().exclude(Ref::keyword("FROM")),
                Ref::keyword("FROM"),
                Ref::new("ExpressionSegment"),
            ]),
            // An extract-like or substring-like function
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("DatetimeUnitSegment"),
                    Ref::new("ExpressionSegment"),
                ]),
                Ref::keyword("FROM"),
                Ref::new("ExpressionSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("DISTINCT").optional(),
                one_of(vec_of_erased![
                    Ref::new("StarSegment"),
                    Ref::new("FunctionContentsExpressionGrammar"),
                ]),
            ]),
            Ref::new("OrderByClauseSegment"),
            // used by string_agg (postgres), group_concat (exasol), listagg (snowflake)
            // like a function call: POSITION ( 'QL' IN 'SQL')
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("ColumnReferenceSegment"),
                ]),
                Ref::keyword("IN"),
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("ColumnReferenceSegment"),
                ]),
            ]),
            Ref::new("IgnoreRespectNullsGrammar"),
            Ref::new("IndexColumnDefinitionSegment"),
            Ref::new("EmptyStructLiteralSegment"),
            Ref::new("ListaggOverflowClauseSegment"),
        ))
        .to_matchable(),
    );

    // An `OVERLAPS` clause like in `SELECT`.
    trino_dialect.add([(
        "OverlapsClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::OverlapsClause, Nothing::new().to_matchable())
            .to_matchable()
            .into(),
    )]);

    // A `SELECT` statement without any ORDER clauses or later.
    trino_dialect.add([(
        "UnorderedSelectStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::UnorderedSelectStatementSegment,
            Sequence::new(vec_of_erased![
                Ref::new("SelectClauseSegment"),
                Ref::new("FromClauseSegment").optional(),
                Ref::new("WhereClauseSegment").optional(),
                Ref::new("GroupByClauseSegment").optional(),
                Ref::new("HavingClauseSegment").optional(),
                Ref::new("NamedWindowSegment").optional(),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // A `VALUES` clause within in `WITH`, `SELECT`, `INSERT`.
    trino_dialect.add([(
        "ValuesClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::ValuesClause,
            Sequence::new(vec_of_erased![
                Ref::keyword("VALUES"),
                Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")]),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    //  An interval representing a span of time.
    trino_dialect.add([(
        "IntervalExpressionSegment".into(),
        NodeMatcher::new(
            SyntaxKind::IntervalExpression,
            Sequence::new(vec_of_erased![
                Ref::keyword("INTERVAL"),
                Ref::new("QuotedLiteralSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("YEAR"),
                    Ref::keyword("MONTH"),
                    Ref::keyword("DAY"),
                    Ref::keyword("HOUR"),
                    Ref::keyword("MINUTE"),
                    Ref::keyword("SECOND"),
                ]),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // A frame clause for window functions.
    let frame_extent = one_of(vec_of_erased![
        Sequence::new(vec_of_erased![Ref::keyword("CURRENT"), Ref::keyword("ROW"),]),
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("NumericLiteralSegment"),
                Ref::new("DateTimeLiteralGrammar"),
                Ref::keyword("UNBOUNDED"),
            ]),
            one_of(vec_of_erased![Ref::keyword("PRECEDING"), Ref::keyword("FOLLOWING"),]),
        ]),
    ]);
    trino_dialect.add([(
        "FrameClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::FrameClause,
            Sequence::new(vec_of_erased![
                Ref::new("FrameClauseUnitGrammar"),
                one_of(vec_of_erased![
                    frame_extent.clone(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("BETWEEN"),
                        frame_extent.clone(),
                        Ref::keyword("AND"),
                        frame_extent.clone(),
                    ]),
                ]),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // A set operator such as Union, Intersect or Except.
    trino_dialect.add([(
        "SetOperatorSegment".into(),
        NodeMatcher::new(
            SyntaxKind::SetOperator,
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNION"),
                    one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")])
                        .config(|c| c.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("INTERSECT"), Ref::keyword("EXCEPT")]),
                    Ref::keyword("ALL").optional(),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXCEPT"),
                    Bracketed::new(vec_of_erased![Ref::new("Anything")]),
                ])
            ])
            .config(|c| {
                c.exclude = Some(
                    Sequence::new(vec_of_erased![
                        Ref::keyword("EXCEPT"),
                        Bracketed::new(vec_of_erased![Ref::new("Anything")])
                    ])
                    .to_matchable(),
                )
            })
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // Overriding StatementSegment to allow for additional segment parsing.
    trino_dialect.replace_grammar(
        "StatementSegment",
        ansi_dialect.grammar("StatementSegment").copy(
            Some(vec_of_erased![Ref::new("AnalyzeStatementSegment")]),
            None,
            None,
            Some(vec_of_erased![Ref::new("TransactionStatementSegment")]),
            Vec::new(),
            false,
        ),
    );

    // An 'ANALYZE' statement as per docs https://trino.io/docs/current/sql/analyze.html
    trino_dialect.add([(
        "AnalyzeStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::AnalyzeStatement,
            Sequence::new(vec_of_erased![
                Ref::keyword("ANALYZE"),
                Ref::new("TableReferenceSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                        vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Ref::new("EqualsSegment"),
                            Ref::new("ExpressionSegment"),
                        ]
                    )])])
                ])
                .config(|c| c.optional()),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);
    //
    // // An WITHIN GROUP clause for window functions.
    trino_dialect.add([(
        "WithinGroupClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::WithingroupClause,
            Sequence::new(vec_of_erased![
                Ref::keyword("WITHIN"),
                Ref::keyword("GROUP"),
                Bracketed::new(vec_of_erased![Ref::new("OrderByClauseSegment").optional()]),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // On OVERFLOW clause of listagg function.
    // https://trino.io/docs/current/functions/aggregate.html#array_agg
    trino_dialect.add([(
        "ListaggOverflowClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::ListaggOverflowClauseSegment,
            Sequence::new(vec_of_erased![
                Ref::keyword("ON"),
                Ref::keyword("OVERFLOW"),
                one_of(vec_of_erased![
                    Ref::keyword("ERROR"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TRUNCATE"),
                        Ref::new("SingleQuotedIdentifierSegment").optional(),
                        one_of(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("WITHOUT"),])
                            .config(|c| c.optional()),
                        Ref::keyword("COUNT").optional(),
                    ]),
                ]),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    trino_dialect
}
