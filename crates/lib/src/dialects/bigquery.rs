use std::rc::Rc;

use super::ansi::{self, ansi_dialect, Node, NodeTrait};
use super::bigquery_keywords::{BIGQUERY_RESERVED_KEYWORDS, BIGQUERY_UNRESERVED_KEYWORDS};
use crate::core::dialects::base::Dialect;
use crate::core::parser::grammar::anyof::{one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::Ref;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::{RegexLexer, StringLexer};
use crate::core::parser::matchable::Matchable;
use crate::core::parser::parsers::{StringParser, TypedParser};
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, SymbolSegment, SymbolSegmentNewArgs,
};
use crate::core::parser::segments::meta::MetaSegment;
use crate::core::parser::types::ParseMode;
use crate::helpers::{Config, ToMatchable};
use crate::vec_of_erased;

pub fn bigquery_dialect() -> Dialect {
    let mut dialect = ansi_dialect();
    dialect.name = "bigquery";

    dialect.insert_lexer_matchers(
        vec![
            Box::new(StringLexer::new(
                "right_arrow",
                "=>",
                &CodeSegment::create,
                CodeSegmentNewArgs::default(),
                None,
                None,
            )),
            Box::new(StringLexer::new(
                "question_mark",
                "?",
                &CodeSegment::create,
                CodeSegmentNewArgs::default(),
                None,
                None,
            )),
            Box::new(
                RegexLexer::new(
                    "at_sign_literal",
                    r#"@[a-zA-Z_][\w]*"#,
                    &CodeSegment::create,
                    CodeSegmentNewArgs::default(),
                    None,
                    None,
                )
                .unwrap(),
            ),
        ],
        "equals",
    );

    dialect.patch_lexer_matchers(vec![
        // Box::new(StringLexer::new(
        //     "single_quote",
        //     "([rR]?[bB]?|[bB]?[rR]?)?('''((?<!\\)(\\{2})*\\'|'{,2}(?!')|[^'])*(?<!\\)(\\{2})*'''|'((?<!\\)(\\{2})*\\'|[^'])*(?<!\\)(\\{2})*')",
        //     &CodeSegment::create,
        //     CodeSegmentNewArgs::default(),
        //     None,
        //     None,
        // )),
        // Box::new(StringLexer::new(
        //     "double_quote",
        //     r#"([rR]?[bB]?|[bB]?[rR]?)?(\"\"\"((?<!\\)(\\{2})*\\\"|\"{,2}(?!\")|[^\"])*(?<!\\)(\\{2})*\"\"\"|"((?<!\\)(\\{2})*\\"|[^"])*(?<!\\)(\\{2})*")"#,
        //     &CodeSegment::create,
        //     CodeSegmentNewArgs::default(),
        //     None,
        //     None,
        // )),
    ]);

    dialect.add([
        (
            "DoubleQuotedLiteralSegment".into(),
            TypedParser::new(
                "double_quote",
                |_| unimplemented!(),
                "quoted_literal".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "StartAngleBracketSegment".into(),
            StringParser::new(
                "<",
                |segment| {
                    SymbolSegment::create(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
                        SymbolSegmentNewArgs { r#type: "remove me" },
                    )
                },
                "start_angle_bracket".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EndAngleBracketSegment".into(),
            StringParser::new(
                ">",
                |segment| {
                    SymbolSegment::create(
                        &segment.get_raw().unwrap(),
                        &segment.get_position_marker().unwrap(),
                        SymbolSegmentNewArgs { r#type: "remove me" },
                    )
                },
                "end_angle_bracket".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
    ]);

    // Set Keywords
    dialect.sets_mut("unreserved_keywords").clear();
    dialect.update_keywords_set_from_multiline_string(
        "unreserved_keywords",
        BIGQUERY_UNRESERVED_KEYWORDS,
    );

    dialect.sets_mut("reserved_keywords").clear();
    dialect
        .update_keywords_set_from_multiline_string("reserved_keywords", BIGQUERY_RESERVED_KEYWORDS);

    // Add additional datetime units
    // https://cloud.google.com/bigquery/docs/reference/standard-sql/timestamp_functions#extract
    dialect.sets_mut("datetime_units").extend([
        "MICROSECOND",
        "MILLISECOND",
        "SECOND",
        "MINUTE",
        "HOUR",
        "DAY",
        "DAYOFWEEK",
        "DAYOFYEAR",
        "WEEK",
        "ISOWEEK",
        "MONTH",
        "QUARTER",
        "YEAR",
        "ISOYEAR",
    ]);

    // Add additional datetime units only recognised in some functions (e.g.
    // extract)
    dialect.sets_mut("extended_datetime_units").extend(["DATE", "DATETIME", "TIME"]);

    dialect.sets_mut("date_part_function_name").clear();
    dialect.sets_mut("date_part_function_name").extend([
        "DATE_DIFF",
        "DATE_TRUNC",
        "DATETIME_DIFF",
        "DATETIME_TRUNC",
        "EXTRACT",
        "LAST_DAY",
        "TIME_DIFF",
        "TIME_TRUNC",
        "TIMESTAMP_DIFF",
        "TIMESTAMP_TRUNC",
    ]);

    // Set value table functions
    dialect.sets_mut("value_table_functions").extend(["UNNEST"]);

    // Set angle bracket pairs
    dialect.bracket_sets_mut("angle_bracket_pairs").extend([(
        "angle".to_string(),
        "StartAngleBracketSegment".to_string(),
        "EndAngleBracketSegment".to_string(),
        false,
    )]);

    macro_rules! add_segments {
        ($dialect:ident, $( $segment:ident ),*) => {
            $(
                $dialect.add([(
                    stringify!($segment).into(),
                    Node::<$segment>::new().to_matchable().into(),
                )]);
            )*
        }
    }

    add_segments!(
        dialect,
        ArrayTypeSegment,
        QualifyClauseSegment,
        SetOperatorSegment,
        SetExpressionSegment,
        SelectStatementSegment,
        UnorderedSelectStatementSegment,
        MultiStatementSegment,
        FileSegment,
        StatementSegment,
        AssertStatementSegment,
        ForInStatementsSegment,
        ForInStatementSegment,
        RepeatStatementsSegment,
        RepeatStatementSegment,
        IfStatementsSegment,
        IfStatementSegment,
        LoopStatementsSegment,
        LoopStatementSegment,
        WhileStatementsSegment,
        WhileStatementSegment,
        SelectClauseModifierSegment,
        IntervalExpressionSegment,
        // ???
        DatePartWeekSegment,
        FunctionNameSegment // FunctionSegment
    );

    dialect.expand();
    dialect
}

pub struct ArrayTypeSegment;

impl NodeTrait for ArrayTypeSegment {
    const TYPE: &'static str = "array_type";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ARRAY"),
            Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")]).config(|this| {
                this.bracket_type = "angle";
                this.bracket_pairs_set = "angle_bracket_pairs";
            })
        ])
        .to_matchable()
    }
}

pub struct QualifyClauseSegment;

impl NodeTrait for QualifyClauseSegment {
    const TYPE: &'static str = "qualify_clause";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("QUALIFY"),
            MetaSegment::indent(),
            optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }
}

pub struct SetOperatorSegment;

impl NodeTrait for SetOperatorSegment {
    const TYPE: &'static str = "set_operator";

    fn match_grammar() -> Rc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("UNION"),
                one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")]),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("INTERSECT"), Ref::keyword("DISTINCT")]),
            Sequence::new(vec_of_erased![Ref::keyword("EXCEPT"), Ref::keyword("DISTINCT")]),
        ])
        .to_matchable()
    }
}

pub struct SetExpressionSegment;

impl NodeTrait for SetExpressionSegment {
    const TYPE: &'static str = "set_expression";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("NonSetSelectableGrammar"),
                Bracketed::new(vec_of_erased![Ref::new("SetExpressionSegment")]),
            ]),
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("SetOperatorSegment"),
                one_of(vec_of_erased![
                    Ref::new("NonSetSelectableGrammar"),
                    Bracketed::new(vec_of_erased![Ref::new("SetExpressionSegment")]),
                ]),
            ])])
            .config(|this| this.min_times = 1),
            Ref::new("OrderByClauseSegment").optional(),
            Ref::new("LimitClauseSegment").optional(),
            Ref::new("NamedWindowSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct SelectStatementSegment;

impl NodeTrait for SelectStatementSegment {
    const TYPE: &'static str = "select_statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        ansi::SelectStatementSegment::match_grammar().copy(
            vec![].into(),
            None,
            Ref::new("OrderByClauseSegment").to_matchable().into(),
            None,
            Vec::new(),
            false,
        )
    }
}

pub struct UnorderedSelectStatementSegment;

impl NodeTrait for UnorderedSelectStatementSegment {
    const TYPE: &'static str = "unordered_select_statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        ansi::UnorderedSelectStatementSegment::match_grammar().copy(
            Some(vec![Ref::new("QualifyClauseSegment").optional().to_matchable()]),
            None,
            Some(Ref::new("OverlapsClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        )
    }
}

pub struct MultiStatementSegment;

impl NodeTrait for MultiStatementSegment {
    const TYPE: &'static str = "multi_statement_segment";

    fn match_grammar() -> Rc<dyn Matchable> {
        one_of(vec_of_erased![
            Ref::new("ForInStatementSegment"),
            Ref::new("RepeatStatementSegment"),
            Ref::new("WhileStatementSegment"),
            Ref::new("LoopStatementSegment"),
            Ref::new("IfStatementSegment"),
            Ref::new("CreateProcedureStatementSegment"),
        ])
        .to_matchable()
    }
}

pub struct FileSegment;

impl NodeTrait for FileSegment {
    const TYPE: &'static str = "file_segment";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("MultiStatementSegment"),
                Ref::new("StatementSegment")
            ])]),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("DelimiterGrammar"),
                one_of(vec_of_erased![
                    Ref::new("MultiStatementSegment"),
                    Ref::new("StatementSegment")
                ])
            ]),
            Ref::new("DelimiterGrammar").optional()
        ])
        .to_matchable()
    }
}

pub struct StatementSegment;

impl NodeTrait for StatementSegment {
    const TYPE: &'static str = "statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        ansi::StatementSegment::match_grammar().copy(
            Some(vec_of_erased![
                // Ref::new("DeclareStatementSegment"),
                // Ref::new("SetStatementSegment"),
                // Ref::new("ExportStatementSegment"),
                // Ref::new("CreateExternalTableStatementSegment"),
                Ref::new("AssertStatementSegment"),
                // Ref::new("CallStatementSegment"),
                // Ref::new("ReturnStatementSegment"),
                // Ref::new("BreakStatementSegment"),
                // Ref::new("LeaveStatementSegment"),
                // Ref::new("ContinueStatementSegment"),
                // Ref::new("RaiseStatementSegment"),
                // Ref::new("AlterViewStatementSegment"),
                // Ref::new("CreateMaterializedViewStatementSegment"),
                // Ref::new("AlterMaterializedViewStatementSegment"),
                // Ref::new("DropMaterializedViewStatementSegment"),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        )
    }
}

pub struct AssertStatementSegment;

impl NodeTrait for AssertStatementSegment {
    const TYPE: &'static str = "assert_statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ASSERT"),
            Ref::new("ExpressionSegment"),
            Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::new("QuotedLiteralSegment")])
                .config(|this| this.optional())
        ])
        .to_matchable()
    }
}

pub struct ForInStatementsSegment;

impl NodeTrait for ForInStatementsSegment {
    const TYPE: &'static str = "for_in_statements";

    fn match_grammar() -> Rc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::new("StatementSegment"), Ref::new("MultiStatementSegment")]),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("END"),
                Ref::keyword("FOR")
            ])];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct ForInStatementSegment;

impl NodeTrait for ForInStatementSegment {
    const TYPE: &'static str = "for_in_statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("FOR"),
            Ref::new("SingleIdentifierGrammar"),
            Ref::keyword("IN"),
            MetaSegment::indent(),
            Ref::new("SelectableGrammar"),
            MetaSegment::dedent(),
            Ref::keyword("DO"),
            MetaSegment::indent(),
            Ref::new("ForInStatementsSegment"),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("FOR")
        ])
        .to_matchable()
    }
}

pub struct RepeatStatementsSegment;

impl NodeTrait for RepeatStatementsSegment {
    const TYPE: &'static str = "repeat_statements";

    fn match_grammar() -> Rc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            one_of(
                vec_of_erased![Ref::new("StatementSegment"), Ref::new("MultiStatementSegment"),]
            ),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Ref::keyword("UNTIL")];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct RepeatStatementSegment;

impl NodeTrait for RepeatStatementSegment {
    const TYPE: &'static str = "repeat_statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("REPEAT"),
            MetaSegment::indent(),
            Ref::new("RepeatStatementsSegment"),
            Ref::keyword("UNTIL"),
            Ref::new("ExpressionSegment"),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("REPEAT")
        ])
        .to_matchable()
    }
}

pub struct IfStatementsSegment;

impl NodeTrait for IfStatementsSegment {
    const TYPE: &'static str = "if_statements";

    fn match_grammar() -> Rc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::new("StatementSegment"), Ref::new("MultiStatementSegment")]),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![
                Ref::keyword("ELSE"),
                Ref::keyword("ELSEIF"),
                Sequence::new(vec_of_erased![Ref::keyword("END"), Ref::keyword("IF")])
            ];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct IfStatementSegment;

impl NodeTrait for IfStatementSegment {
    const TYPE: &'static str = "if_statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("IF"),
            Ref::new("ExpressionSegment"),
            Ref::keyword("THEN"),
            MetaSegment::indent(),
            Ref::new("IfStatementsSegment"),
            MetaSegment::dedent(),
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("ELSEIF"),
                Ref::new("ExpressionSegment"),
                Ref::keyword("THEN"),
                MetaSegment::indent(),
                Ref::new("IfStatementsSegment"),
                MetaSegment::dedent()
            ])]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ELSE"),
                MetaSegment::indent(),
                Ref::new("IfStatementsSegment"),
                MetaSegment::dedent()
            ])
            .config(|this| this.optional()),
            Ref::keyword("END"),
            Ref::keyword("IF")
        ])
        .to_matchable()
    }
}

pub struct LoopStatementsSegment;

impl NodeTrait for LoopStatementsSegment {
    const TYPE: &'static str = "loop_statements";

    fn match_grammar() -> Rc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::new("StatementSegment"), Ref::new("MultiStatementSegment")]),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("END"),
                Ref::keyword("LOOP")
            ])];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct LoopStatementSegment;

impl NodeTrait for LoopStatementSegment {
    const TYPE: &'static str = "loop_statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("LOOP"),
            MetaSegment::indent(),
            Ref::new("LoopStatementsSegment"),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("LOOP")
        ])
        .to_matchable()
    }
}

pub struct WhileStatementsSegment;

impl NodeTrait for WhileStatementsSegment {
    const TYPE: &'static str = "while_statements";

    fn match_grammar() -> Rc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            Ref::new("StatementSegment"),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("END"),
                Ref::keyword("WHILE")
            ])];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct WhileStatementSegment;

impl NodeTrait for WhileStatementSegment {
    const TYPE: &'static str = "while_statement";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WHILE"),
            Ref::new("ExpressionSegment"),
            Ref::keyword("DO"),
            MetaSegment::indent(),
            Ref::new("WhileStatementsSegment"),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("WHILE")
        ])
        .to_matchable()
    }
}

pub struct SelectClauseModifierSegment;

impl NodeTrait for SelectClauseModifierSegment {
    const TYPE: &'static str = "select_clause_modifier";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                one_of(vec_of_erased![Ref::keyword("STRUCT"), Ref::keyword("VALUE")])
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
    }
}

pub struct IntervalExpressionSegment;

impl NodeTrait for IntervalExpressionSegment {
    const TYPE: &'static str = "interval_expression";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("INTERVAL"),
            Ref::new("ExpressionSegment"),
            one_of(vec_of_erased![
                Ref::new("QuotedLiteralSegment"),
                Ref::new("DatetimeUnitSegment"),
                Sequence::new(vec_of_erased![
                    Ref::new("DatetimeUnitSegment"),
                    Ref::keyword("TO"),
                    Ref::new("DatetimeUnitSegment")
                ])
            ])
        ])
        .to_matchable()
    }
}

pub struct ExtractFunctionNameSegment;

impl NodeTrait for ExtractFunctionNameSegment {
    const TYPE: &'static str = "function_name";

    fn match_grammar() -> Rc<dyn Matchable> {
        unimplemented!()
    }
}

pub struct ArrayFunctionNameSegment;

impl NodeTrait for ArrayFunctionNameSegment {
    const TYPE: &'static str = "function_name";

    fn match_grammar() -> Rc<dyn Matchable> {
        unimplemented!()
    }
}

pub struct DatePartWeekSegment;

impl NodeTrait for DatePartWeekSegment {
    const TYPE: &'static str = "date_part_week";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WEEK"),
            Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                Ref::keyword("SUNDAY"),
                Ref::keyword("MONDAY"),
                Ref::keyword("TUESDAY"),
                Ref::keyword("WEDNESDAY"),
                Ref::keyword("THURSDAY"),
                Ref::keyword("FRIDAY"),
                Ref::keyword("SATURDAY")
            ])])
        ])
        .to_matchable()
    }
}

pub struct NormalizeFunctionNameSegment;

pub struct FunctionNameSegment;

impl NodeTrait for FunctionNameSegment {
    const TYPE: &'static str = "function_name";

    fn match_grammar() -> Rc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            // AnyNumberOf to handle project names, schemas, or the SAFE keyword
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("SAFE"),
                        Ref::new("SingleIdentifierGrammar")
                    ]),
                    Ref::new("DotSegment"),
                ])
                .terminators(vec_of_erased![Ref::new("BracketedSegment")])
            ]),
            // Base function name
            one_of(vec_of_erased![
                Ref::new("FunctionNameIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment")
            ])
            .config(|this| this.terminators = vec_of_erased![Ref::new("BracketedSegment")]),
        ])
        .allow_gaps(true)
        .to_matchable()
    }
}

pub struct FunctionSegment;

impl NodeTrait for FunctionSegment {
    const TYPE: &'static str = "function";

    fn match_grammar() -> Rc<dyn Matchable> {
        unimplemented!()
    }
}

pub struct FunctionDefinitionGrammar;

#[cfg(test)]
mod tests {
    use expect_test::expect_file;
    use itertools::Itertools;
    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

    use crate::core::config::{FluffConfig, Value};
    use crate::core::linter::linter::Linter;
    use crate::core::parser::segments::base::ErasedSegment;
    use crate::helpers;

    fn parse_sql(sql: &str) -> ErasedSegment {
        let linter = Linter::new(
            FluffConfig::new(
                [(
                    "core".into(),
                    Value::Map([("dialect".into(), Value::String("bigquery".into()))].into()),
                )]
                .into(),
                None,
                None,
            ),
            None,
            None,
        );
        let parsed = linter.parse_string(sql.into(), None, None, None).unwrap();
        parsed.tree.unwrap()
    }

    #[test]
    fn base_parse_struct() {
        let files =
            glob::glob("test/fixtures/dialects/bigquery/*.sql").unwrap().flatten().collect_vec();

        files.par_iter().for_each(|file| {
            let _panic = helpers::enter_panic(file.display().to_string());

            let yaml = file.with_extension("yml");
            let yaml = std::path::absolute(yaml).unwrap();

            let actual = {
                let sql = std::fs::read_to_string(file).unwrap();
                let tree = parse_sql(&sql);
                let tree = tree.to_serialised(true, true, false);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml].assert_eq(&actual);
        });
    }
}
