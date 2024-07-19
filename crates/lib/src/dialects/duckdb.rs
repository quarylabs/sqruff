use std::sync::Arc;

use crate::core::dialects::base::Dialect;
use crate::core::dialects::init::DialectKind;
use crate::core::parser::grammar::anyof::one_of;
use crate::core::parser::grammar::base::Ref;
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::Matcher;
use crate::core::parser::parsers::StringParser;
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, Segment, SymbolSegment, SymbolSegmentNewArgs,
};
use crate::core::parser::segments::meta::MetaSegment;
use crate::helpers::{Config, ToMatchable};
use crate::vec_of_erased;

pub fn dialect() -> Dialect {
    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let ansi_dialect = super::ansi::raw_dialect();
    let postgres_dialect = super::postgres::dialect();
    let mut duckdb_dialect = postgres_dialect;
    duckdb_dialect.name = DialectKind::Duckdb;

    duckdb_dialect.add([
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
                Ref::new("SingleQuotedIdentifierSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DivideSegment".into(),
            one_of(vec_of_erased![
                StringParser::new(
                    "//",
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
                ),
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
            ])
            .to_matchable()
            .into(),
        ),
        (
            "UnionGrammar".into(),
            ansi_dialect
                .grammar("UnionGrammar")
                .copy(
                    Some(vec_of_erased![
                        Sequence::new(vec_of_erased![Ref::keyword("BY"), Ref::keyword("NAME")])
                            .config(|this| this.optional())
                    ]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
    ]);

    duckdb_dialect.insert_lexer_matchers(
        vec![Matcher::string("double_divide", "//", |slice, pos| {
            CodeSegment::create(
                slice,
                Some(pos),
                CodeSegmentNewArgs { code_type: "double_divide", ..CodeSegmentNewArgs::default() },
            )
        })],
        "divide",
    );

    duckdb_dialect.replace_grammar(
        "SelectClauseElementSegment",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("WildcardExpressionSegment"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("EXCLUDE"),
                        one_of(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment")
                            ])])
                        ])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("REPLACE"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("BaseExpressionElementGrammar"),
                                Ref::new("AliasExpressionSegment").optional()
                            ])
                        ])])
                    ])
                ])
                .config(|config| {
                    config.optional();
                })
            ]),
            Sequence::new(vec_of_erased![
                Ref::new("BaseExpressionElementGrammar"),
                Ref::new("AliasExpressionSegment").optional()
            ])
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "OrderByClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ORDER"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ALL"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment")
                ]),
                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")]).config(
                    |config| {
                        config.optional();
                    }
                ),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NULLS"),
                    one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST")])
                ])
                .config(|config| {
                    config.optional();
                })
            ])])
            .config(|config| {
                config.allow_trailing = true;
                config.terminators = vec_of_erased![Ref::new("OrderByClauseTerminators")];
            }),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "GroupByClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("GROUP"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Ref::keyword("ALL"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("NumericLiteralSegment"),
                Ref::new("ExpressionSegment")
            ])])
            .config(|config| {
                config.allow_trailing = true;
                config.terminators = vec_of_erased![Ref::new("GroupByClauseTerminatorGrammar")];
            }),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "ObjectLiteralElementSegment",
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            Ref::new("ColonSegment"),
            Ref::new("BaseExpressionElementGrammar")
        ])
        .to_matchable(),
    );

    duckdb_dialect
}

#[cfg(test)]
mod tests {
    use expect_test::expect_file;
    use itertools::Itertools;
    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

    use crate::core::config::{FluffConfig, Value};
    use crate::core::linter::linter::Linter;
    use crate::core::parser::segments::base::ErasedSegment;
    use crate::helpers;

    fn parse_sql(linter: &Linter, sql: &str) -> ErasedSegment {
        let parsed = linter.parse_string(sql, None, None, None).unwrap();
        parsed.tree.unwrap()
    }

    #[test]
    fn base_parse_struct() {
        let linter = Linter::new(
            FluffConfig::new(
                [(
                    "core".into(),
                    Value::Map([("dialect".into(), Value::String("duckdb".into()))].into()),
                )]
                .into(),
                None,
                None,
            ),
            None,
            None,
        );

        let files =
            glob::glob("test/fixtures/dialects/duckdb/*.sql").unwrap().flatten().collect_vec();

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
