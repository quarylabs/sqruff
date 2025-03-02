use ahash::{AHashMap, AHashSet};
use itertools::Itertools;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::edit_type::EditType;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder, Tables};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};

#[derive(Default, Clone, Debug)]
pub struct RuleCV06 {
    multiline_newline: bool,
    require_final_semicolon: bool,
}

impl Rule for RuleCV06 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        let multiline_newline = config["multiline_newline"].as_bool().unwrap();
        let require_final_semicolon = config["require_final_semicolon"].as_bool().unwrap();
        Ok(Self {
            multiline_newline,
            require_final_semicolon,
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "convention.terminator"
    }

    fn description(&self) -> &'static str {
        "Statements must end with a semi-colon."
    }

    fn long_description(&self) -> &'static str {
        r"
**Anti-pattern**

A statement is not immediately terminated with a semi-colon. The `•` represents space.

```sql
SELECT
    a
FROM foo

;

SELECT
    b
FROM bar••;
```

**Best practice**

Immediately terminate the statement with a semi-colon.

```sql
SELECT
    a
FROM foo;
```"
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        debug_assert!(context.segment.is_type(SyntaxKind::File));

        let mut results = vec![];
        for (idx, segment) in context.segment.segments().iter().enumerate() {
            let mut res = None;
            if segment.is_type(SyntaxKind::StatementTerminator) {
                // First we can simply handle the case of existing semi-colon alignment.
                // If it's a terminator then we know it's raw.

                res =
                    self.handle_semicolon(context.tables, segment.clone(), context.segment.clone());
            } else if self.require_final_semicolon && idx == context.segment.segments().len() - 1 {
                // Otherwise, handle the end of the file separately.
                res = self.ensure_final_semicolon(context.tables, context.segment.clone());
            }
            if let Some(res) = res {
                results.push(res);
            }
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

impl RuleCV06 {
    // Adjust anchor_segment to not move trailing inline comment.
    //
    // We don't want to move inline comments that are on the same line
    // as the preceding code segment as they could contain noqa instructions.
    fn handle_trailing_inline_comments(
        parent_segment: ErasedSegment,
        anchor_segment: ErasedSegment,
    ) -> ErasedSegment {
        // See if we have a trailing inline comment on the same line as the preceding
        // segment.
        for comment_segment in parent_segment
            .recursive_crawl(
                const {
                    &SyntaxSet::new(&[
                        SyntaxKind::Comment,
                        SyntaxKind::InlineComment,
                        SyntaxKind::BlockComment,
                    ])
                },
                true,
                &SyntaxSet::EMPTY,
                false,
            )
            .iter()
        {
            assert!(comment_segment.get_position_marker().is_some());
            assert!(anchor_segment.get_position_marker().is_some());
            if comment_segment
                .get_position_marker()
                .unwrap()
                .working_line_no
                == anchor_segment
                    .get_position_marker()
                    .unwrap()
                    .working_line_no
                && !comment_segment.is_type(SyntaxKind::BlockComment)
            {
                return comment_segment.clone();
            }
        }
        anchor_segment
    }

    fn is_one_line_statement(parent_segment: ErasedSegment, segment: ErasedSegment) -> bool {
        let statement_segment = parent_segment
            .path_to(&segment)
            .iter()
            .filter(|&it| it.segment.is_type(SyntaxKind::Statement))
            .map(|it| it.segment.clone())
            .next();

        match statement_segment {
            None => false,
            Some(statement_segment) => statement_segment
                .recursive_crawl(
                    const { &SyntaxSet::new(&[SyntaxKind::Newline]) },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                )
                .is_empty(),
        }
    }

    fn handle_semicolon(
        &self,
        tables: &Tables,
        target_segment: ErasedSegment,
        parent_segment: ErasedSegment,
    ) -> Option<LintResult> {
        let info = Self::get_segment_move_context(target_segment.clone(), parent_segment.clone());
        let semicolon_newline = if !info.is_one_line {
            self.multiline_newline
        } else {
            false
        };

        if !semicolon_newline {
            self.handle_semicolon_same_line(tables, target_segment, parent_segment, info)
        } else {
            self.handle_semicolon_newline(tables, target_segment, parent_segment, info)
        }
    }

    fn handle_semicolon_same_line(
        &self,
        tables: &Tables,
        target_segment: ErasedSegment,
        parent_segment: ErasedSegment,
        info: SegmentMoveContext,
    ) -> Option<LintResult> {
        if info.before_segment.is_empty() {
            return None;
        }

        // If preceding segments are found then delete the old
        // semicolon and its preceding whitespace and then insert
        // the semicolon in the correct location.
        let fixes = self.create_semicolon_and_delete_whitespace(
            target_segment,
            parent_segment,
            info.anchor_segment.clone(),
            info.whitespace_deletions,
            vec![
                SegmentBuilder::token(tables.next_id(), ";", SyntaxKind::StatementTerminator)
                    .finish(),
            ],
        );

        Some(LintResult::new(
            Some(info.anchor_segment),
            fixes,
            None,
            None,
        ))
    }

    /// Adjust segments to not move preceding inline comments.
    ///
    /// We don't want to move inline comments that are on the same line
    /// as the preceding code segment as they could contain noqa instructions.
    fn handle_preceding_inline_comments(
        before_segment: Segments,
        anchor_segment: ErasedSegment,
    ) -> (Segments, ErasedSegment) {
        // See if we have a preceding inline comment on the same line as the preceding
        // segment.

        let same_line_comment = before_segment.iter().find(|s| {
            s.is_comment()
                && !s.is_type(SyntaxKind::BlockComment)
                && s.get_position_marker().is_some()
                && s.get_position_marker().unwrap().working_loc().0
                    == anchor_segment
                        .get_raw_segments()
                        .last()
                        .unwrap()
                        .get_position_marker()
                        .unwrap()
                        .working_loc()
                        .0
        });

        // If so then make that our new anchor segment and adjust
        // before_segment accordingly.
        if let Some(same_line_comment) = same_line_comment {
            let anchor_segment = same_line_comment.clone();
            let before_segment = before_segment
                .iter()
                .take_while(|s| *s != same_line_comment)
                .cloned()
                .collect();
            let before_segment = Segments::from_vec(before_segment, None);
            (before_segment, anchor_segment)
        } else {
            (before_segment, anchor_segment)
        }
    }

    fn handle_semicolon_newline(
        &self,
        tables: &Tables,
        target_segment: ErasedSegment,
        parent_segment: ErasedSegment,
        info: SegmentMoveContext,
    ) -> Option<LintResult> {
        // Adjust before_segment and anchor_segment for preceding inline
        // comments. Inline comments can contain noqa logic so we need to add the
        // newline after the inline comment.
        let (before_segment, anchor_segment) = Self::handle_preceding_inline_comments(
            info.before_segment.clone(),
            info.anchor_segment.clone(),
        );

        if before_segment.len() == 1
            && before_segment.all(Some(|segment: &ErasedSegment| {
                segment.is_type(SyntaxKind::Newline)
            }))
        {
            return None;
        }

        // If preceding segment is not a single newline then delete the old
        // semicolon/preceding whitespace and then insert the
        // semicolon in the correct location.
        let anchor_segment =
            Self::handle_trailing_inline_comments(parent_segment.clone(), anchor_segment.clone());
        let fixes = if anchor_segment == target_segment {
            vec![LintFix::replace(
                anchor_segment.clone(),
                vec![
                    SegmentBuilder::whitespace(tables.next_id(), "\n"),
                    SegmentBuilder::token(tables.next_id(), ";", SyntaxKind::StatementTerminator)
                        .finish(),
                ],
                None,
            )]
        } else {
            self.create_semicolon_and_delete_whitespace(
                target_segment,
                parent_segment,
                anchor_segment.clone(),
                info.whitespace_deletions.clone(),
                vec![
                    SegmentBuilder::newline(tables.next_id(), "\n"),
                    SegmentBuilder::token(tables.next_id(), ";", SyntaxKind::StatementTerminator)
                        .finish(),
                ],
            )
        };

        Some(LintResult::new(Some(anchor_segment), fixes, None, None))
    }

    fn create_semicolon_and_delete_whitespace(
        &self,
        target_segment: ErasedSegment,
        parent_segment: ErasedSegment,
        anchor_segment: ErasedSegment,
        mut whitespace_deletions: Segments,
        create_segments: Vec<ErasedSegment>,
    ) -> Vec<LintFix> {
        let anchor_segment = choose_anchor_segment(
            &parent_segment,
            EditType::CreateAfter,
            &anchor_segment,
            true,
        );

        let mut lintfix_fn: fn(
            ErasedSegment,
            Vec<ErasedSegment>,
            Option<Vec<ErasedSegment>>,
        ) -> LintFix = LintFix::create_after;
        if AHashSet::from_iter(whitespace_deletions.base.clone()).contains(&anchor_segment) {
            lintfix_fn = LintFix::replace;
            whitespace_deletions = whitespace_deletions.select(
                Some(|it: &ErasedSegment| it.id() != anchor_segment.id()),
                None,
                None,
                None,
            );
        }

        let mut fixes = vec![
            lintfix_fn(anchor_segment, create_segments, None),
            LintFix::delete(target_segment),
        ];
        fixes.extend(whitespace_deletions.into_iter().map(LintFix::delete));
        fixes
    }

    fn ensure_final_semicolon(
        &self,
        tables: &Tables,
        parent_segment: ErasedSegment,
    ) -> Option<LintResult> {
        // Iterate backwards over complete stack to find
        // if the final semicolon is already present.
        let mut anchor_segment = parent_segment.segments().last().cloned();
        let trigger_segment = parent_segment.segments().last().cloned();
        let mut semi_colon_exist_flag = false;
        let mut is_one_line = false;
        let mut before_segment = vec![];

        let mut found_code = false;
        for segment in parent_segment.segments().iter().rev() {
            anchor_segment = Some(segment.clone());
            if segment.is_type(SyntaxKind::StatementTerminator) {
                semi_colon_exist_flag = true;
            } else if segment.is_code() {
                is_one_line = Self::is_one_line_statement(parent_segment.clone(), segment.clone());
                found_code = true;
                break;
            } else if !segment.is_meta() {
                before_segment.push(segment.clone());
            }
        }

        if !found_code {
            return None;
        }

        let semicolon_newline = if is_one_line {
            false
        } else {
            self.multiline_newline
        };
        if !semi_colon_exist_flag {
            // Create the final semicolon if it does not yet exist.

            // Semicolon on same line.
            return if !semicolon_newline {
                let fixes = vec![LintFix::create_after(
                    anchor_segment.unwrap().clone(),
                    vec![
                        SegmentBuilder::token(
                            tables.next_id(),
                            ";",
                            SyntaxKind::StatementTerminator,
                        )
                        .finish(),
                    ],
                    None,
                )];
                Some(LintResult::new(
                    Some(trigger_segment.unwrap().clone()),
                    fixes,
                    None,
                    None,
                ))
            } else {
                // Semi-colon on new line.
                // Adjust before_segment and anchor_segment for inline
                // comments.
                let (_before_segment, anchor_segment) = Self::handle_preceding_inline_comments(
                    Segments::from_vec(before_segment, None),
                    anchor_segment.unwrap().clone(),
                );
                let fixes = vec![LintFix::create_after(
                    anchor_segment.clone(),
                    vec![
                        SegmentBuilder::newline(tables.next_id(), "\n"),
                        SegmentBuilder::token(
                            tables.next_id(),
                            ";",
                            SyntaxKind::StatementTerminator,
                        )
                        .finish(),
                    ],
                    None,
                )];

                Some(LintResult::new(
                    Some(trigger_segment.unwrap().clone()),
                    fixes,
                    None,
                    None,
                ))
            };
        }
        None
    }

    fn get_segment_move_context(
        target_segment: ErasedSegment,
        parent_segment: ErasedSegment,
    ) -> SegmentMoveContext {
        // Locate the segment to be moved (i.e. context.segment) and search back
        // over the raw stack to find the end of the preceding statement.

        let reversed_raw_stack =
            Segments::from_vec(parent_segment.get_raw_segments(), None).reversed();

        let before_code = reversed_raw_stack.select::<fn(&ErasedSegment) -> bool>(
            None,
            Some(|s| !s.is_code()),
            Some(&target_segment),
            None,
        );
        let before_segment = before_code.select(
            Some(|segment: &ErasedSegment| !segment.is_meta()),
            None,
            None,
            None,
        );

        // We're selecting from the raw stack, so we know that before_code is made of
        // raw elements.
        let anchor_segment = if !before_code.is_empty() {
            before_code.last().unwrap().clone()
        } else {
            target_segment.clone()
        };

        let first_code = reversed_raw_stack
            .select(
                Some(|s: &ErasedSegment| s.is_code()),
                None,
                Some(&target_segment),
                None,
            )
            .first()
            .cloned();

        let is_one_line = first_code
            .is_some_and(|segment| Self::is_one_line_statement(parent_segment, segment.clone()));

        // We can tidy up any whitespace between the segment and the preceding
        // code/comment segment. Don't mess with the comment spacing/placement.
        let whitespace_deletions = before_segment.select::<fn(&ErasedSegment) -> bool>(
            None,
            Some(|segment| segment.is_whitespace()),
            None,
            None,
        );
        SegmentMoveContext {
            anchor_segment,
            is_one_line,
            before_segment,
            whitespace_deletions,
        }
    }
}

struct SegmentMoveContext {
    anchor_segment: ErasedSegment,
    is_one_line: bool,
    before_segment: Segments,
    whitespace_deletions: Segments,
}

pub fn choose_anchor_segment(
    root_segment: &ErasedSegment,
    edit_type: EditType,
    segment: &ErasedSegment,
    filter_meta: bool,
) -> ErasedSegment {
    if !matches!(edit_type, EditType::CreateBefore | EditType::CreateAfter) {
        return segment.clone();
    }

    let mut anchor = segment.clone();
    let mut child = segment.clone();

    let mut path = root_segment
        .path_to(segment)
        .into_iter()
        .map(|it| it.segment)
        .collect_vec();
    path.reverse();

    for seg in path {
        if seg.can_start_end_non_code() {
            break;
        }

        let mut children_lists = Vec::new();
        if filter_meta {
            children_lists.push(
                seg.segments()
                    .iter()
                    .filter(|child| !child.is_meta())
                    .cloned()
                    .collect_vec(),
            );
        }
        children_lists.push(seg.segments().to_vec());
        for children in children_lists {
            match edit_type {
                EditType::CreateBefore if children[0].id() == child.id() => {
                    unreachable!()
                }
                EditType::CreateAfter if children.last().unwrap().id() == child.id() => {
                    anchor = seg.clone();
                    child = seg;
                    break;
                }
                _ => {}
            }
        }
    }

    anchor
}
