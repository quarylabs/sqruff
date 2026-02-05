use hashbrown::HashMap;
use regex::Regex;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;
use sqruff_lib_core::parser::segments::fix::SourceFix;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

/// Represents the parsed components of a Jinja tag.
struct JinjaTagComponents {
    opening: String,
    leading_ws: String,
    content: String,
    trailing_ws: String,
    closing: String,
}

/// Parse the whitespace structure of a Jinja tag.
///
/// Given a raw Jinja tag like `{{ my_variable }}`, this function extracts:
/// - opening: `{{`
/// - leading_ws: ` `
/// - content: `my_variable`
/// - trailing_ws: ` `
/// - closing: `}}`
fn get_whitespace_ends(raw: &str) -> Option<JinjaTagComponents> {
    // Regex to match Jinja tags: {{ }}, {% %}, {# #}
    // Captures: opening bracket (with optional modifier), content, closing bracket (with optional
    // modifier)
    let re = Regex::new(r"^(\{[\{%#][-+]?)(.*?)([-+]?[\}%#]\})$").ok()?;

    let captures = re.captures(raw)?;

    let opening = captures.get(1)?.as_str().to_string();
    let inner = captures.get(2)?.as_str();
    let closing = captures.get(3)?.as_str().to_string();

    // Extract leading and trailing whitespace from inner content
    let inner_len = inner.len();
    let trimmed_start = inner.trim_start();
    let leading_ws_len = inner_len - trimmed_start.len();
    let leading_ws = inner[..leading_ws_len].to_string();

    let trimmed = trimmed_start.trim_end();
    let trailing_ws = trimmed_start[trimmed.len()..].to_string();

    let content = trimmed.to_string();

    Some(JinjaTagComponents {
        opening,
        leading_ws,
        content,
        trailing_ws,
        closing,
    })
}

/// Check if whitespace is acceptable.
///
/// Whitespace is acceptable if it's either:
/// - exactly a single space, OR
/// - contains at least one newline (multi-line formatting is OK)
fn is_acceptable_whitespace(ws: &str) -> bool {
    ws == " " || ws.contains('\n')
}

#[derive(Default, Debug, Clone)]
pub struct RuleJJ01;

impl Rule for RuleJJ01 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleJJ01.erased())
    }

    fn name(&self) -> &'static str {
        "jinja.padding"
    }

    fn description(&self) -> &'static str {
        "Jinja tags should have a single whitespace on either side."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Jinja tags with either no whitespace or very long whitespace are hard to read.

```sql
SELECT {{a}} from {{ref('foo')}}
```

**Best practice**

A single whitespace surrounding Jinja tags, alternatively longer gaps containing
newlines are acceptable.

```sql
SELECT {{ a }} from {{ ref('foo') }};
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Jinja]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // This rule only applies when we have a templated file
        let Some(templated_file) = &context.templated_file else {
            return Vec::new();
        };

        // Check if this is a templated file (not just a plain SQL file)
        if !templated_file.is_templated() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let mut source_fixes = Vec::new();

        // Get the source-only slices (these are the template tags that don't render to output)
        // and also check the raw sliced file for templated sections
        for raw_slice in templated_file.raw_sliced() {
            // Only check templated sections (not literal SQL)
            // The slice_type tells us what kind of template construct this is
            let slice_type = raw_slice.slice_type();

            // We want to check template expressions and statements, not literal SQL
            // "templated" = {{ expr }}, "block_start" = {% if %}, "block_end" = {% endif %},
            // "block_mid" = {% else %}, "comment" = {# comment #}
            if !matches!(
                slice_type,
                "templated" | "block_start" | "block_end" | "block_mid" | "comment"
            ) {
                continue;
            }

            let raw = raw_slice.raw();

            // Check if it looks like a Jinja tag (starts with { and ends with })
            if !raw.starts_with('{') || !raw.ends_with('}') {
                continue;
            }

            // Parse the whitespace structure
            let Some(components) = get_whitespace_ends(raw) else {
                continue;
            };

            // Check leading and trailing whitespace
            let leading_ok = is_acceptable_whitespace(&components.leading_ws);
            let trailing_ok = is_acceptable_whitespace(&components.trailing_ws);

            if !leading_ok || !trailing_ok {
                // Build the expected corrected tag
                let fixed_tag = format!(
                    "{} {} {}",
                    components.opening, components.content, components.closing
                );

                let description = if !leading_ok && !trailing_ok {
                    format!(
                        "Jinja tags should have a single whitespace on either side: `{}` -> `{}`",
                        raw, fixed_tag
                    )
                } else if !leading_ok {
                    format!(
                        "Jinja tags should have a single whitespace on the left side: `{}` -> `{}`",
                        raw, fixed_tag
                    )
                } else {
                    format!(
                        "Jinja tags should have a single whitespace on the right side: `{}` -> \
                         `{}`",
                        raw, fixed_tag
                    )
                };

                // Create a source fix for this jinja tag
                let source_slice = raw_slice.source_slice();
                // For templated_slice, we use an empty range since template tags
                // don't have a direct mapping to the templated output
                let templated_slice = 0..0;

                source_fixes.push(SourceFix::new(
                    SmolStr::new(&fixed_tag),
                    source_slice,
                    templated_slice,
                ));

                // Report violation
                results.push(LintResult::new(
                    Some(context.segment.clone()),
                    vec![], // Fixes will be added below after collecting all
                    Some(description),
                    None,
                ));
            }
        }

        // If we have source fixes, create a single fix that contains all of them
        if !source_fixes.is_empty() && !results.is_empty() {
            // Find the first raw segment to use as an anchor
            // We can't use the root segment because apply_fixes only looks at children
            let raw_segments = context.segment.get_raw_segments();
            if let Some(anchor_seg) = raw_segments.first() {
                // Create a segment that carries the source fixes
                // The segment must have the same raw text as the anchor for is_just_source_edit
                // We wrap the anchor's content in a node that has source_fixes
                let inner_token = SegmentBuilder::token(
                    context.tables.next_id(),
                    anchor_seg.raw().as_ref(),
                    anchor_seg.get_type(),
                )
                .with_position(anchor_seg.get_position_marker().cloned().unwrap())
                .finish();

                let fix_segment = SegmentBuilder::node(
                    context.tables.next_id(),
                    SyntaxKind::File,
                    context.dialect.name,
                    vec![inner_token],
                )
                .with_source_fixes(source_fixes)
                .with_position(anchor_seg.get_position_marker().cloned().unwrap())
                .finish();

                // Create a LintFix::Replace with the first raw segment as anchor
                let fix = LintFix::replace(anchor_seg.clone(), vec![fix_segment], None);

                // Add the fix to all results
                for result in &mut results {
                    result.fixes = vec![fix.clone()];
                }
            }
        }

        results
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        // Run once per file at the root level
        RootOnlyCrawler.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_whitespace_ends_basic() {
        let result = get_whitespace_ends("{{ foo }}").unwrap();
        assert_eq!(result.opening, "{{");
        assert_eq!(result.leading_ws, " ");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, "}}");
    }

    #[test]
    fn test_get_whitespace_ends_no_whitespace() {
        let result = get_whitespace_ends("{{foo}}").unwrap();
        assert_eq!(result.opening, "{{");
        assert_eq!(result.leading_ws, "");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, "");
        assert_eq!(result.closing, "}}");
    }

    #[test]
    fn test_get_whitespace_ends_excessive_whitespace() {
        let result = get_whitespace_ends("{{   foo   }}").unwrap();
        assert_eq!(result.opening, "{{");
        assert_eq!(result.leading_ws, "   ");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, "   ");
        assert_eq!(result.closing, "}}");
    }

    #[test]
    fn test_get_whitespace_ends_block() {
        let result = get_whitespace_ends("{% if x %}").unwrap();
        assert_eq!(result.opening, "{%");
        assert_eq!(result.leading_ws, " ");
        assert_eq!(result.content, "if x");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, "%}");
    }

    #[test]
    fn test_get_whitespace_ends_comment() {
        let result = get_whitespace_ends("{# comment #}").unwrap();
        assert_eq!(result.opening, "{#");
        assert_eq!(result.leading_ws, " ");
        assert_eq!(result.content, "comment");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, "#}");
    }

    #[test]
    fn test_get_whitespace_ends_with_modifier() {
        let result = get_whitespace_ends("{{- foo -}}").unwrap();
        assert_eq!(result.opening, "{{-");
        assert_eq!(result.leading_ws, " ");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, "-}}");
    }

    #[test]
    fn test_is_acceptable_whitespace() {
        assert!(is_acceptable_whitespace(" "));
        assert!(is_acceptable_whitespace("\n"));
        assert!(is_acceptable_whitespace("  \n  "));
        assert!(!is_acceptable_whitespace(""));
        assert!(!is_acceptable_whitespace("  "));
        assert!(!is_acceptable_whitespace("\t"));
    }
}
