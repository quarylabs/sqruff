use ahash::AHashMap;
use itertools::Itertools;
use smol_str::ToSmolStr;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder, Tables};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased as _, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;
use crate::utils::reflow::reindent::{IndentUnit, construct_single_indent};

#[derive(Clone, Debug, Default)]
pub struct RuleST04;

impl Rule for RuleST04 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST04.erased())
    }

    fn name(&self) -> &'static str {
        "structure.nested_case"
    }

    fn description(&self) -> &'static str {
        "Nested ``CASE`` statement in ``ELSE`` clause could be flattened."
    }

    fn long_description(&self) -> &'static str {
        r"
## Anti-pattern

In this example, the outer `CASE`'s `ELSE` is an unnecessary, nested `CASE`.

```sql
SELECT
  CASE
    WHEN species = 'Cat' THEN 'Meow'
    ELSE
    CASE
       WHEN species = 'Dog' THEN 'Woof'
    END
  END as sound
FROM mytable
```

## Best practice

Move the body of the inner `CASE` to the end of the outer one.

```sql
SELECT
  CASE
    WHEN species = 'Cat' THEN 'Meow'
    WHEN species = 'Dog' THEN 'Woof'
  END AS sound
FROM mytable
```
"
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(context).segment();
        let case1_children = segment.children_all();
        let case1_keywords =
            case1_children.find_first_where(|it: &ErasedSegment| it.is_keyword("CASE"));
        let case1_first_case = case1_keywords.first().unwrap();
        let case1_when_list = case1_children.find_first_where(|it: &ErasedSegment| {
            matches!(
                it.get_type(),
                SyntaxKind::WhenClause | SyntaxKind::ElseClause
            )
        });
        let case1_first_when = case1_when_list.first().unwrap();
        let when_clause_list =
            case1_children.find_last_where(|it| it.is_type(SyntaxKind::WhenClause));
        let case1_last_when = when_clause_list.first();
        let case1_else_clause =
            case1_children.find_last_where(|it| it.is_type(SyntaxKind::ElseClause));
        let case1_else_expressions =
            case1_else_clause.children_where(|it| it.is_type(SyntaxKind::Expression));
        let case2 = case1_else_expressions.children_all();
        let case2_children = case2.children_all();
        let case2_case_list =
            case2_children.find_first_where(|it: &ErasedSegment| it.is_keyword("CASE"));
        let case2_first_case = case2_case_list.first();
        let case2_when_list = case2_children.find_first_where(|it: &ErasedSegment| {
            matches!(
                it.get_type(),
                SyntaxKind::WhenClause | SyntaxKind::ElseClause
            )
        });
        let case2_first_when = case2_when_list.first();

        let Some(case1_last_when) = case1_last_when else {
            return Vec::new();
        };
        if case1_else_expressions.len() > 1 || case2.len() > 1 || case2.is_empty() {
            return Vec::new();
        }

        // Check if case2 actually contains a CASE expression
        // If there's no nested CASE, we shouldn't proceed with flattening
        let Some(case2_first_case) = case2_first_case else {
            return Vec::new();
        };

        // Additionally check that case2 is actually a CASE expression
        if !case2.any_match(|seg: &ErasedSegment| seg.is_type(SyntaxKind::CaseExpression)) {
            return Vec::new();
        }

        let x1 = segment
            .children_where(|it| it.is_code())
            .between_exclusive(case1_first_case, case1_first_when)
            .into_iter()
            .map(|it| it.raw().to_smolstr());

        let code2 = case2.children_where(|it| it.is_code());
        let range2 = if let Some(stop) = case2_first_when {
            code2.between_exclusive(case2_first_case, stop)
        } else {
            code2.after(case2_first_case)
        };
        let x2 = range2.into_iter().map(|it| it.raw().to_smolstr());

        if x1.ne(x2) {
            return Vec::new();
        }

        let case1_else_clause_seg = case1_else_clause.first().unwrap();

        let case1_to_delete =
            case1_children.between_exclusive(case1_last_when, case1_else_clause_seg);

        let comments = case1_to_delete.find_last_where(|it: &ErasedSegment| it.is_comment());
        let after_last_comment_index = comments
            .first()
            .and_then(|comment| case1_to_delete.iter().position(|it| it == comment))
            .map_or(0, |n| n + 1);

        let case1_comments_to_restore =
            if let Some(stop_seg) = case1_to_delete.base.get(after_last_comment_index) {
                case1_to_delete.before(stop_seg)
            } else {
                case1_to_delete.clone()
            };
        let after_else_comment = {
            let children = case1_else_clause.children_all();
            let range = if let Some(stop) = case1_else_expressions.first() {
                children.before(stop)
            } else {
                children
            };
            range.filter(|it: &ErasedSegment| {
                matches!(
                    it.get_type(),
                    SyntaxKind::Newline
                        | SyntaxKind::InlineComment
                        | SyntaxKind::BlockComment
                        | SyntaxKind::Comment
                        | SyntaxKind::Whitespace
                )
            })
        };

        let mut fixes = case1_to_delete
            .into_iter()
            .map(LintFix::delete)
            .collect_vec();

        let tab_space_size = context.config.raw["indentation"]["tab_space_size"]
            .as_int()
            .unwrap() as usize;
        let indent_unit = context.config.raw["indentation"]["indent_unit"]
            .as_string()
            .unwrap();
        let indent_unit = IndentUnit::from_type_and_size(indent_unit, tab_space_size);

        let when_indent_str = indentation(&case1_children, case1_last_when, indent_unit);
        let end_indent_str = indentation(&case1_children, case1_first_case, indent_unit);

        let nested_clauses = case2.children_where(|it: &ErasedSegment| {
            matches!(
                it.get_type(),
                SyntaxKind::WhenClause
                    | SyntaxKind::ElseClause
                    | SyntaxKind::Newline
                    | SyntaxKind::InlineComment
                    | SyntaxKind::BlockComment
                    | SyntaxKind::Comment
                    | SyntaxKind::Whitespace
            )
        });

        let mut segments = case1_comments_to_restore.base;
        segments.append(&mut rebuild_spacing(
            context.tables,
            &when_indent_str,
            after_else_comment,
        ));
        segments.append(&mut rebuild_spacing(
            context.tables,
            &when_indent_str,
            nested_clauses,
        ));

        fixes.push(LintFix::create_after(
            case1_last_when.clone(),
            segments,
            None,
        ));
        fixes.push(LintFix::delete(case1_else_clause_seg.clone()));
        fixes.append(&mut nested_end_trailing_comment(
            context.tables,
            case1_children,
            case1_else_clause_seg,
            &end_indent_str,
        ));

        vec![LintResult::new(case2.first().cloned(), fixes, None, None)]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::CaseExpression]) }).into()
    }
}

fn indentation(
    parent_segments: &Segments,
    segment: &ErasedSegment,
    indent_unit: IndentUnit,
) -> String {
    let leading_whitespace = parent_segments
        .before(segment)
        .reversed()
        .find_first_where(|it: &ErasedSegment| it.is_type(SyntaxKind::Whitespace));
    let seg_indent = parent_segments
        .before(segment)
        .find_last_where(|it| it.is_type(SyntaxKind::Indent));
    let mut indent_level = 1;
    if let Some(segment_indent) = seg_indent
        .last()
        .filter(|segment_indent| segment_indent.is_indent())
    {
        indent_level = segment_indent.indent_val() as usize + 1;
    }

    if let Some(whitespace_seg) = leading_whitespace.first() {
        if !leading_whitespace.is_empty() && whitespace_seg.raw().len() > 1 {
            leading_whitespace
                .iter()
                .map(|seg| seg.raw().to_string())
                .collect::<String>()
        } else {
            construct_single_indent(indent_unit).repeat(indent_level)
        }
    } else {
        construct_single_indent(indent_unit).repeat(indent_level)
    }
}

fn rebuild_spacing(
    tables: &Tables,
    indent_str: &str,
    nested_clauses: Segments,
) -> Vec<ErasedSegment> {
    let mut buff = Vec::new();

    let mut prior_newline = nested_clauses
        .find_last_where(|it: &ErasedSegment| !it.is_whitespace())
        .any_match(|it: &ErasedSegment| it.is_comment());
    let mut prior_whitespace = String::new();

    for seg in nested_clauses {
        if matches!(
            seg.get_type(),
            SyntaxKind::WhenClause | SyntaxKind::ElseClause
        ) || (prior_newline && seg.is_comment())
        {
            buff.push(SegmentBuilder::newline(tables.next_id(), "\n"));
            buff.push(SegmentBuilder::whitespace(tables.next_id(), indent_str));
            buff.push(seg.clone());
            prior_newline = false;
            prior_whitespace.clear();
        } else if seg.is_type(SyntaxKind::Newline) {
            prior_newline = true;
            prior_whitespace.clear();
        } else if !prior_newline && seg.is_comment() {
            buff.push(SegmentBuilder::whitespace(
                tables.next_id(),
                &prior_whitespace,
            ));
            buff.push(seg.clone());
            prior_newline = false;
            prior_whitespace.clear();
        } else if seg.is_whitespace() {
            prior_whitespace = seg.raw().to_string();
        }
    }

    buff
}

fn nested_end_trailing_comment(
    tables: &Tables,
    case1_children: Segments,
    case1_else_clause_seg: &ErasedSegment,
    end_indent_str: &str,
) -> Vec<LintFix> {
    // Prepend newline spacing to comments on the final nested `END` line.
    let trailing_end = case1_children
        .after(case1_else_clause_seg)
        .take_while(|seg: &ErasedSegment| !seg.is_type(SyntaxKind::Newline));

    let mut fixes = trailing_end
        .take_while(|seg: &ErasedSegment| !seg.is_comment())
        .filter(|seg: &ErasedSegment| seg.is_whitespace())
        .into_iter()
        .map(LintFix::delete)
        .collect_vec();

    if let Some(first_comment) = trailing_end
        .find_first_where(|seg: &ErasedSegment| seg.is_comment())
        .first()
    {
        let segments = vec![
            SegmentBuilder::newline(tables.next_id(), "\n"),
            SegmentBuilder::whitespace(tables.next_id(), end_indent_str),
        ];
        fixes.push(LintFix::create_before(first_comment.clone(), segments));
    }

    fixes
}
