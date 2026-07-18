use hashbrown::{HashMap, HashSet};
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::{BlockType, ErasedSegment, SegmentBuilder};
use sqruff_lib_core::templaters::{RawFileSlice, TemplateSliceKind, TemplatedFile};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{
    Erased, ErasedRule, LintPhase, LintResult, Rule, RuleGroups, targets_templated,
};
use crate::utils::functional::context::FunctionalContext;

fn get_trailing_newlines(segment: &ErasedSegment) -> Vec<ErasedSegment> {
    let mut result = Vec::new();

    for seg in segment.recursive_crawl_all(true) {
        if seg.is_type(SyntaxKind::Newline) {
            result.push(seg.clone());
        } else if !seg.is_whitespace()
            && !seg.is_type(SyntaxKind::Dedent)
            && !seg.is_type(SyntaxKind::EndOfFile)
            && !is_source_only_template_placeholder(&seg)
        {
            break;
        }
    }

    result
}

fn trailing_newline_count(segments: &Segments) -> usize {
    segments
        .iter()
        .map(|segment| segment.raw().chars().filter(|&ch| ch == '\n').count())
        .sum()
}

fn get_last_segment(mut segment: Segments) -> (Vec<ErasedSegment>, Segments) {
    let mut parent_stack = Vec::new();

    loop {
        let children = segment.children_all();

        if !children.is_empty() {
            parent_stack.push(segment.first().unwrap().clone());
            segment = children.find_last_where(|s| {
                !s.is_type(SyntaxKind::EndOfFile) && !is_source_only_template_placeholder(s)
            });
        } else {
            return (parent_stack, segment);
        }
    }
}

fn is_source_only_template_placeholder(segment: &ErasedSegment) -> bool {
    segment.is_type(SyntaxKind::Placeholder)
        && matches!(
            segment.block_type(),
            Some(
                BlockType::Comment
                    | BlockType::BlockStart
                    | BlockType::BlockMid
                    | BlockType::BlockEnd
            )
        )
}

fn source_eof_anchor(segment: &ErasedSegment, source_len: usize) -> Option<ErasedSegment> {
    let mut anchor = None;

    for seg in segment.recursive_crawl_all(true) {
        if seg.is_type(SyntaxKind::EndOfFile) {
            continue;
        }

        if seg
            .get_position_marker()
            .is_some_and(|marker| marker.source_slice.end == source_len)
        {
            anchor = Some(seg.clone());
        }
    }

    anchor
}

fn templated_source_missing_final_newline(context: &RuleContext) -> bool {
    context
        .templated_file
        .as_ref()
        .is_some_and(|templated_file| {
            !templated_file.source_str.ends_with('\n')
                && templated_file.templated().ends_with('\n')
                && templated_file
                    .raw_sliced()
                    .iter()
                    .any(|slice| !slice.has_slice_kind(TemplateSliceKind::Literal))
        })
}

fn raw_slice_index_at_source_pos(raw_sliced: &[RawFileSlice], source_pos: usize) -> Option<usize> {
    raw_sliced.iter().position(|slice| {
        slice.source_slice().start <= source_pos && source_pos < slice.source_slice().end
    })
}

fn whitespace_only_literal_inside_template_block(
    templated_file: &TemplatedFile,
    source_pos: usize,
) -> bool {
    let raw_sliced = templated_file.raw_sliced();
    let Some(raw_idx) = raw_slice_index_at_source_pos(raw_sliced, source_pos) else {
        return false;
    };
    let raw_slice = &raw_sliced[raw_idx];

    raw_slice.has_slice_kind(TemplateSliceKind::Literal)
        && raw_slice.raw().chars().all(char::is_whitespace)
        && raw_sliced[raw_idx + 1..].iter().any(|next_slice| {
            next_slice.block_idx() == raw_slice.block_idx()
                && matches!(
                    next_slice.slice_kind(),
                    TemplateSliceKind::BlockMid | TemplateSliceKind::BlockEnd
                )
        })
}

fn templated_file_has_extra_final_newline(context: &RuleContext) -> bool {
    context
        .templated_file
        .as_ref()
        .is_some_and(|templated_file| {
            if !templated_file
                .raw_sliced()
                .iter()
                .any(|slice| !slice.has_slice_kind(TemplateSliceKind::Literal))
            {
                return false;
            }

            let templated = templated_file.templated();
            let mut literal_trailing_newline_source_positions = HashSet::new();

            for idx in (0..templated.len()).rev() {
                if templated.as_bytes()[idx] != b'\n' {
                    break;
                }

                let Some(slice) = templated_file.sliced_file.iter().find(|slice| {
                    slice.templated_slice.start <= idx && idx < slice.templated_slice.end
                }) else {
                    continue;
                };

                if slice.has_slice_kind(TemplateSliceKind::Literal) {
                    let source_idx = slice.source_slice.start + (idx - slice.templated_slice.start);
                    if source_idx < slice.source_slice.end
                        && !whitespace_only_literal_inside_template_block(
                            templated_file,
                            source_idx,
                        )
                    {
                        literal_trailing_newline_source_positions.insert(source_idx);
                    }
                }
            }

            literal_trailing_newline_source_positions.len() > 1
        })
}

#[derive(Debug, Default, Clone)]
pub struct RuleLT12;

impl Rule for RuleLT12 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT12.erased())
    }
    fn lint_phase(&self) -> LintPhase {
        LintPhase::Post
    }

    fn name(&self) -> &'static str {
        "layout.end_of_file"
    }

    fn description(&self) -> &'static str {
        "Files must end with a single trailing newline."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

The content in file does not end with a single trailing newline. The $ represents end of file.

```sql
 SELECT
     a
 FROM foo$

 -- Ending on an indented line means there is no newline
 -- at the end of the file, the • represents space.

 SELECT
 ••••a
 FROM
 ••••foo
 ••••$

 -- Ending on a semi-colon means the last line is not a
 -- newline.

 SELECT
     a
 FROM foo
 ;$

 -- Ending with multiple newlines.

 SELECT
     a
 FROM foo

 $
```

**Best practice**

Add trailing newline to the end. The $ character represents end of file.

```sql
 SELECT
     a
 FROM foo
 $

 -- Ensuring the last line is not indented so is just a
 -- newline.

 SELECT
 ••••a
 FROM
 ••••foo
 $

 -- Even when ending on a semi-colon, ensure there is a
 -- newline after.

 SELECT
     a
 FROM foo
 ;
 $
```
"#
    }
    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }

    targets_templated!();

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // Edge case: if the file is totally empty, there's nothing to enforce a
        // trailing newline on. sqruff represents an empty file as a single
        // zero-length literal placeholder followed by `end_of_file`, so the
        // "last segment" is that empty placeholder rather than nothing. Return
        // without error, mirroring SQLFluff's `if not segment: return None`.
        if context.segment.raw().is_empty() {
            return Vec::new();
        }

        let source_missing_final_newline = templated_source_missing_final_newline(context);
        let templated_extra_final_newline = templated_file_has_extra_final_newline(context);
        let (parent_stack, segment) = get_last_segment(FunctionalContext::new(context).segment());

        if segment.is_empty() {
            return if source_missing_final_newline {
                context
                    .templated_file
                    .as_ref()
                    .and_then(|templated_file| {
                        source_eof_anchor(&context.segment, templated_file.source_str.len())
                    })
                    .map(|anchor| LintResult::new(anchor.into(), Vec::new(), None, None))
                    .into_iter()
                    .collect()
            } else {
                Vec::new()
            };
        }

        let trailing_newlines = Segments::from_vec(get_trailing_newlines(&context.segment), None);
        let trailing_newline_count = trailing_newline_count(&trailing_newlines);
        let has_non_literal_slices =
            context
                .templated_file
                .as_ref()
                .is_some_and(|templated_file| {
                    templated_file
                        .raw_sliced()
                        .iter()
                        .any(|slice| !slice.has_slice_kind(TemplateSliceKind::Literal))
                });

        if trailing_newlines.is_empty() || source_missing_final_newline {
            let default_fix_anchor_segment = if parent_stack.len() == 1 {
                segment.first().unwrap().clone()
            } else {
                parent_stack[1].clone()
            };
            let fix_anchor_segment = if source_missing_final_newline {
                context
                    .templated_file
                    .as_ref()
                    .and_then(|templated_file| {
                        source_eof_anchor(&context.segment, templated_file.source_str.len())
                    })
                    .unwrap_or(default_fix_anchor_segment)
            } else {
                default_fix_anchor_segment
            };

            let fixes = if source_missing_final_newline {
                Vec::new()
            } else {
                vec![LintFix::create_after(
                    fix_anchor_segment,
                    vec![SegmentBuilder::newline(context.tables.next_id(), "\n")],
                    None,
                )]
            };

            vec![LintResult::new(
                segment.first().unwrap().clone().into(),
                fixes,
                None,
                None,
            )]
        } else if (!has_non_literal_slices && trailing_newline_count > 1)
            || templated_extra_final_newline
        {
            let fixes = if templated_extra_final_newline
                || has_non_literal_slices
                || trailing_newlines
                    .iter()
                    .any(|segment| segment.raw().chars().filter(|&ch| ch == '\n').count() > 1)
            {
                Vec::new()
            } else {
                trailing_newlines
                    .into_iter()
                    .skip(1)
                    .map(|d| LintFix::delete(d.clone()))
                    .collect()
            };

            vec![LintResult::new(
                segment.first().unwrap().clone().into(),
                fixes,
                None,
                None,
            )]
        } else {
            vec![]
        }
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}
