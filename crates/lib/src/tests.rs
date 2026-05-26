use itertools::Itertools;
use sqruff_lib::api::ParseErrors;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib::core::test_functions::fresh_ansi_dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::context::ParseContext;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::segments::Tables;
use sqruff_lib_core::parser::segments::test_functions::lex;

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
    let config: FluffConfig = FluffConfig::new(<_>::default(), None, None);

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
        let lnt = Linter::new(
            FluffConfig::new(<_>::default(), None, None),
            None,
            ParseErrors::Suppress,
        )
        .unwrap();
        let tables = Tables::default();
        let parsed = lnt.parse_string(&tables, raw, None).unwrap();
        assert!(!parsed.violations.is_empty());

        let locs: Vec<(usize, usize)> = parsed
            .violations
            .iter()
            .map(|v| (v.line_no, v.line_pos))
            .collect();
        assert_eq!(locs, err_locations);
    }
}

#[test]
fn test_dialect_ansi_is_whitespace() {
    let lnt = Linter::new(
        FluffConfig::new(<_>::default(), None, None),
        None,
        ParseErrors::Suppress,
    )
    .unwrap();
    let file_content = std::fs::read_to_string(
        "../lib-dialects/test/fixtures/dialects/ansi/sqlfluff/select_in_multiline_comment.sql",
    )
    .expect("Unable to read file");

    let tables = Tables::default();
    let parsed = lnt.parse_string(&tables, &file_content, None).unwrap();

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
    let lnt = Linter::new(
        FluffConfig::new(<_>::default(), None, None),
        None,
        ParseErrors::Suppress,
    )
    .unwrap();

    for (sql_string, meta_loc) in cases {
        let tables = Tables::default();
        let parsed = lnt.parse_string(&tables, sql_string, None).unwrap();
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

/// Issue #2607: layout rules must not flag false positives inside
/// template-generated regions. Uses real jinja slices (captured from the jinja
/// templater) for a nested `{% for %}`/`{% if %}` so the test runs without the
/// Python jinja runtime. Before the template-placeholder reindent port this
/// produced false-positive LT02 violations on the template-tag lines.
#[test]
fn test_reindent_no_false_positive_in_jinja_for_loop() {
    use sqruff_lib_core::templaters::{
        RawFileSlice, TemplateSliceKind, TemplatedFile, TemplatedFileSlice,
    };

    let source = "select\n    {% for col in ['a', 'b', 'c'] %}\n        {{ col }}\n        {% if not loop.last %}, {% endif %}\n    {% endfor %}\nfrom my_table\n";
    let templated = "select\n    \n        a\n        , \n    \n        b\n        , \n    \n        c\n        \n    \nfrom my_table\n";

    // (slice_type, source_start, source_end, templated_start, templated_end)
    let sliced: &[(&str, usize, usize, usize, usize)] = &[
        ("literal", 0, 11, 0, 11),
        ("block_start", 11, 43, 11, 11),
        ("literal", 43, 52, 11, 20),
        ("templated", 52, 61, 20, 21),
        ("literal", 61, 70, 21, 30),
        ("block_start", 70, 92, 30, 30),
        ("literal", 92, 94, 30, 32),
        ("block_end", 94, 105, 32, 32),
        ("literal", 105, 110, 32, 37),
        ("block_end", 110, 122, 37, 37),
        ("literal", 43, 52, 37, 46),
        ("templated", 52, 61, 46, 47),
        ("literal", 61, 70, 47, 56),
        ("block_start", 70, 92, 56, 56),
        ("literal", 92, 94, 56, 58),
        ("block_end", 94, 105, 58, 58),
        ("literal", 105, 110, 58, 63),
        ("block_end", 110, 122, 63, 63),
        ("literal", 43, 52, 63, 72),
        ("templated", 52, 61, 72, 73),
        ("literal", 61, 70, 73, 82),
        ("block_start", 70, 92, 82, 82),
        ("block_end", 94, 105, 82, 82),
        ("literal", 105, 110, 82, 87),
        ("block_end", 110, 122, 87, 87),
        ("literal", 122, 137, 87, 102),
    ];
    // (slice_type, source_idx, block_idx, raw)
    let raw: &[(&str, usize, usize, &str)] = &[
        ("literal", 0, 0, "select\n    "),
        ("block_start", 11, 1, "{% for col in ['a', 'b', 'c'] %}"),
        ("literal", 43, 1, "\n        "),
        ("templated", 52, 1, "{{ col }}"),
        ("literal", 61, 1, "\n        "),
        ("block_start", 70, 2, "{% if not loop.last %}"),
        ("literal", 92, 2, ", "),
        ("block_end", 94, 2, "{% endif %}"),
        ("literal", 105, 3, "\n    "),
        ("block_end", 110, 3, "{% endfor %}"),
        ("literal", 122, 4, "\nfrom my_table\n"),
    ];

    let sliced_file = sliced
        .iter()
        .map(|&(k, ss, se, ts, te)| {
            TemplatedFileSlice::new_typed(
                TemplateSliceKind::from_slice_type(k).unwrap(),
                ss..se,
                ts..te,
            )
        })
        .collect();
    let raw_sliced = raw
        .iter()
        .map(|&(k, idx, blk, r)| {
            RawFileSlice::new_typed(
                r.to_string(),
                TemplateSliceKind::from_slice_type(k).unwrap(),
                idx,
                None,
                Some(blk),
            )
        })
        .collect();

    let templated_file = TemplatedFile::new(
        source.to_string(),
        "a.sql".to_string(),
        Some(templated.to_string()),
        Some(sliced_file),
        Some(raw_sliced),
    )
    .unwrap();

    // Use `crate::` paths so these match the internal crate instance (the
    // module imports `sqruff_lib` as an external crate at the top of the file).
    let rendered = crate::core::linter::common::RenderedFile {
        templated_file,
        templater_violations: vec![],
        filename: "a.sql".to_string(),
        source_str: source.to_string(),
    };

    let lnt = crate::core::linter::core::Linter::new(
        crate::core::config::FluffConfig::new(<_>::default(), None, None),
        None,
        None,
        false,
    )
    .unwrap();
    let linted = lnt.lint_rendered(rendered, false).unwrap();

    let layout: Vec<_> = linted
        .violations()
        .iter()
        .filter(|v| matches!(v.rule_code(), "LT01" | "LT02"))
        .map(|v| (v.rule_code(), v.line_no, v.line_pos))
        .collect();
    assert!(
        layout.is_empty(),
        "unexpected layout violations in templated region: {layout:?}"
    );
}

/// LT12 is a source-file rule. A templated file can render with a final newline
/// while the source file still lacks one because trailing Jinja blocks render to
/// zero length.
#[test]
fn test_lt12_reports_missing_source_newline_after_jinja_block() {
    use sqruff_lib_core::templaters::{
        RawFileSlice, TemplateSliceKind, TemplatedFile, TemplatedFileSlice,
    };

    let source =
        "select\n    id\nfrom my_table\n{% if is_incremental() %}\nwhere id > 0\n{% endif %}";
    let templated = "select\n    id\nfrom my_table\n";

    let sliced_file = vec![
        TemplatedFileSlice::new_typed(TemplateSliceKind::Literal, 0..28, 0..28),
        TemplatedFileSlice::new_typed(TemplateSliceKind::BlockStart, 28..53, 28..28),
        TemplatedFileSlice::new_typed(TemplateSliceKind::Literal, 53..67, 28..28),
        TemplatedFileSlice::new_typed(TemplateSliceKind::BlockEnd, 67..78, 28..28),
    ];
    let raw_sliced = vec![
        RawFileSlice::new_typed(
            "select\n    id\nfrom my_table\n".to_string(),
            TemplateSliceKind::Literal,
            0,
            None,
            Some(0),
        ),
        RawFileSlice::new_typed(
            "{% if is_incremental() %}".to_string(),
            TemplateSliceKind::BlockStart,
            28,
            None,
            Some(1),
        ),
        RawFileSlice::new_typed(
            "\nwhere id > 0\n".to_string(),
            TemplateSliceKind::Literal,
            53,
            None,
            Some(1),
        ),
        RawFileSlice::new_typed(
            "{% endif %}".to_string(),
            TemplateSliceKind::BlockEnd,
            67,
            None,
            Some(1),
        ),
    ];

    let templated_file = TemplatedFile::new(
        source.to_string(),
        "a.sql".to_string(),
        Some(templated.to_string()),
        Some(sliced_file),
        Some(raw_sliced),
    )
    .unwrap();

    let rendered = crate::core::linter::common::RenderedFile {
        templated_file,
        templater_violations: vec![],
        filename: "a.sql".to_string(),
        source_str: source.to_string(),
    };

    let lnt = crate::core::linter::core::Linter::new(
        crate::core::config::FluffConfig::new(<_>::default(), None, None),
        None,
        None,
        false,
    )
    .unwrap();
    let linted = lnt.lint_rendered(rendered, false).unwrap();

    let lt12: Vec<_> = linted
        .violations()
        .iter()
        .filter(|v| v.rule_code() == "LT12")
        .map(|v| (v.line_no, v.line_pos))
        .collect();
    assert!(!lt12.is_empty(), "expected LT12 violation");
}

/// Trailing source-only Jinja blocks should not hide extra rendered newlines
/// from LT12.
#[test]
fn test_lt12_reports_extra_rendered_newline_before_jinja_block() {
    use sqruff_lib_core::templaters::{
        RawFileSlice, TemplateSliceKind, TemplatedFile, TemplatedFileSlice,
    };

    let source = "select\n    id\nfrom my_table\n\n{% if is_incremental() %}\n{% endif %}\n";
    let templated = "select\n    id\nfrom my_table\n\n";

    let sliced_file = vec![
        TemplatedFileSlice::new_typed(TemplateSliceKind::Literal, 0..29, 0..29),
        TemplatedFileSlice::new_typed(TemplateSliceKind::BlockStart, 29..54, 29..29),
        TemplatedFileSlice::new_typed(TemplateSliceKind::Literal, 54..55, 29..29),
        TemplatedFileSlice::new_typed(TemplateSliceKind::BlockEnd, 55..66, 29..29),
        TemplatedFileSlice::new_typed(TemplateSliceKind::Literal, 66..67, 29..29),
    ];
    let raw_sliced = vec![
        RawFileSlice::new_typed(
            "select\n    id\nfrom my_table\n\n".to_string(),
            TemplateSliceKind::Literal,
            0,
            None,
            Some(0),
        ),
        RawFileSlice::new_typed(
            "{% if is_incremental() %}".to_string(),
            TemplateSliceKind::BlockStart,
            29,
            None,
            Some(1),
        ),
        RawFileSlice::new_typed(
            "\n".to_string(),
            TemplateSliceKind::Literal,
            54,
            None,
            Some(1),
        ),
        RawFileSlice::new_typed(
            "{% endif %}".to_string(),
            TemplateSliceKind::BlockEnd,
            55,
            None,
            Some(1),
        ),
        RawFileSlice::new_typed(
            "\n".to_string(),
            TemplateSliceKind::Literal,
            66,
            None,
            Some(1),
        ),
    ];

    let templated_file = TemplatedFile::new(
        source.to_string(),
        "a.sql".to_string(),
        Some(templated.to_string()),
        Some(sliced_file),
        Some(raw_sliced),
    )
    .unwrap();

    let rendered = crate::core::linter::common::RenderedFile {
        templated_file,
        templater_violations: vec![],
        filename: "a.sql".to_string(),
        source_str: source.to_string(),
    };

    let lnt = crate::core::linter::core::Linter::new(
        crate::core::config::FluffConfig::new(<_>::default(), None, None),
        None,
        None,
        false,
    )
    .unwrap();
    let linted = lnt.lint_rendered(rendered, false).unwrap();

    let lt12: Vec<_> = linted
        .violations()
        .iter()
        .filter(|v| v.rule_code() == "LT12")
        .map(|v| (v.line_no, v.line_pos))
        .collect();
    assert!(!lt12.is_empty(), "expected LT12 violation");
}
