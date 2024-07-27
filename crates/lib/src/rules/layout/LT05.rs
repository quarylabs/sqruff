use ahash::{AHashMap, AHashSet};
use itertools::enumerate;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::dialects::SyntaxKind;
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Debug, Default, Clone)]
pub struct RuleLT05 {
    ignore_comment_lines: bool,
    ignore_comment_clauses: bool,
}

impl Rule for RuleLT05 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT05 {
            ignore_comment_lines: _config["ignore_comment_lines"].as_bool().unwrap(),
            ignore_comment_clauses: _config["ignore_comment_lines"].as_bool().unwrap(),
        }
        .erased())
    }
    fn name(&self) -> &'static str {
        "layout.long_lines"
    }

    fn description(&self) -> &'static str {
        "Line is too long."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the line is too long.

```sql
SELECT
    my_function(col1 + col2, arg2, arg3) over (partition by col3, col4 order by col5 rows between unbounded preceding and current row) as my_relatively_long_alias,
    my_other_function(col6, col7 + col8, arg4) as my_other_relatively_long_alias,
    my_expression_function(col6, col7 + col8, arg4) = col9 + col10 as another_relatively_long_alias
FROM my_table
```

**Best practice**

Wraps the line to be within the maximum line length.

```sql
SELECT
    my_function(col1 + col2, arg2, arg3)
        over (
            partition by col3, col4
            order by col5 rows between unbounded preceding and current row
        )
        as my_relatively_long_alias,
    my_other_function(col6, col7 + col8, arg4)
        as my_other_relatively_long_alias,
    my_expression_function(col6, col7 + col8, arg4)
    = col9 + col10 as another_relatively_long_alias
FROM my_table"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }
    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut results =
            ReflowSequence::from_root(context.segment.clone(), context.config.unwrap())
                .break_long_lines()
                .results();

        let mut to_remove = AHashSet::new();

        if self.ignore_comment_lines {
            let raw_segments = context.segment.get_raw_segments();
            for (res_idx, res) in enumerate(&results) {
                if res.anchor.as_ref().unwrap().is_type(SyntaxKind::Comment)
                    || res.anchor.as_ref().unwrap().is_type(SyntaxKind::InlineComment)
                {
                    to_remove.insert(res_idx);
                    continue;
                }

                let pos_marker = res.anchor.as_ref().unwrap().get_position_marker().unwrap();
                let raw_idx =
                    raw_segments.iter().position(|it| it == res.anchor.as_ref().unwrap()).unwrap();

                for seg in &raw_segments[raw_idx..] {
                    if seg.get_position_marker().unwrap().working_line_no
                        != pos_marker.working_line_no
                    {
                        break;
                    }

                    if seg.is_type(SyntaxKind::Comment) || seg.is_type(SyntaxKind::InlineComment) {
                        to_remove.insert(res_idx);
                        break;
                    } else if seg.is_type(SyntaxKind::Placeholder) {
                        unimplemented!()
                    }
                }
            }
        }

        if self.ignore_comment_clauses {
            let raw_segments = context.segment.get_raw_segments();
            for (res_idx, res) in enumerate(&results) {
                let raw_idx =
                    raw_segments.iter().position(|it| it == res.anchor.as_ref().unwrap()).unwrap();

                for seg in &raw_segments[raw_idx..] {
                    if seg.get_position_marker().unwrap().working_line_no
                        != res
                            .anchor
                            .as_ref()
                            .unwrap()
                            .get_position_marker()
                            .unwrap()
                            .working_line_no
                    {
                        break;
                    }

                    let mut is_break = false;

                    for ps in context.segment.path_to(seg) {
                        if ps.segment.is_type(SyntaxKind::CommentClause)
                            || ps.segment.is_type(SyntaxKind::CommentEqualsClause)
                        {
                            let line_pos =
                                ps.segment.get_position_marker().unwrap().working_line_pos;
                            if (line_pos as i32)
                                < context
                                    .config
                                    .as_ref()
                                    .unwrap()
                                    .get("max_line_length", "core")
                                    .as_int()
                                    .unwrap()
                            {
                                to_remove.insert(res_idx);
                                is_break = true;
                                break;
                            }
                        }
                    }

                    if is_break {
                        break;
                    } else {
                        continue;
                    }
                }
            }
        }

        for idx in to_remove {
            results.remove(idx);
        }

        results
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::RuleLT05;
    use crate::api::simple::get_simple_config;
    use crate::core::config::Value;
    use crate::core::errors::SQLBaseError;
    use crate::core::linter::linter::Linter;
    use crate::core::linter::linting_result::LintingResult;
    use crate::core::rules::base::{Erased, ErasedRule};

    fn rules(ignore_comment_lines: bool, ignore_comment_clauses: bool) -> Vec<ErasedRule> {
        vec![RuleLT05 { ignore_comment_lines, ignore_comment_clauses }.erased()]
    }

    #[derive(Default)]
    struct Options {
        max_line_length: Option<usize>,
        trailing_comments: Option<&'static str>,
        ignore_comment_lines: bool,
        ignore_comment_clauses: bool,
        dialect: Option<&'static str>,
    }

    fn lint_inner(sql: &'static str, options: Options) -> LintingResult {
        let mut cfg =
            get_simple_config(options.dialect.map(ToOwned::to_owned), None, None, None).unwrap();

        if let Some(max_line_length) = options.max_line_length {
            cfg.raw
                .get_mut("core")
                .unwrap()
                .as_map_mut()
                .unwrap()
                .insert("max_line_length".into(), Value::Int(max_line_length as i32));
        }

        if let Some(trailing_comments) = options.trailing_comments {
            cfg.raw
                .get_mut("indentation")
                .unwrap()
                .as_map_mut()
                .unwrap()
                .insert("trailing_comments".into(), Value::String(trailing_comments.into()));
        }

        cfg.reload_reflow();

        let mut linter = Linter::new(cfg, None, None);
        linter.lint_string_wrapped(
            sql,
            None,
            Some(true),
            rules(options.ignore_comment_lines, options.ignore_comment_clauses),
        )
    }

    fn lint(sql: &'static str, options: Options) -> Vec<SQLBaseError> {
        let mut result = lint_inner(sql, options);
        std::mem::take(&mut result.paths[0].files[0].violations)
    }

    fn fix(sql: &'static str, options: Options) -> String {
        let mut result = lint_inner(sql, options);
        std::mem::take(&mut result.paths[0].files[0]).fix_string()
    }

    #[test]
    fn test_pass_line_too_long_config_override() {
        let actual = fix(
            "SELECT COUNT(*) FROM tbl\n",
            Options { max_line_length: 30.into(), ..Default::default() },
        );
        assert_eq!(actual, "SELECT COUNT(*) FROM tbl\n");
    }

    #[test]
    fn test_fail_line_too_long_with_comments_1() {
        let actual = fix(
            "SELECT 1 -- Some Comment\n",
            Options { max_line_length: 18.into(), ..Default::default() },
        );
        assert_eq!(actual, "-- Some Comment\nSELECT 1\n");
    }

    #[test]
    fn test_fail_line_too_long_with_comments_1_after() {
        let actual = fix(
            "SELECT 1 -- Some Comment\n",
            Options {
                max_line_length: 17.into(),
                trailing_comments: "after".into(),
                ..Default::default()
            },
        );
        assert_eq!(actual, "SELECT 1\n-- Some Comment\n");
    }

    #[test]
    fn test_fail_line_too_long_with_comments_1_no_newline() {
        let actual = fix(
            "SELECT 1 -- Some Comment",
            Options { max_line_length: 18.into(), ..Default::default() },
        );
        assert_eq!(actual, "-- Some Comment\nSELECT 1");
    }

    #[test]
    fn test_fail_line_too_long_with_comments_2() {
        let actual = fix(
            "    SELECT COUNT(*) FROM tbl\n",
            Options { max_line_length: 20.into(), ..Default::default() },
        );
        assert_eq!(actual, "    SELECT COUNT(*)\nFROM tbl\n");
    }

    #[test]
    fn test_fail_line_too_long_with_comments_3() {
        let actual = fix(
            "SELECT COUNT(*) FROM tbl -- Some Comment\n",
            Options { max_line_length: 18.into(), ..Default::default() },
        );
        assert_eq!(actual, "-- Some Comment\nSELECT COUNT(*)\nFROM tbl\n");
    }

    #[test]
    fn test_fail_line_too_long_with_comments_4() {
        let fail_str = "SELECT\n    c1\n    ,--  the \"y variable\" and uses_small_subject_line \
                        to be the \"x variable\" in terms of the regression line.\n    c2";
        let result = lint(fail_str, Options { max_line_length: 80.into(), ..Default::default() });

        assert_eq!(result[0].line_no, 3);
        assert_eq!(result[0].line_pos, 5);
        assert_eq!(result[0].desc(), "Line is too long (109 > 80).");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_pass_line_too_long_with_comments_ignore_comment_lines() {
        let pass_str = r#"
SELECT
c1
,--  the "y variable" and uses_small_subject_line to be the "x variable" in terms of the regression line.
c2"#;

        let options =
            Options { max_line_length: Some(80), ignore_comment_lines: true, ..Default::default() };

        let result = lint(pass_str, options);
        assert_eq!(result, []);
    }

    #[test]
    fn test_fail_line_too_long_only_comments() {
        let fail_str = "-- Some really long comments on their own line\n\nSELECT 1";
        let result = lint(fail_str, Options { max_line_length: 18.into(), ..Default::default() });

        assert_eq!(result[0].line_no, 1);
        assert_eq!(result[0].line_pos, 1);
        assert_eq!(result[0].desc(), "Line is too long (46 > 18).");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_fail_line_too_long_handling_indents() {
        let actual =
            fix("SELECT 12345\n", Options { max_line_length: 10.into(), ..Default::default() });
        assert_eq!(actual, "SELECT\n    12345\n");
    }

    #[test]
    fn test_pass_line_too_long_ignore_comments_true() {
        let pass_str = "SELECT 1\n-- Some long comment over 10 characters\n";
        let options =
            Options { max_line_length: Some(10), ignore_comment_lines: true, ..Default::default() };
        let result = lint(pass_str, options);
        assert_eq!(result, []);
    }

    #[test]
    fn test_pass_line_too_long_ignore_comments_false() {
        let fail_str = "SELECT 1\n-- Some long comment over 10 characters\n";
        let options = Options {
            max_line_length: Some(10),
            ignore_comment_lines: false,
            ..Default::default()
        };
        let result = lint(fail_str, options);
        assert_eq!(result[0].line_no, 2);
        assert_eq!(result[0].line_pos, 1);
        assert_eq!(result[0].desc(), "Line is too long (39 > 10).");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_long_functions_and_aliases() {
        let fail_str = r#"
SELECT
    my_function(col1 + col2, arg2, arg3) over (partition by col3, col4 order by col5 rows between unbounded preceding and current row) as my_relatively_long_alias,
    my_other_function(col6, col7 + col8, arg4) as my_other_relatively_long_alias,
    my_expression_function(col6, col7 + col8, arg4) = col9 + col10 as another_relatively_long_alias
FROM my_table"#;

        let fix_str = r#"
SELECT
    my_function(col1 + col2, arg2, arg3)
        over (
            partition by col3, col4
            order by col5 rows between unbounded preceding and current row
        )
        as my_relatively_long_alias,
    my_other_function(col6, col7 + col8, arg4)
        as my_other_relatively_long_alias,
    my_expression_function(col6, col7 + col8, arg4)
    = col9 + col10 as another_relatively_long_alias
FROM my_table"#;

        let result = fix(fail_str, Options::default());
        assert_eq!(result, fix_str);
    }

    #[test]
    fn test_pass_window_function() {
        let sql = r#"
        select
            col,
            rank() over (
                partition by a, b, c
                order by d desc
            ) as rnk
        from foo
    "#;

        let result = lint(sql, Options::default());
        assert_eq!(result, []);
    }

    #[test]
    fn test_pass_ignore_comment_clauses_postgres() {
        let pass_str = r#"
            CREATE TABLE IF NOT EXISTS foo
            ( id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
              name TEXT NOT NULL
            );

            COMMENT ON TABLE foo IS 'Windows Phone 8, however, was never able to overcome a long string of disappointments for Microsoft. ';
        "#;

        let options = Options {
            max_line_length: Some(80),
            ignore_comment_clauses: true,
            dialect: "postgres".into(),
            ..Default::default()
        };

        let result = lint(pass_str, options);
        assert_eq!(result, []);
    }
}
