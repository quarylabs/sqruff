use super::ansi::ansi_dialect;
use super::bigquery_keywords::{BIGQUERY_RESERVED_KEYWORDS, BIGQUERY_UNRESERVED_KEYWORDS};
use crate::core::dialects::base::Dialect;
use crate::core::parser::lexer::{RegexLexer, StringLexer};
use crate::core::parser::segments::base::{CodeSegment, CodeSegmentNewArgs};

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

    // Set Keywords
    dialect.sets_mut("unreserved_keywords").clear();
    dialect.update_keywords_set_from_multiline_string(
        "unreserved_keywords",
        BIGQUERY_UNRESERVED_KEYWORDS,
    );

    dialect.sets_mut("reserved_keywords").clear();
    dialect.update_keywords_set_from_multiline_string(
        "unreserved_keywords",
        BIGQUERY_RESERVED_KEYWORDS,
    );

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

    dialect
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
    fn hi() {
        let sql = parse_sql("SELECT 1");
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
