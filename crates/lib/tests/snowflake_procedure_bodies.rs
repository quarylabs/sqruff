use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;

fn linter(rules: &str) -> Linter {
    Linter::new(
        FluffConfig::from_source(
            &format!("[sqruff]\ndialect = snowflake\nrules = {rules}\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n"),
            None,
        ),
        None,
        None,
        true,
    )
    .unwrap()
}

fn procedure(language: &str, body: &str) -> String {
    format!("CREATE OR REPLACE PROCEDURE P()\nRETURNS INTEGER\n{language}AS\n{body};\n")
}

#[test]
fn formats_dollar_quoted_sql_and_is_idempotent() {
    let linter = linter("CP01,LT01,LT02,LT12,LT13");
    let input = procedure(
        "LANGUAGE SQL\n",
        "$$\nbegin\nselect 1+2 as total;\nreturn 1;\nend;\n$$",
    );
    let expected = procedure(
        "LANGUAGE SQL\n",
        "$$\nBEGIN\n    SELECT 1 + 2 AS total;\n    RETURN 1;\nEND;\n$$",
    );
    let linted = linter.lint_string(&input, None, true).unwrap();
    assert!(
        !linted.has_parse_or_templating_errors(),
        "{:?}",
        linted.violations()
    );
    assert_eq!(linted.fix_string(), expected);
    let linted = linter.lint_string(&expected, None, true).unwrap();
    assert!(linted.violations().is_empty(), "{:?}", linted.violations());
    assert_eq!(linted.fix_string(), expected);
}

#[test]
fn omitted_language_defaults_to_sql() {
    let linter = linter("CP01");
    let input = procedure("", "$$begin return 1; end;$$");
    assert_eq!(
        linter.lint_string(&input, None, true).unwrap().fix_string(),
        procedure("", "$$BEGIN RETURN 1; END;$$")
    );
}

#[test]
fn non_sql_bodies_and_literals_are_preserved() {
    let linter = linter("CP01,LT01,LT02");
    for language in ["JAVASCRIPT", "PYTHON", "JAVA", "SCALA"] {
        // Even SQL-looking contents must stay untouched in another language.
        let input = procedure(
            &format!("LANGUAGE {language}\n"),
            "$$begin return 1+2; end;$$",
        );
        let linted = linter.lint_string(&input, None, true).unwrap();
        assert!(linted.violations().is_empty(), "{:?}", linted.violations());
        assert_eq!(linted.fix_string(), input);
    }
    let input = "SELECT $$select 1+2$$;\n";
    assert_eq!(
        linter.lint_string(input, None, true).unwrap().fix_string(),
        input
    );
}

#[test]
fn single_quoted_bodies_preserve_string_values() {
    let linter = linter("CP01,LT01");
    let input = procedure("LANGUAGE SQL\n", "'begin return ''a\\\\b''''c''; end;'");
    let expected = procedure("LANGUAGE SQL\n", "'BEGIN RETURN ''a\\\\b''''c''; END;'");
    let linted = linter.lint_string(&input, None, true).unwrap();
    assert!(
        !linted.has_parse_or_templating_errors(),
        "{:?}",
        linted.violations()
    );
    assert_eq!(linted.fix_string(), expected);
    assert_eq!(
        linter
            .lint_string(&expected, None, true)
            .unwrap()
            .fix_string(),
        expected
    );
}

#[test]
fn lint_positions_refer_to_the_containing_file() {
    let input = procedure(
        "LANGUAGE SQL\n",
        "$$\nBEGIN\nSELECT 1+2;\nRETURN 1;\nEND;\n$$",
    );
    let linted = linter("LT01").lint_string(&input, None, false).unwrap();
    let positions: Vec<_> = linted
        .violations()
        .iter()
        .map(|v| (v.line_no, v.line_pos))
        .collect();
    assert_eq!(positions, vec![(7, 9), (7, 10)]);
    for violation in linted.violations() {
        assert!(matches!(&input[violation.source_slice.clone()], "+" | "2"));
    }
}

#[test]
fn unparsable_body_is_not_modified() {
    let input = procedure(
        "LANGUAGE SQL\n",
        "$$\nbegin\nselect 1+2;\nthis is not valid SQL;\nend;\n$$",
    );
    let linted = linter("CP01,LT01,LT02")
        .lint_string(&input, None, true)
        .unwrap();
    assert!(linted.has_parse_or_templating_errors());
    assert!(
        linted
            .violations()
            .iter()
            .any(|v| v.description == "Unparsable section")
    );
    assert_eq!(linted.fix_string(), input);
}

#[test]
fn body_and_wrapper_fixes_are_combined() {
    let input = "create or replace procedure p()\nreturns integer\nlanguage sql\nas $$begin return 1+2; end;$$;\n";
    let expected = "CREATE OR REPLACE PROCEDURE p()\nRETURNS integer\nLANGUAGE SQL\nAS $$BEGIN RETURN 1 + 2; END;$$;\n";
    assert_eq!(
        linter("CP01,LT01")
            .lint_string(input, None, true)
            .unwrap()
            .fix_string(),
        expected
    );
}

#[test]
fn multiple_bodies_have_independent_languages() {
    let sql = procedure("LANGUAGE SQL\n", "$$begin return 1+2; end;$$");
    let js = procedure("LANGUAGE JAVASCRIPT\n", "$$return 1+2;$$");
    let implicit = procedure("", "$$begin return 3+4; end;$$");
    let input = format!("{sql}{js}{implicit}");
    let expected = format!(
        "{}{js}{}",
        procedure("LANGUAGE SQL\n", "$$BEGIN RETURN 1 + 2; END;$$"),
        procedure("", "$$BEGIN RETURN 3 + 4; END;$$")
    );
    assert_eq!(
        linter("CP01,LT01")
            .lint_string(&input, None, true)
            .unwrap()
            .fix_string(),
        expected
    );
}

#[test]
fn noqa_inside_and_outside_the_body_prevents_fixes() {
    let linter = linter("CP01,LT01");
    let input = procedure(
        "LANGUAGE SQL\n",
        "$$\nBEGIN\nRETURN 1+2; -- noqa: LT01\nEND;\n$$",
    );
    let linted = linter.lint_string(&input, None, true).unwrap();
    assert!(linted.violations().is_empty(), "{:?}", linted.violations());
    assert_eq!(linted.fix_string(), input);

    let input = format!(
        "-- noqa: disable=LT01\n{}",
        procedure("LANGUAGE SQL\n", "$$BEGIN RETURN 1+2; END;$$")
    );
    let linted = linter.lint_string(&input, None, true).unwrap();
    assert!(linted.violations().is_empty(), "{:?}", linted.violations());
    assert_eq!(linted.fix_string(), input);
}

#[test]
fn declarations_and_dynamic_sql_use_existing_rules() {
    let input = procedure(
        "LANGUAGE SQL\n",
        "$$\ndeclare\nn integer default 0;\nbegin\nn:=1+2;\nexecute immediate 'select 1+2';\nreturn n;\nend;\n$$",
    );
    let expected = procedure(
        "LANGUAGE SQL\n",
        "$$\nDECLARE\n    n integer DEFAULT 0;\nBEGIN\n    n := 1 + 2;\n    EXECUTE IMMEDIATE 'select 1+2';\n    RETURN n;\nEND;\n$$",
    );
    let linted = linter("CP01,LT01,LT02")
        .lint_string(&input, None, true)
        .unwrap();
    assert!(
        !linted.has_parse_or_templating_errors(),
        "{:?}",
        linted.violations()
    );
    assert_eq!(linted.fix_string(), expected);
}

#[test]
fn escaped_newlines_and_unicode_diagnostics_map_to_raw_source() {
    let input = procedure("LANGUAGE SQL\n", "'begin\\nreturn ''雪'';\\nend;'");
    let linter = linter("CP01");
    let linted = linter.lint_string(&input, None, false).unwrap();
    assert!(
        !linted.has_parse_or_templating_errors(),
        "{:?}",
        linted.violations()
    );
    for violation in linted.violations() {
        let raw = &input[violation.source_slice.clone()];
        assert!(matches!(raw, "begin" | "return" | "end"), "{raw:?}");
        assert_eq!(violation.line_no, 5);
        let expected_column = input.lines().nth(4).unwrap().find(raw).unwrap();
        assert_eq!(
            violation.line_pos,
            input.lines().nth(4).unwrap()[..expected_column]
                .chars()
                .count()
                + 1
        );
    }
    let expected = procedure("LANGUAGE SQL\n", "'BEGIN\nRETURN ''雪'';\nEND;'");
    assert_eq!(
        linter.lint_string(&input, None, true).unwrap().fix_string(),
        expected
    );
}

#[test]
fn file_boundary_rules_do_not_change_body_delimiters() {
    let input = procedure("LANGUAGE SQL\n", "$$ BEGIN RETURN 1; END; $$");
    let linted = linter("LT12,LT13").lint_string(&input, None, true).unwrap();
    assert!(linted.violations().is_empty(), "{:?}", linted.violations());
    assert_eq!(linted.fix_string(), input);
}

#[test]
fn functions_and_non_snowflake_procedures_keep_their_quoted_bodies() {
    let input = "CREATE FUNCTION F() RETURNS INTEGER LANGUAGE SQL AS $$select 1+2$$;\n";
    assert_eq!(
        linter("CP01,LT01")
            .lint_string(input, None, true)
            .unwrap()
            .fix_string(),
        input
    );
    let linter = Linter::new(
        FluffConfig::from_source("[sqruff]\ndialect = postgres\nrules = LT01\n", None),
        None,
        None,
        true,
    )
    .unwrap();
    let input = "CREATE PROCEDURE p() LANGUAGE SQL AS $$SELECT 1+2;$$;\n";
    assert_eq!(
        linter.lint_string(input, None, true).unwrap().fix_string(),
        input
    );
}

#[test]
fn template_expansions_are_never_written_into_the_body() {
    let linter = Linter::new(
        FluffConfig::from_source(
            "[sqruff]\ndialect = snowflake\ntemplater = placeholder\nrules = LT01\n[sqruff:templater:placeholder]\nparam_style = colon\nvalue = 42\n",
            None,
        ), None, None, true,
    ).unwrap();
    let input = procedure("LANGUAGE SQL\n", "$$BEGIN RETURN :value+1; END;$$");
    let linted = linter.lint_string(&input, None, true).unwrap();
    assert!(
        !linted.has_parse_or_templating_errors(),
        "{:?}",
        linted.violations()
    );
    assert_eq!(linted.fix_string(), input);
}

#[test]
fn escaped_crlf_diagnostics_keep_original_positions() {
    let input = procedure("LANGUAGE SQL\n", "'begin\\r\\nreturn 1;\\r\\nend;'");
    let linted = linter("CP01").lint_string(&input, None, false).unwrap();
    let raw: Vec<_> = linted
        .violations()
        .iter()
        .map(|violation| &input[violation.source_slice.clone()])
        .collect();
    assert_eq!(raw, vec!["begin", "return", "end"]);
    let expected = procedure("LANGUAGE SQL\n", "'BEGIN\\r\nRETURN 1;\\r\nEND;'");
    assert_eq!(
        linter("CP01")
            .lint_string(&input, None, true)
            .unwrap()
            .fix_string(),
        expected
    );
}

#[test]
fn indentation_settings_and_rule_exclusions_apply_inside_the_body() {
    let linter = Linter::new(
        FluffConfig::from_source(
            "[sqruff]\ndialect = snowflake\nrules = CP01,LT01,LT02\nexclude_rules = LT01\n[sqruff:indentation]\ntab_space_size = 2\n[sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = lower\n",
            None,
        ), None, None, true,
    ).unwrap();
    let input = "create procedure p()\nreturns integer\nlanguage sql\nas $$\nBEGIN\nRETURN 1+2;\nEND;\n$$;\n";
    let expected = "create procedure p()\nreturns integer\nlanguage sql\nas $$\nbegin\n  return 1+2;\nend;\n$$;\n";
    let linted = linter.lint_string(input, None, true).unwrap();
    assert!(
        !linted.has_parse_or_templating_errors(),
        "{:?}",
        linted.violations()
    );
    assert_eq!(linted.fix_string(), expected);
}

#[test]
fn escaped_carriage_returns_in_string_values_are_preserved() {
    let input = procedure("LANGUAGE SQL\n", "'begin return ''a\\rb''; end;'");
    let expected = procedure("LANGUAGE SQL\n", "'BEGIN RETURN ''a\\rb''; END;'");
    let linter = linter("CP01");
    let linted = linter.lint_string(&input, None, true).unwrap();
    assert!(
        !linted.has_parse_or_templating_errors(),
        "{:?}",
        linted.violations()
    );
    assert_eq!(linted.fix_string(), expected);
    assert_eq!(
        linter
            .lint_string(&expected, None, true)
            .unwrap()
            .fix_string(),
        expected
    );
}
