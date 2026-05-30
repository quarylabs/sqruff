use std::borrow::Cow;

use crate::api::{Engine, EngineOptions, ParseErrors, Source, SourceId};
use crate::config::FluffConfig;
use crate::core::test_functions::fresh_ansi_dialect;
use itertools::Itertools;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::context::ParseContext;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::segments::Tables;
use sqruff_lib_core::parser::segments::test_functions::lex;

fn parse_ansi_source(sql: &str) -> crate::api::ParsedDebugReport {
    Engine::new(
        FluffConfig::default(),
        EngineOptions {
            parse_errors: ParseErrors::Suppress,
        },
    )
    .unwrap()
    .parse_source(Source {
        id: SourceId::Virtual("test.sql".into()),
        text: Cow::Borrowed(sql),
    })
    .unwrap()
}

#[test]
fn test_dialect_ansi_file_lex() {
    // Define the test cases
    let test_cases = vec![
        ("a b", vec!["a", " ", "b", ""]),
        ("b.c", vec!["b", ".", "c", ""]),
        (
            "abc \n \t def  ;blah",
            vec!["abc", " ", "\n", " \t ", "def", "  ", ";", "blah", ""],
        ),
    ];

    for (raw, res) in test_cases {
        // Assume FluffConfig and Lexer are defined somewhere in your codebase
        let ansi = fresh_ansi_dialect();
        let lexer = ansi.lexer();

        let tables = Tables::default();
        // Assume that the lex function returns a Result with tokens
        let (tokens, errors) = lexer.lex(&tables, raw);

        assert_eq!(errors.len(), 0, "Lexing failed for input: {}", raw);

        // Check if the raw components of the tokens match the expected result
        let raw_list: Vec<_> = tokens.iter().map(|token| token.raw()).collect();
        assert_eq!(raw_list, res, "Mismatch for input: {:?}", raw);

        // Check if the concatenated raw components of the tokens match the original raw
        // string
        let concatenated: String = tokens.iter().map(|token| token.raw().as_str()).collect();
        assert_eq!(
            concatenated, raw,
            "Concatenation mismatch for input: {}",
            raw
        );
    }
}

#[test]
fn test_dialect_ansi_specific_segment_parses() {
    let cases = [
        ("SELECT", "select"),
        ("NakedIdentifierSegment", "online_sales"),
        ("BareFunctionSegment", "current_timestamp"),
        ("FunctionSegment", "current_timestamp()"),
        ("NumericLiteralSegment", "1000.0"),
        ("ExpressionSegment", "online_sales / 1000.0"),
        ("IntervalExpressionSegment", "INTERVAL 1 YEAR"),
        (
            "ExpressionSegment",
            "CASE WHEN id = 1 THEN 'nothing' ELSE 'test' END",
        ),
        // Nested Case Expressions
        (
            "ExpressionSegment",
            "CASE WHEN id = 1 THEN CASE WHEN true THEN 'something' ELSE 'nothing' END
            ELSE 'test' END",
        ),
        // Casting expressions
        (
            "ExpressionSegment",
            "CAST(ROUND(online_sales / 1000.0) AS varchar)",
        ),
        // Like expressions
        ("ExpressionSegment", "name NOT LIKE '%y'"),
        // Functions with a space
        ("SelectClauseElementSegment", "MIN (test.id) AS min_test_id"),
        // Interval literals
        (
            "ExpressionSegment",
            "DATE_ADD(CURRENT_DATE('America/New_York'), INTERVAL 1
            year)",
        ),
        // Array accessors
        ("ExpressionSegment", "my_array[1]"),
        ("ExpressionSegment", "my_array[OFFSET(1)]"),
        ("ExpressionSegment", "my_array[5:8]"),
        ("ExpressionSegment", "4 + my_array[OFFSET(1)]"),
        ("ExpressionSegment", "bits[OFFSET(0)] + 7"),
        (
            "SelectClauseElementSegment",
            "(count_18_24 * bits[OFFSET(0)]) / audience_size AS relative_abundance",
        ),
        (
            "ExpressionSegment",
            "count_18_24 * bits[OFFSET(0)] + count_25_34",
        ),
        (
            "SelectClauseElementSegment",
            "(count_18_24 * bits[OFFSET(0)] + count_25_34) / audience_size AS \
                 relative_abundance",
        ),
        // Dense math expressions
        (
            "SelectStatementSegment",
            "SELECT t.val/t.id FROM test WHERE id*1.0/id > 0.8",
        ),
        (
            "SelectStatementSegment",
            "SELECT foo FROM bar INNER JOIN baz",
        ),
        ("SelectClauseElementSegment", "t.val/t.id"),
        // Issue with casting raise as part of PR #177
        ("SelectClauseElementSegment", "CAST(num AS INT64)"),
        // Casting as datatype with arguments
        ("SelectClauseElementSegment", "CAST(num AS numeric(8,4))"),
        // Wildcard field selection
        ("SelectClauseElementSegment", "a.*"),
        ("SelectClauseElementSegment", "a.b.*"),
        ("SelectClauseElementSegment", "a.b.c.*"),
        // Default Element Syntax
        ("SelectClauseElementSegment", "a..c.*"),
        // Negative Elements
        ("SelectClauseElementSegment", "-some_variable"),
        ("SelectClauseElementSegment", "- some_variable"),
        // Complex Functions
        (
            "ExpressionSegment",
            "concat(left(uaid, 2), '|', right(concat('0000000', SPLIT_PART(uaid, '|', 4)),
            10), '|', '00000000')",
        ),
        // Notnull and Isnull
        ("ExpressionSegment", "c is null"),
        ("ExpressionSegment", "c is not null"),
        ("SelectClauseElementSegment", "c is null as c_isnull"),
        ("SelectClauseElementSegment", "c is not null as c_notnull"),
        // Shorthand casting
        ("ExpressionSegment", "NULL::INT"),
        ("SelectClauseElementSegment", "NULL::INT AS user_id"),
        ("TruncateStatementSegment", "TRUNCATE TABLE test"),
        ("TruncateStatementSegment", "TRUNCATE test"),
        ("FunctionNameSegment", "cte_1.foo"),
        (
            "SelectStatementSegment",
            "select * from my_cte cross join other_cte",
        ),
    ];

    let dialect = fresh_ansi_dialect();
    let config: FluffConfig = FluffConfig::default();

    for (segment_ref, sql_string) in cases {
        let config = config.clone();
        let parser: Parser = (&config).into();
        let mut ctx: ParseContext = (&parser).into();

        let segment = dialect.r#ref(segment_ref);
        let mut segments = lex(&dialect, sql_string);

        if segments.last().unwrap().get_type() == SyntaxKind::EndOfFile {
            segments.pop();
        }

        let tables = Tables::default();
        let match_result = segment.match_segments(&segments, 0, &mut ctx).unwrap();
        let mut parsed = match_result.apply(&tables, DialectKind::Ansi, &segments);

        assert_eq!(parsed.len(), 1, "failed {segment_ref}, {sql_string}");

        let parsed = parsed.pop().unwrap();
        assert_eq!(sql_string, parsed.raw());
    }
}

#[test]
fn test_dialect_ansi_specific_segment_not_parse() {
    let tests = vec![
        ("SELECT 1 + (2 ", vec![(1, 12)]),
        // ("SELECT * FROM a ORDER BY 1 UNION SELECT * FROM b", vec![(1, 28)]),
        // (
        //     "SELECT * FROM a LIMIT 1 UNION SELECT * FROM b",
        //     vec![(1, 25)],
        // ),
        // (
        //     "SELECT * FROM a ORDER BY 1 LIMIT 1 UNION SELECT * FROM b",
        //     vec![(1, 36)],
        // ),
    ];

    for (raw, err_locations) in tests {
        let parsed = parse_ansi_source(raw);
        assert!(!parsed.diagnostics.is_empty());

        let locs: Vec<(usize, usize)> = parsed
            .diagnostics
            .iter()
            .map(|v| (v.line, v.column))
            .collect();
        assert_eq!(locs, err_locations);
    }
}

#[test]
fn test_dialect_ansi_is_whitespace() {
    let file_content = std::fs::read_to_string(
        "../lib-dialects/test/fixtures/dialects/ansi/sqlfluff/select_in_multiline_comment.sql",
    )
    .expect("Unable to read file");

    let parsed = parse_ansi_source(&file_content);

    for raw_seg in parsed.tree.unwrap().get_raw_segments() {
        if raw_seg.is_type(SyntaxKind::Whitespace) || raw_seg.is_type(SyntaxKind::Newline) {
            assert!(raw_seg.is_whitespace());
        }
    }
}

#[test]
fn test_dialect_ansi_parse_indented_joins() {
    let cases = [
        (
            "select field_1 from my_table as alias_1",
            [1, 5, 8, 11, 15, 16, 17].as_slice(),
        ),
        (
            "select field_1 from my_table as alias_1 join foo using (field_1)",
            [1, 5, 8, 11, 15, 17, 19, 23, 24, 26, 29, 31, 33, 34, 35].as_slice(),
        ),
    ];

    for (sql_string, meta_loc) in cases {
        let parsed = parse_ansi_source(sql_string);
        let tree = parsed.tree.unwrap();

        let res_meta_locs = tree
            .get_raw_segments()
            .into_iter()
            .enumerate()
            .filter_map(|(idx, raw_seg)| raw_seg.is_meta().then_some(idx))
            .collect_vec();

        assert_eq!(res_meta_locs, meta_loc);
    }
}
