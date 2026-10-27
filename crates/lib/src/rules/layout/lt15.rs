use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

/// Where in the file a gap sits, which decides whose limit applies.
///
/// The three scopes are the ones the maximum has always distinguished; naming
/// them lets the minimum resolve the same way instead of keeping a second,
/// slightly different copy of the logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    InsideStatement,
    BetweenStatements,
    BetweenBatches,
}

/// Per-file cache of whether the file's top level contains batch nodes, stored
/// in [`RuleContext`] memory so the file-level scope check stays O(1) per gap.
#[derive(Debug, Clone, Copy)]
struct FileHasBatches(bool);

#[derive(Debug, Clone)]
pub struct RuleLT15 {
    maximum_empty_lines_between_statements: usize,
    maximum_empty_lines_inside_statements: usize,
    maximum_empty_lines_between_batches: usize,
    minimum_empty_lines_between_statements: usize,
}

impl Default for RuleLT15 {
    fn default() -> Self {
        Self {
            maximum_empty_lines_between_statements: 2,
            maximum_empty_lines_inside_statements: 1,
            maximum_empty_lines_between_batches: 1,
            minimum_empty_lines_between_statements: 0,
        }
    }
}

/// Whether a segment is a batch node, in either of the dialects that have them.
///
/// SQLFluff gives both T-SQL and Oracle batches the single type `batch`;
/// sqruff splits them into [`SyntaxKind::Batch`] and [`SyntaxKind::OracleBatch`],
/// so both have to be checked to match the upstream behaviour.
fn is_batch(segment: &ErasedSegment) -> bool {
    segment.is_type(SyntaxKind::Batch) || segment.is_type(SyntaxKind::OracleBatch)
}

impl RuleLT15 {
    /// Which of the three scopes this position sits in.
    ///
    /// Shared by the maximum's limit selection and the minimum's gate so the two
    /// cannot drift apart. Batches are detected structurally rather than by
    /// dialect name: the previous `dialect.name == tsql` test missed Oracle,
    /// which also has batch grammar, so a file-level gap there was measured
    /// against the between-statements maximum instead of the between-batches
    /// one. Testing for the node itself also means a dialect that gains batch
    /// grammar later needs no change here.
    fn resolve_scope(&self, context: &RuleContext) -> Scope {
        if context
            .parent_stack
            .iter()
            .any(|seg| seg.is_type(SyntaxKind::Statement))
        {
            // Directly inside a with_compound_statement (between CTEs, or between
            // the last CTE and the main query) uses the between-statements limit
            // to avoid conflicting with LT08, which requires blank lines
            // after CTEs.
            if context
                .parent_stack
                .last()
                .is_some_and(|seg| seg.is_type(SyntaxKind::WithCompoundStatement))
            {
                return Scope::BetweenStatements;
            }
            return Scope::InsideStatement;
        }

        if context.parent_stack.iter().any(is_batch) {
            return Scope::BetweenStatements;
        }

        // File level: a gap out here separates batches in the dialects that have
        // them, and statements everywhere else.
        if self.file_has_batches(context) {
            return Scope::BetweenBatches;
        }
        Scope::BetweenStatements
    }

    /// Whether the enclosing node holds any batch child.
    ///
    /// At file level every gap shares the same parent (the file root), so
    /// whether that node contains a batch is a property of the whole file. It
    /// is computed once and cached: scanning every sibling at every gap — and
    /// cloning both sibling slices to do it — made the rule quadratic in the
    /// number of statements, even at the default minimum of zero where scope is
    /// still resolved for the maximum. The parent's children are scanned by
    /// reference rather than through the cloning sibling helpers.
    fn file_has_batches(&self, context: &RuleContext) -> bool {
        if let Some(FileHasBatches(cached)) = context.try_get::<FileHasBatches>() {
            return cached;
        }
        let has_batches = context
            .parent_stack
            .last()
            .is_some_and(|parent| parent.segments().iter().any(is_batch));
        context.set(FileHasBatches(has_batches));
        has_batches
    }

    /// Blank lines in the run of newlines ending at the current segment.
    ///
    /// Both directions measure the gap with this, so they cannot disagree about
    /// what "two blank lines" means. Two consecutive newlines bound one blank
    /// line, hence the run length less one. Whitespace does not break the run,
    /// so a line of spaces still counts as blank.
    ///
    /// A templated newline ends the run rather than being counted: those lines
    /// are not in the source, so neither direction may add or remove them, and
    /// neither should let them stand in for lines the user wrote. Counting only
    /// as far as the nearest one leaves both directions measuring the same set
    /// of editable lines.
    ///
    /// Returns the count and whether the run was cut short by templated output.
    /// The maximum only ever deletes an editable newline, so the count alone is
    /// enough for it. The minimum has to add lines, and padding a gap whose
    /// rendered form already contains lines the source does not would be
    /// guessing at the result, so it declines those gaps entirely.
    ///
    /// Returns `None` when the anchor itself is templated, since there is then
    /// nothing editable to anchor a fix to.
    fn count_blank_lines(&self, context: &RuleContext) -> Option<(usize, bool)> {
        if context.segment.is_templated() {
            return None;
        }

        // Walk back to the statement that opens the gap, counting blank lines.
        // A comment does not end the gap: `SELECT a;`, a blank line, a comment,
        // then `SELECT b;` is one gap that already has its blank line, and
        // stopping at the comment would hide it and pad the gap a second time.
        // A comment does occupy a line, though, so it ends the current run of
        // blank lines rather than being counted as one.
        let mut blank_lines: usize = 0;
        let mut run: usize = 1;
        let mut touched_templated = false;
        let mut in_comment = false;
        // A multi-line block comment lexes as one block_comment segment per
        // physical line, with its own newlines interleaved between them. Those
        // newlines are the comment's internal formatting, not blank lines
        // between statements, so once inside an unclosed comment body they must
        // not accumulate toward run or be banked. A segment starts a comment
        // (rather than continuing one seen further down) when its raw text
        // itself opens with a comment marker; only `/* */` bodies span more
        // than one segment, so `--`/`#` inline comments never trigger this.
        let mut in_open_block_comment_body = false;
        for raw_seg in context.raw_stack.iter().rev() {
            if raw_seg.is_comment() {
                if !in_comment {
                    // Bank the run that ended at this comment block. Further
                    // segments of the same multi-line comment must not bank
                    // again: they are one occupied line, not several.
                    blank_lines += run.saturating_sub(1);
                    run = 0;
                    in_comment = true;
                }
                in_open_block_comment_body = raw_seg.is_type(SyntaxKind::BlockComment)
                    && !raw_seg.raw().trim_start().starts_with("/*");
                continue;
            }
            if raw_seg.is_type(SyntaxKind::Newline) {
                if raw_seg.is_templated() {
                    touched_templated = true;
                    break;
                }
                if in_open_block_comment_body {
                    // Interior to the comment body just walked past: not a gap.
                    continue;
                }
                in_comment = false;
                run += 1;
            } else if raw_seg.is_type(SyntaxKind::Whitespace) {
                continue;
            } else {
                break;
            }
        }
        Some((blank_lines + run.saturating_sub(1), touched_templated))
    }

    /// The minimum actually enforced, capped by the maximum for the scope.
    ///
    /// See the note in the rule docs: uncapped, the two fixers undo each other
    /// and `fix` never converges.
    fn effective_minimum(&self) -> usize {
        self.minimum_empty_lines_between_statements
            .min(self.maximum_empty_lines_between_statements)
    }

    /// Whether the position is a gap with a statement on each side.
    ///
    /// A gap needs a statement on both sides, tested on the statement type
    /// rather than on any code. Without the preceding test, a file opening with
    /// a blank line or a comment reads as a gap before its first statement and
    /// gets padded. Without the following test, a batch delimiter (T-SQL `GO`,
    /// Oracle `/`) reads as the next statement and the rule demands a blank
    /// line in front of it; those are `go_statement` and `slash_buffer_executor`,
    /// and they end a batch rather than starting one.
    fn is_gap_between_statements(&self, context: &RuleContext, shares_line: bool) -> bool {
        let following: Vec<ErasedSegment> = context
            .siblings_post()
            .into_iter()
            .filter(|seg| {
                !seg.is_type(SyntaxKind::Dedent)
                    && !seg.is_type(SyntaxKind::Indent)
                    && !seg.is_type(SyntaxKind::Whitespace)
            })
            .collect();

        if shares_line {
            // Whitespace and trailing comments are skipped rather than treated as
            // the end of the search: `SELECT a; -- x` still ends its line at the
            // newline after the comment, so stopping at the comment would report a
            // shared line that is not shared and tear the comment off it.
            for seg in &following {
                if seg.is_comment() {
                    continue;
                }
                // A newline already separates the statements, so the run-based
                // path owns this gap.
                if seg.is_type(SyntaxKind::Newline) {
                    return false;
                }
                // A template tag in the gap is not a statement sharing the line,
                // and its surrounding whitespace is not the user's to rewrite.
                if seg.is_type(SyntaxKind::Placeholder) {
                    return false;
                }
                break;
            }
        } else {
            // Only act on the last newline of the run, or the same gap is
            // reported once per newline in it.
            if following
                .first()
                .is_some_and(|seg| seg.is_type(SyntaxKind::Newline))
            {
                return false;
            }
            // A comment splits the gap into two runs of newlines, and both would
            // otherwise qualify: each has a statement behind it and a statement
            // ahead of it, so one gap gets padded twice. The run that ends at the
            // next statement is the gap's anchor, so a run that reaches a comment
            // first is not.
            for seg in &following {
                if seg.is_comment() {
                    return false;
                }
                if seg.is_type(SyntaxKind::Statement) {
                    break;
                }
            }
        }

        if !context
            .siblings_pre()
            .iter()
            .any(|seg| seg.is_type(SyntaxKind::Statement))
        {
            return false;
        }
        if !following
            .iter()
            .any(|seg| seg.is_type(SyntaxKind::Statement))
        {
            return false;
        }

        // A template block start/end sits in the gap as a placeholder meta. The
        // newlines either side of it are not a gap between two statements, so
        // padding them puts blank lines around the tag rather than between the
        // statements, and the tag is not the user's line to move.
        for seg in context.siblings_pre().iter().rev() {
            if seg.is_code() {
                break;
            }
            if seg.is_type(SyntaxKind::Placeholder) {
                return false;
            }
        }
        for seg in context.siblings_post() {
            if seg.is_code() {
                break;
            }
            if seg.is_type(SyntaxKind::Placeholder) {
                return false;
            }
        }

        true
    }

    /// Require at least `minimum_empty_lines_between_statements` blank lines.
    ///
    /// Handles both shapes a too-small gap can take. Usually there is a run of
    /// newlines to measure and pad. When the statements share a line
    /// (`SELECT a; SELECT b;`) there is no newline to crawl at all, so the run
    /// cannot be measured, the gap is zero by definition, and the fix has to
    /// create the line break as well as the blank lines.
    fn check_minimum(&self, context: &RuleContext, shares_line: bool) -> Vec<LintResult> {
        let minimum = self.effective_minimum();
        if minimum == 0 {
            return Vec::new();
        }

        if !self.is_gap_between_statements(context, shares_line) {
            return Vec::new();
        }

        let (blank_lines, newlines_to_add) = if shares_line {
            // No run to measure, so the templated check that the counter performs
            // for the other path has to happen here instead.
            if context.segment.is_templated() {
                return Vec::new();
            }
            // One extra newline ends the first statement's line; the rest are the
            // blank lines themselves.
            (0, minimum + 1)
        } else {
            let Some((counted, touched_templated)) = self.count_blank_lines(context) else {
                // Templated anchor: not ours to rewrite.
                return Vec::new();
            };
            if touched_templated || counted >= minimum {
                return Vec::new();
            }
            (counted, minimum - counted)
        };

        self.warn_if_capped();

        let mut fixes = vec![LintFix::create_after(
            context.segment.clone(),
            (0..newlines_to_add)
                .map(|_| SegmentBuilder::newline(context.tables.next_id(), "\n"))
                .collect(),
            None,
        )];
        if shares_line {
            // Drop the space that separated the statements; it would otherwise be
            // left indenting the statement now starting the line.
            for seg in context.siblings_post() {
                if seg.is_type(SyntaxKind::Dedent) || seg.is_type(SyntaxKind::Indent) {
                    continue;
                }
                if seg.is_type(SyntaxKind::Whitespace) {
                    fixes.push(LintFix::delete(seg));
                } else {
                    break;
                }
            }
        }

        let description = if shares_line {
            format!(
                "Expected at least {minimum} blank line(s) between statements, \
                 found 0 (statements share a line)."
            )
        } else {
            format!(
                "Expected at least {minimum} blank line(s) between statements, \
                 found {blank_lines}."
            )
        };

        vec![LintResult::new(
            context.segment.clone().into(),
            fixes,
            Some(description),
            None,
        )]
    }

    /// Tell the user when the cap is what decided the reported gap.
    ///
    /// Only fires where it changes an actual result, so a contradictory config
    /// that never meets two statements stays quiet.
    fn warn_if_capped(&self) {
        let configured = self.minimum_empty_lines_between_statements;
        let maximum = self.maximum_empty_lines_between_statements;
        if configured > maximum {
            log::warn!(
                "minimum_empty_lines_between_statements ({configured}) is greater than \
                 maximum_empty_lines_between_statements ({maximum}); enforcing {maximum}. No \
                 file can satisfy both, and applying the minimum as configured would leave \
                 `sqruff fix` alternating between the two results instead of converging."
            );
        }
    }
}

impl Rule for RuleLT15 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT15 {
            maximum_empty_lines_between_statements: config
                .get("maximum_empty_lines_between_statements")
                .and_then(Value::as_int)
                .map(|v| v as usize)
                .unwrap_or(self.maximum_empty_lines_between_statements),
            maximum_empty_lines_inside_statements: config
                .get("maximum_empty_lines_inside_statements")
                .and_then(Value::as_int)
                .map(|v| v as usize)
                .unwrap_or(self.maximum_empty_lines_inside_statements),
            maximum_empty_lines_between_batches: config
                .get("maximum_empty_lines_between_batches")
                .and_then(Value::as_int)
                .map(|v| v as usize)
                .unwrap_or(self.maximum_empty_lines_between_batches),
            minimum_empty_lines_between_statements: config
                .get("minimum_empty_lines_between_statements")
                .and_then(Value::as_int)
                .map(|v| v as usize)
                .unwrap_or(self.minimum_empty_lines_between_statements),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "layout.newlines"
    }

    fn description(&self) -> &'static str {
        "Too many consecutive blank lines."
    }

    fn long_description(&self) -> &'static str {
        r#"**Anti-pattern**

In this example, the maximum number of empty lines inside a statement is set to 0.

```sql
SELECT 'a' AS col
FROM tab


WHERE x = 4
ORDER BY y


LIMIT 5
;
```

**Best practice**

```sql
SELECT 'a' AS col
FROM tab
WHERE x = 4
ORDER BY y
LIMIT 5
;
```

A minimum can also be required between statements. With
`minimum_empty_lines_between_statements = 1`, this is an anti-pattern:

```sql
SELECT a FROM tab;
SELECT b FROM tab;
```

and this is the best practice:

```sql
SELECT a FROM tab;

SELECT b FROM tab;
```

The minimum defaults to `0`, which leaves the rule's existing behaviour
unchanged. It applies only between statements, never inside one or between
batches, matching the scope its name describes.

`minimum_empty_lines_between_statements` is capped by
`maximum_empty_lines_between_statements`. The two settings validate
independently, so a minimum above the maximum is accepted configuration, but no
file can satisfy both: the minimum inserts blank lines and the maximum deletes
them straight back, so `sqruff fix` alternates between the two results on every
run instead of converging. Where the cap changes what would otherwise be
reported, a warning is emitted naming both values."#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let scope = self.resolve_scope(context);

        // Terminators are only sought for the same-line minimum check; none of
        // the maximum logic below applies to them.
        if context.segment.is_type(SyntaxKind::StatementTerminator) {
            if scope != Scope::BetweenStatements {
                return Vec::new();
            }
            return self.check_minimum(context, true);
        }

        if !context.segment.is_type(SyntaxKind::Newline) {
            return Vec::new();
        }

        // The minimum runs first because the two cannot both fire on one gap: one
        // wants newlines added, the other wants them removed.
        if scope == Scope::BetweenStatements {
            let minimum_result = self.check_minimum(context, false);
            if !minimum_result.is_empty() {
                return minimum_result;
            }
        }

        let maximum_empty_lines = match scope {
            Scope::InsideStatement => self.maximum_empty_lines_inside_statements,
            Scope::BetweenStatements => self.maximum_empty_lines_between_statements,
            Scope::BetweenBatches => self.maximum_empty_lines_between_batches,
        };

        // The maximum keeps its original windowed eligibility test rather than
        // reusing `count_blank_lines`. The two are not interchangeable: the
        // counter skips whitespace and counts the editable newlines ahead of a
        // templated one, where this check suppresses the violation outright for
        // both. Sharing it silently widened a long-standing rule, so the shared
        // helper stays with the minimum and this stays as it shipped.
        if context.raw_stack.len() < maximum_empty_lines {
            return Vec::new();
        }

        let start = context
            .raw_stack
            .len()
            .saturating_sub(maximum_empty_lines + 1);
        for raw_segment in &context.raw_stack[start..] {
            if raw_segment.is_templated() || !raw_segment.is_type(SyntaxKind::Newline) {
                return Vec::new();
            }
        }

        vec![LintResult::new(
            context.segment.clone().into(),
            vec![LintFix::delete(context.segment.clone())],
            None,
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        // Statement terminators are sought as well as newlines: a minimum between
        // statements has to be enforceable when the statements share a line, where
        // there is no newline to crawl at all.
        SegmentSeekerCrawler::new(
            const { SyntaxSet::new(&[SyntaxKind::Newline, SyntaxKind::StatementTerminator]) },
        )
        .provide_raw_stack()
        .into()
    }
}
