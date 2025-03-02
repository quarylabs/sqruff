use ahash::AHashMap;
use itertools::{Itertools, enumerate};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::edit_type::EditType;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder, Tables};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

struct SelectTargetsInfo {
    select_idx: Option<usize>,
    first_new_line_idx: Option<usize>,
    first_select_target_idx: Option<usize>,

    #[allow(dead_code)]
    first_whitespace_idx: Option<usize>,
    comment_after_select_idx: Option<usize>,
    select_targets: Segments,
    from_segment: Option<ErasedSegment>,
    pre_from_whitespace: Segments,
}

#[derive(Debug, Clone)]
pub struct RuleLT09 {
    wildcard_policy: String,
}

impl Rule for RuleLT09 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT09 {
            wildcard_policy: _config["wildcard_policy"].as_string().unwrap().to_owned(),
        }
        .erased())
    }
    fn name(&self) -> &'static str {
        "layout.select_targets"
    }

    fn description(&self) -> &'static str {
        "Select targets should be on a new line unless there is only one select target."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Multiple select targets on the same line.

```sql
select a, b
from foo;

-- Single select target on its own line.

SELECT
    a
FROM foo;
```

**Best practice**

Multiple select targets each on their own line.

```sql
select
    a,
    b
from foo;

-- Single select target on the same line as the ``SELECT``
-- keyword.

SELECT a
FROM foo;

-- When select targets span multiple lines, however they
-- can still be on a new line.

SELECT
    SUM(
        1 + SUM(
            2 + 3
        )
    ) AS col
FROM test_table;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let select_targets_info = Self::get_indexes(context);
        let select_clause = FunctionalContext::new(context).segment();

        let wildcards = select_clause
            .children(Some(|sp| sp.is_type(SyntaxKind::SelectClauseElement)))
            .children(Some(|sp| sp.is_type(SyntaxKind::WildcardExpression)));

        let has_wildcard = !wildcards.is_empty();

        if select_targets_info.select_targets.len() == 1
            && (!has_wildcard || self.wildcard_policy == "single")
        {
            return self.eval_single_select_target_element(select_targets_info, context);
        } else if !select_targets_info.select_targets.is_empty() {
            return self.eval_multiple_select_target_elements(
                context.tables,
                select_targets_info,
                context.segment.clone(),
            );
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectClause]) }).into()
    }
}

impl RuleLT09 {
    fn get_indexes(context: &RuleContext) -> SelectTargetsInfo {
        let children = FunctionalContext::new(context).segment().children(None);

        let select_targets = children.select(
            Some(|segment: &ErasedSegment| segment.is_type(SyntaxKind::SelectClauseElement)),
            None,
            None,
            None,
        );

        let first_select_target_idx = select_targets
            .get(0, None)
            .and_then(|it| children.find(&it));

        let selects = children.select(
            Some(|segment: &ErasedSegment| segment.is_keyword("select")),
            None,
            None,
            None,
        );

        let select_idx =
            (!selects.is_empty()).then(|| children.find(&selects.get(0, None).unwrap()).unwrap());

        let newlines = children.select(
            Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Newline)),
            None,
            None,
            None,
        );

        let first_new_line_idx =
            (!newlines.is_empty()).then(|| children.find(&newlines.get(0, None).unwrap()).unwrap());
        let mut comment_after_select_idx = None;

        if !newlines.is_empty() {
            let comment_after_select = children.select(
                Some(|seg: &ErasedSegment| seg.is_type(SyntaxKind::Comment)),
                Some(|seg| {
                    seg.is_type(SyntaxKind::Comment)
                        | seg.is_type(SyntaxKind::Whitespace)
                        | seg.is_meta()
                }),
                selects.get(0, None).as_ref(),
                newlines.get(0, None).as_ref(),
            );

            if !comment_after_select.is_empty() {
                comment_after_select_idx = (!comment_after_select.is_empty()).then(|| {
                    children
                        .find(&comment_after_select.get(0, None).unwrap())
                        .unwrap()
                });
            }
        }

        let mut first_whitespace_idx = None;
        if let Some(first_new_line_idx) = first_new_line_idx {
            let segments_after_first_line = children.select(
                Some(|seg: &ErasedSegment| seg.is_type(SyntaxKind::Whitespace)),
                None,
                Some(&children[first_new_line_idx]),
                None,
            );

            if !segments_after_first_line.is_empty() {
                first_whitespace_idx =
                    children.find(&segments_after_first_line.get(0, None).unwrap());
            }
        }

        let siblings_post = FunctionalContext::new(context).siblings_post();
        let from_segment = siblings_post
            .find_first(Some(|seg: &ErasedSegment| {
                seg.is_type(SyntaxKind::FromClause)
            }))
            .find_first::<fn(&ErasedSegment) -> bool>(None)
            .get(0, None);
        let pre_from_whitespace = siblings_post.select(
            Some(|seg: &ErasedSegment| seg.is_type(SyntaxKind::Whitespace)),
            None,
            None,
            from_segment.as_ref(),
        );

        SelectTargetsInfo {
            select_idx,
            first_new_line_idx,
            first_select_target_idx,
            first_whitespace_idx,
            comment_after_select_idx,
            select_targets,
            from_segment,
            pre_from_whitespace,
        }
    }

    fn eval_multiple_select_target_elements(
        &self,
        tables: &Tables,
        select_targets_info: SelectTargetsInfo,
        segment: ErasedSegment,
    ) -> Vec<LintResult> {
        let mut fixes = Vec::new();

        for (i, select_target) in enumerate(select_targets_info.select_targets.iter()) {
            let base_segment = if i == 0 {
                segment.clone()
            } else {
                select_targets_info.select_targets[i - 1].clone()
            };

            if let Some((_, _)) = base_segment
                .get_position_marker()
                .zip(select_target.get_position_marker())
                .filter(|(a, b)| a.working_line_no == b.working_line_no)
            {
                let mut start_seg = select_targets_info.select_idx.unwrap();
                let modifier =
                    segment.child(const { &SyntaxSet::new(&[SyntaxKind::SelectClauseModifier]) });

                if let Some(modifier) = modifier {
                    start_seg = segment
                        .segments()
                        .iter()
                        .position(|it| it == &modifier)
                        .unwrap();
                }

                let segments = segment.segments();

                let start = if i == 0 {
                    &segments[start_seg]
                } else {
                    &select_targets_info.select_targets[i - 1]
                };

                let start_position = segments.iter().position(|it| it == start).unwrap();
                let ws_to_delete = segments[start_position + 1..]
                    .iter()
                    .take_while(|it| {
                        it.is_type(SyntaxKind::Whitespace)
                            | it.is_type(SyntaxKind::Comma)
                            | it.is_meta()
                    })
                    .filter(|it| it.is_type(SyntaxKind::Whitespace));

                fixes.extend(ws_to_delete.cloned().map(LintFix::delete));
                fixes.push(LintFix::create_before(
                    select_target.clone(),
                    vec![SegmentBuilder::newline(tables.next_id(), "\n")],
                ));
            }

            if let Some(from_segment) = &select_targets_info.from_segment {
                if i + 1 == select_targets_info.select_targets.len()
                    && select_target.get_position_marker().unwrap().working_line_no
                        == from_segment.get_position_marker().unwrap().working_line_no
                {
                    fixes.extend(
                        select_targets_info
                            .pre_from_whitespace
                            .clone()
                            .into_iter()
                            .map(LintFix::delete),
                    );

                    fixes.push(LintFix::create_before(
                        from_segment.clone(),
                        vec![SegmentBuilder::newline(tables.next_id(), "\n")],
                    ));
                }
            }
        }

        if !fixes.is_empty() {
            return vec![LintResult::new(segment.into(), fixes, None, None)];
        }

        Vec::new()
    }

    fn eval_single_select_target_element(
        &self,
        select_targets_info: SelectTargetsInfo,
        context: &RuleContext,
    ) -> Vec<LintResult> {
        let select_clause = FunctionalContext::new(context).segment();
        let parent_stack = &context.parent_stack;

        if !(select_targets_info.select_idx < select_targets_info.first_new_line_idx
            && select_targets_info.first_new_line_idx < select_targets_info.first_select_target_idx)
        {
            return Vec::new();
        }

        let select_children = select_clause.children(None);
        let mut modifier = select_children.find_first(Some(|seg: &ErasedSegment| {
            seg.is_type(SyntaxKind::SelectClauseModifier)
        }));

        if select_children[select_targets_info.first_select_target_idx.unwrap()]
            .descendant_type_set()
            .contains(SyntaxKind::Newline)
        {
            return Vec::new();
        }

        let mut insert_buff = vec![
            SegmentBuilder::whitespace(context.tables.next_id(), " "),
            select_children[select_targets_info.first_select_target_idx.unwrap()].clone(),
        ];

        if !modifier.is_empty()
            && select_children.index(&modifier.get(0, None).unwrap())
                < select_targets_info.first_new_line_idx
        {
            modifier = Segments::from_vec(Vec::new(), None);
        }

        let mut fixes = vec![LintFix::delete(
            select_children[select_targets_info.first_select_target_idx.unwrap()].clone(),
        )];

        let start_idx = if !modifier.is_empty() {
            let buff = std::mem::take(&mut insert_buff);

            insert_buff = vec![
                SegmentBuilder::whitespace(context.tables.next_id(), " "),
                modifier[0].clone(),
            ];

            insert_buff.extend(buff);

            let modifier_idx = select_children
                .index(&modifier.get(0, None).unwrap())
                .unwrap();

            if select_children.len() > modifier_idx + 1
                && select_children[modifier_idx + 2].is_whitespace()
            {
                fixes.push(LintFix::delete(select_children[modifier_idx + 2].clone()));
            }

            fixes.push(LintFix::delete(modifier[0].clone()));

            modifier_idx
        } else {
            select_targets_info.first_select_target_idx.unwrap()
        };

        if !parent_stack.is_empty()
            && parent_stack
                .last()
                .unwrap()
                .is_type(SyntaxKind::SelectStatement)
        {
            let select_stmt = parent_stack.last().unwrap();
            let select_clause_idx = select_stmt
                .segments()
                .iter()
                .position(|it| it.clone() == select_clause.get(0, None).unwrap())
                .unwrap();
            let after_select_clause_idx = select_clause_idx + 1;

            let fixes_for_move_after_select_clause =
                |fixes: &mut Vec<LintFix>,
                 stop_seg: ErasedSegment,
                 delete_segments: Option<Segments>,
                 add_newline: bool| {
                    let start_seg = if !modifier.is_empty() {
                        modifier[0].clone()
                    } else {
                        select_children[select_targets_info.first_new_line_idx.unwrap()].clone()
                    };

                    let move_after_select_clause = select_children
                        .select::<fn(&ErasedSegment) -> bool>(
                            None,
                            None,
                            (&start_seg).into(),
                            (&stop_seg).into(),
                        );
                    let mut local_fixes = Vec::new();
                    let mut all_deletes = fixes
                        .iter()
                        .filter(|fix| fix.edit_type == EditType::Delete)
                        .map(|fix| fix.anchor.clone())
                        .collect_vec();
                    for seg in delete_segments.unwrap_or_default() {
                        fixes.push(LintFix::delete(seg.clone()));
                        all_deletes.push(seg);
                    }

                    let new_fixes = move_after_select_clause
                        .iter()
                        .filter(|it| !all_deletes.contains(it))
                        .cloned()
                        .map(LintFix::delete);
                    local_fixes.extend(new_fixes);

                    if !move_after_select_clause.is_empty() || add_newline {
                        local_fixes.push(LintFix::create_after(
                            select_clause[0].clone(),
                            if add_newline {
                                vec![SegmentBuilder::newline(context.tables.next_id(), "\n")]
                            } else {
                                vec![]
                            }
                            .into_iter()
                            .chain(move_after_select_clause)
                            .collect_vec(),
                            None,
                        ));
                    }

                    local_fixes
                };

            if select_stmt.segments().len() > after_select_clause_idx {
                if select_stmt.segments()[after_select_clause_idx].is_type(SyntaxKind::Newline) {
                    let to_delete = select_children
                        .reversed()
                        .select::<fn(&ErasedSegment) -> bool>(
                            None,
                            Some(|seg| seg.is_type(SyntaxKind::Whitespace)),
                            (&select_children[start_idx]).into(),
                            None,
                        );

                    if !to_delete.is_empty() {
                        let delete_last_newline = select_children[start_idx - to_delete.len() - 1]
                            .is_type(SyntaxKind::Newline);

                        if delete_last_newline {
                            fixes.push(LintFix::delete(
                                select_stmt.segments()[after_select_clause_idx].clone(),
                            ));
                        }

                        let new_fixes = fixes_for_move_after_select_clause(
                            &mut fixes,
                            to_delete.last().unwrap().clone(),
                            to_delete.into(),
                            true,
                        );
                        fixes.extend(new_fixes);
                    }
                } else if select_stmt.segments()[after_select_clause_idx]
                    .is_type(SyntaxKind::Whitespace)
                {
                    fixes.push(LintFix::delete(
                        select_stmt.segments()[after_select_clause_idx].clone(),
                    ));

                    let new_fixes = fixes_for_move_after_select_clause(
                        &mut fixes,
                        select_children[select_targets_info.first_select_target_idx.unwrap()]
                            .clone(),
                        None,
                        true,
                    );

                    fixes.extend(new_fixes);
                } else if select_stmt.segments()[after_select_clause_idx]
                    .is_type(SyntaxKind::Dedent)
                {
                    let start_seg = if select_clause_idx == 0 {
                        select_children.last().unwrap()
                    } else {
                        &select_children[select_clause_idx - 1]
                    };

                    let to_delete = select_children
                        .reversed()
                        .select::<fn(&ErasedSegment) -> bool>(
                            None,
                            Some(|it| it.is_type(SyntaxKind::Whitespace)),
                            Some(start_seg),
                            None,
                        );

                    if !to_delete.is_empty() {
                        let add_newline =
                            to_delete.iter().any(|it| it.is_type(SyntaxKind::Newline));
                        let local_fixes = fixes_for_move_after_select_clause(
                            &mut fixes,
                            to_delete.last().unwrap().clone(),
                            to_delete.into(),
                            add_newline,
                        );
                        fixes.extend(local_fixes);
                    }
                } else {
                    let local_fixes = fixes_for_move_after_select_clause(
                        &mut fixes,
                        select_children[select_targets_info.first_select_target_idx.unwrap()]
                            .clone(),
                        None,
                        true,
                    );
                    fixes.extend(local_fixes);
                }
            }
        }

        if select_targets_info.comment_after_select_idx.is_none() {
            fixes.push(LintFix::replace(
                select_children[select_targets_info.first_new_line_idx.unwrap()].clone(),
                insert_buff,
                None,
            ));
        }

        vec![LintResult::new(
            select_clause.get(0, None).unwrap().clone().into(),
            fixes,
            None,
            None,
        )]
    }
}

impl Default for RuleLT09 {
    fn default() -> Self {
        Self {
            wildcard_policy: "single".into(),
        }
    }
}
