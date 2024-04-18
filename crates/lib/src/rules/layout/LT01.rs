use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::reflow::sequence::{Filter, ReflowSequence};

#[derive(Default, Debug, Clone)]
pub struct RuleLT01 {}

impl Rule for RuleLT01 {
    fn name(&self) -> &'static str {
        "layout.spacing"
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let sequence = ReflowSequence::from_root(context.segment, context.config.unwrap());
        sequence.respace(false, Filter::All).results()
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT01::RuleLT01;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT01::default().erased()]
    }

    // LT01-commas.yml

    #[test]
    fn test_fail_whitespace_before_comma() {
        let sql = fix("SELECT 1 ,4".into(), rules());
        assert_eq!(sql, "SELECT 1, 4");
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_fail_whitespace_before_comma_template() {
        let sql = lint("{{ 'SELECT 1 ,4' }}".into(), "ansi".into(), rules(), None, None).unwrap();

        dbg!(sql);
    }

    #[test]
    fn test_lint_drop_cast_no_errors() {
        let sql =
            lint("DROP CAST (sch.udt_1 AS sch.udt_2);".into(), "ansi".into(), rules(), None, None)
                .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_pass_errors_only_in_templated_and_ignore() {
        // ignore_templated_areas: true
        let sql = lint("{{ 'SELECT 1 ,4' }}".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_fail_errors_only_in_non_templated_and_ignore() {
        // ignore_templated_areas: true
        let sql = fix("{{ 'SELECT 1, 4' }}, 5 , 6".into(), rules());
        assert_eq!(sql, "{{ 'SELECT 1, 4' }}, 5, 6");
    }

    #[test]
    fn test_pass_single_whitespace_after_comma() {
        let sql = fix("SELECT 1, 4".into(), rules());
        assert_eq!(sql, "SELECT 1, 4");
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_pass_single_whitespace_after_comma_template() {
        // ignore_templated_areas: false
        let sql = lint("{{ 'SELECT 1, 4' }}".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_fail_multiple_whitespace_after_comma() {
        let sql = fix("SELECT 1,   4".into(), rules());
        assert_eq!(sql, "SELECT 1, 4");
    }

    #[test]
    fn test_fail_no_whitespace_after_comma() {
        let sql = fix("SELECT 1,4".into(), rules());
        assert_eq!(sql, "SELECT 1, 4");
    }

    #[test]
    fn test_fail_no_whitespace_after_comma_2() {
        let sql = fix("SELECT FLOOR(dt) ,count(*) FROM test".into(), rules());
        assert_eq!(sql, "SELECT FLOOR(dt), count(*) FROM test");
    }

    #[test]
    fn test_pass_bigquery_trailing_comma() {
        let sql = lint("SELECT 1, 2,".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    // LT01-missing.yml
    #[test]
    fn test_fail_no_space_after_using_clause() {
        let sql = fix("select * from a JOIN b USING(x)".into(), rules());
        assert_eq!(sql, "select * from a JOIN b USING (x)");
    }

    #[test]
    fn test_pass_newline_after_using_clause() {
        // Check LT01 passes if there's a newline between
        let sql =
            lint("select * from a JOIN b USING\n(x)".into(), "ansi".into(), rules(), None, None)
                .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_fail_cte_no_space_after_as() {
        let sql = fix("WITH a AS(select 1) select * from a".into(), rules());
        assert_eq!(sql, "WITH a AS (select 1) select * from a");
    }

    #[test]
    fn test_fail_multiple_spaces_after_as() {
        let sql = fix("WITH a AS  (select 1) select * from a".into(), rules());
        assert_eq!(sql, "WITH a AS (select 1) select * from a");
    }

    #[test]
    #[ignore = "incorrect spacing"]
    fn test_fail_cte_newline_after_as() {
        let sql = fix("WITH a AS\n(select 1)\nselect * from a".into(), rules());
        assert_eq!(sql, "WITH a AS (select 1)\nselect * from a");
    }

    #[test]
    #[ignore = "incorrect spacing"]
    fn test_fail_cte_newline_and_spaces_after_as() {
        let sql = fix("WITH a AS\n\n\n(select 1)\nselect * from a".into(), rules());
        assert_eq!(sql, "WITH a AS (select 1)\nselect * from a");
    }

    // LT01-alignment.yml

    // configs: layout: type: alias_expression: spacing_before: single
    #[test]
    fn test_excess_space_without_align_alias() {
        let sql = fix(
            "
        SELECT
            a    AS first_column,
            b      AS second_column,
            (a + b) / 2 AS third_column
        FROM foo"
                .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        SELECT
            a AS first_column,
            b AS second_column,
            (a + b) / 2 AS third_column
        FROM foo"
        );
    }

    // configs: layout: type: alias_expression: spacing_before: align, align_within:
    // select_clause, align_scope: bracketed
    #[test]
    #[ignore = "parser needs further development"]
    fn test_excess_space_with_align_alias() {
        let sql = fix(
            "
        SELECT
            a    AS first_column,
            b      AS second_column,
            (a + b) / 2 AS third_column
        FROM foo   AS bar
    "
            .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        SELECT
            a           AS first_column,
            b           AS second_column,
            (a + b) / 2 AS third_column
        FROM foo AS bar
    "
        );
    }

    // configs: *align_alias
    #[test]
    #[ignore = "parser needs further development"]
    fn test_missing_keyword_with_align_alias() {
        let sql = fix(
            "
        SELECT
            a    first_column,
            b      AS second_column,
            (a + b) / 2 AS third_column
        FROM foo
    "
            .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        SELECT
            a           first_column,
            b           AS second_column,
            (a + b) / 2 AS third_column
        FROM foo
    "
        );
    }

    // configs: *align_alias
    #[test]
    fn test_skip_alias_with_align_alias() {
        let sql = fix(
            "
        SELECT
            a   ,
            b   ,
            (a   +   b) /   2
        FROM foo"
                .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        SELECT
            a,
            b,
            (a + b) / 2
        FROM foo"
        );
    }

    // configs: *align_alias_wider
    #[test]
    #[ignore = "parser needs further development"]
    fn test_excess_space_with_align_alias_wider() {
        let sql = fix(
            "
        SELECT
            a    AS first_column,
            b      AS second_column,
            (a      +      b)      /      2 AS third_column
        FROM foo   AS first_table
        JOIN my_tbl AS second_table USING(a)
    "
            .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        SELECT
            a           AS first_column,
            b           AS second_column,
            (a + b) / 2 AS third_column
        FROM foo        AS first_table
        JOIN my_tbl     AS second_table USING (a)
    "
        );
    }

    // configs: *align_alias
    #[test]
    #[ignore = "parser needs further development"]
    fn test_align_alias_boundary() {
        let sql = fix(
            "
        SELECT
            a    AS first_column,
            (SELECT b AS c)      AS second_column
    "
            .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        SELECT
            a               AS first_column,
            (SELECT b AS c) AS second_column
    "
        );
    }

    // configs: *align_alias
    #[test]
    fn test_align_alias_inline_pass() {
        let sql = lint("SELECT a AS b, c AS d FROM tbl".into(), "ansi".into(), rules(), None, None)
            .unwrap();
        assert_eq!(sql, &[]);
    }

    // configs: *align_alias
    #[test]
    fn test_align_alias_inline_fail() {
        let sql = fix("SELECT a   AS   b  ,   c   AS   d    FROM tbl".into(), rules());
        assert_eq!(sql, "SELECT a AS b, c AS d FROM tbl");
    }

    // configs: layout: type: data_type: spacing_before: align, align_within:
    // create_table_statement, column_constraint_segment: spacing_before: align,
    // align_within: create_table_statement
    #[test]
    #[ignore = "parser needs further development"]
    fn test_align_multiple_a() {
        let sql = fix(
            "
        CREATE TABLE tbl (
            foo VARCHAR(25) NOT NULL,
            barbar INT NULL
        )
    "
            .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        CREATE TABLE tbl (
            foo    VARCHAR(25) NOT NULL,
            barbar INT         NULL
        )
    "
        );
    }

    // configs: [same as test_align_multiple_a]
    #[test]
    #[ignore = "parser needs further development"]
    fn test_align_multiple_b() {
        let sql = fix(
            "
        create table tab (
            foo    varchar(25)  not null,
            barbar int not null unique
        )
    "
            .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        create table tab (
            foo    varchar(25) not null,
            barbar int         not null unique
        )
        "
        );
    }

    // LT01-brackets.yml

    #[test]
    fn test_pass_parenthesis_block_isolated() {
        let sql = lint(
            "SELECT * FROM (SELECT 1 AS C1) AS T1;".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    // configs: core: ignore_templated_areas: false
    #[test]
    #[ignore = "parser needs further development"]
    fn test_pass_parenthesis_block_isolated_template() {
        let sql = lint(
            "{{ 'SELECT * FROM (SELECT 1 AS C1) AS T1;' }}".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_fail_parenthesis_block_not_isolated() {
        let sql = fix("SELECT * FROM(SELECT 1 AS C1)AS T1;".into(), rules());
        assert_eq!(sql, "SELECT * FROM (SELECT 1 AS C1) AS T1;");
    }

    // configs: core: ignore_templated_areas: false
    #[test]
    #[ignore = "parser needs further development"]
    fn test_fail_parenthesis_block_not_isolated_templated() {
        let sql = fix("{{ 'SELECT * FROM(SELECT 1 AS C1)AS T1;' }}".into(), rules());
        assert_eq!(sql, "{{ 'SELECT * FROM (SELECT 1 AS C1) AS T1;' }}");
    }

    #[test]
    fn test_pass_parenthesis_function() {
        let sql = lint("SELECT foo;".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    // LT01-excessive.yml
    #[test]
    fn test_basic() {
        let sql = lint("SELECT 1".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    // configs: core: ignore_templated_areas: false
    #[test]
    #[ignore = "parser needs further development"]
    fn test_basic_template() {
        let sql = lint("{{ 'SELECT 1' }}".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_basic_fix() {
        let sql = fix("SELECT     1".into(), rules());
        assert_eq!(sql, "SELECT 1");
    }

    // configs: core: ignore_templated_areas: false
    #[test]
    #[ignore = "parser needs further development"]
    fn test_basic_fail_template() {
        let sql = fix("{{ 'SELECT     1' }}".into(), rules());
        assert_eq!(sql, "{{ 'SELECT 1' }}");
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_simple_fix() {
        let sql = fix(
            "
        select
            1 + 2     + 3     + 4        -- Comment
        from     foo
    "
            .into(),
            rules(),
        );
        assert_eq!(
            sql,
            "
        select
            1 + 2 + 3 + 4        -- Comment
        from foo
    "
        );
    }

    // LT01-literals.yml

    #[test]
    fn test_pass_simple_select() {
        let sql = lint("SELECT 'foo'".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_pass_expression() {
        let sql =
            lint("SELECT ('foo' || 'bar') as buzz".into(), "ansi".into(), rules(), None, None)
                .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_fail_as() {
        let sql = fix("SELECT 'foo'AS   bar FROM foo".into(), rules());
        assert_eq!(sql, "SELECT 'foo' AS bar FROM foo");
    }

    #[test]
    fn test_fail_expression() {
        let sql = fix("SELECT ('foo'||'bar') as buzz".into(), rules());
        assert_eq!(sql, "SELECT ('foo' || 'bar') as buzz");
    }

    #[test]
    fn test_pass_comma() {
        let sql = lint(
            "
        SELECT
            col1,
            'string literal' AS new_column_literal,
            CASE WHEN col2 IN ('a', 'b') THEN 'Y' ELSE 'N' END AS new_column_case
        FROM some_table
        WHERE col2 IN ('a', 'b', 'c', 'd');
    "
            .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    // configs: core: dialect: snowflake
    #[test]
    #[ignore = "snowflake"]
    fn test_pass_semicolon() {
        let sql = lint(
            "ALTER SESSION SET TIMEZONE = 'UTC';".into(),
            "snowflake".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    // configs: core: dialect: bigquery
    #[test]
    #[ignore = "bigquery"]
    fn test_pass_bigquery_udf_triple_single_quote() {
        let sql = lint(
            "
        CREATE TEMPORARY FUNCTION a()
        LANGUAGE js
        AS '''
        CODE GOES HERE
        ''';
    "
            .into(),
            "bigquery".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    // configs: core: dialect: bigquery
    #[test]
    #[ignore = "bigquery"]
    fn test_pass_bigquery_udf_triple_double_quote() {
        let sql = lint(
            r#"
        CREATE TEMPORARY FUNCTION a()
        LANGUAGE js
        AS """
        CODE GOES HERE
        """;
    "#
            .into(),
            "bigquery".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_pass_ansi_single_quote() {
        let sql = lint("SELECT a + 'b' + 'c' FROM tbl;".into(), "ansi".into(), rules(), None, None)
            .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_fail_ansi_single_quote() {
        let sql = fix("SELECT a +'b'+ 'c' FROM tbl;".into(), rules());
        assert_eq!(sql, "SELECT a + 'b' + 'c' FROM tbl;");
    }

    // LT01-operators.yml

    #[test]
    fn test_pass_brackets() {
        let sql = lint("SELECT COUNT(*) FROM tbl\n\n".into(), "ansi".into(), rules(), None, None)
            .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_pass_expression_operators() {
        let sql = lint(
            "
        select
            field,
            date(field_1) - date(field_2) as diff
        from table
    "
            .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_fail_expression_operators() {
        let sql = fix(
            "
        select
            field,
            date(field_1)-date(field_2) as diff
        from table"
                .into(),
            rules(),
        );

        assert_eq!(
            sql,
            "
        select
            field,
            date(field_1) - date(field_2) as diff
        from table"
        );
    }

    #[test]
    fn test_pass_newline_1() {
        let sql = lint("SELECT 1\n\n+ 2".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_pass_newline_2() {
        let sql = lint("SELECT 1\n\t+ 2".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_pass_newline_3() {
        let sql = lint("SELECT 1\n    + 2".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_pass_sign_indicators() {
        let sql = lint("SELECT 1, +2, -4".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn test_pass_tilde() {
        let sql = lint("SELECT ~1".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    fn fail_simple() {
        let sql = fix("SELECT 1+2".into(), rules());
        assert_eq!(sql, "SELECT 1 + 2");
    }

    // configs: core: dialect: bigquery
    #[test]
    #[ignore = "bigquery"]
    fn pass_bigquery_hyphen() {
        let sql = lint(
            "SELECT col_foo FROM foo-bar.foo.bar".into(),
            "bigquery".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    // LT01-trailing.yml

    #[test]
    fn test_fail_trailing_whitespace() {
        let sql = fix("SELECT 1     \n".into(), rules());
        assert_eq!(sql, "SELECT 1\n");
    }

    #[test]
    fn test_fail_trailing_whitespace_on_initial_blank_line() {
        let sql = fix(" \nSELECT 1     \n".into(), rules());
        assert_eq!(sql, "\nSELECT 1\n");
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_pass_trailing_whitespace_before_template_code() {
        let sql = lint(
            "
        SELECT
            {% for elem in [\"a\", \"b\"] %}
            {{ elem }},
            {% endfor %}
            0
    "
            .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_fail_trailing_whitespace_and_whitespace_control() {
        let sql = fix("{%- set temp = 'temp' -%}\n\nSELECT\n    1, \n    2,\n".into(), rules());
        assert_eq!(sql, "{%- set temp = 'temp' -%}\n\nSELECT\n    1,\n    2,\n");
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_pass_macro_trailing() {
        let sql = lint(
            "
        {% macro foo(bar) %}
            {{bar}}
        {% endmacro %}

        with base as (
            select
                a,
                b,
                {{ foo(1) }} as c
            from tblb
        )

        select *
        from tbl
    "
            .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(sql, &[]);
    }
}
