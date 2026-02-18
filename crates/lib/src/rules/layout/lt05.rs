use hashbrown::{HashMap, HashSet};
use itertools::enumerate;
use sqruff_lib_core::dialects::syntax::SyntaxKind;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Debug, Default, Clone)]
pub struct RuleLT05 {
    ignore_comment_lines: bool,
    ignore_comment_clauses: bool,
}

impl Rule for RuleLT05 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT05 {
            ignore_comment_lines: config["ignore_comment_lines"].as_bool().unwrap(),
            ignore_comment_clauses: config["ignore_comment_clauses"].as_bool().unwrap(),
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
FROM my_table
```"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }
    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut results = ReflowSequence::from_root(&context.segment, context.config)
            .break_long_lines(context.tables)
            .results();

        let mut to_remove = HashSet::new();

        if self.ignore_comment_lines {
            let raw_segments = context.segment.get_raw_segments();
            for (res_idx, res) in enumerate(&results) {
                if res.anchor.as_ref().unwrap().is_type(SyntaxKind::Comment)
                    || res
                        .anchor
                        .as_ref()
                        .unwrap()
                        .is_type(SyntaxKind::InlineComment)
                {
                    to_remove.insert(res_idx);
                    continue;
                }

                let pos_marker = res.anchor.as_ref().unwrap().get_position_marker().unwrap();
                let raw_idx = raw_segments
                    .iter()
                    .position(|it| it == res.anchor.as_ref().unwrap())
                    .unwrap();

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
                let raw_idx = raw_segments
                    .iter()
                    .position(|it| it == res.anchor.as_ref().unwrap())
                    .unwrap();

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

        // Sort indices in reversed order to avoid index shifting issues when removing.
        // Remove items from the end of the vector first.
        let mut to_remove_vec: Vec<usize> = to_remove.into_iter().collect();
        to_remove_vec.sort_by(|a, b| b.cmp(a));
        for idx in to_remove_vec {
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
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    /// Verifies that moving a trailing comment before its code line doesn't
    /// merge it with the code, which would produce broken SQL since
    /// everything after `--` becomes part of the comment.
    #[test]
    fn test_comment_not_merged_with_next_line() {
        let sql = "\
SELECT
    COALESCE(
        REGEXP_EXTRACT(project_id, '^foo-bar-(.+)$'),                -- foo-bar-baz -> baz
        REGEXP_EXTRACT(project_id, '^qux-(.+)$')                     -- qux-corge -> corge
    ) AS result
FROM t
";
        let linter = Linter::new(FluffConfig::default(), None, None, true, None);
        let result = linter.lint_string(sql, None, true).unwrap();
        let fixed = result.fix_string();

        for line in fixed.lines() {
            if let Some(comment_pos) = line.find("--") {
                let before_comment = line[..comment_pos].trim();
                if before_comment.is_empty() {
                    assert!(
                        !after_double_dash_has_code(line, comment_pos),
                        "Comment merged with code on line: {line}"
                    );
                }
            }
        }
    }

    fn after_double_dash_has_code(line: &str, comment_pos: usize) -> bool {
        let after_comment = &line[comment_pos..];
        after_comment.contains("REGEXP_EXTRACT")
            || after_comment.contains("SELECT")
            || after_comment.contains("FROM")
    }
}
