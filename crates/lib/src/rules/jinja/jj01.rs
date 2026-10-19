use hashbrown::HashMap;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::markers::PositionMarker;
use sqruff_lib_core::parser::segments::SegmentBuilder;
use sqruff_lib_core::parser::segments::fix::SourceFix;
use sqruff_lib_core::templaters::TemplateSliceKind;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups, targets_templated};

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
/// Given a raw Jinja tag like `{{ my_variable }}` and its open (`{{`) and
/// close (`}}`) delimiters, this function extracts:
/// - opening: `{{`
/// - leading_ws: ` `
/// - content: `my_variable`
/// - trailing_ws: ` `
/// - closing: `}}`
///
/// The delimiters are passed in rather than hardcoded so that custom Jinja
/// delimiters (e.g. Snowflake CLI's `<% %>`) are handled. Ported from
/// SQLFluff #8277.
fn get_whitespace_ends(
    raw: &str,
    open_delim: &str,
    close_delim: &str,
) -> Option<JinjaTagComponents> {
    if !raw.starts_with(open_delim) || !raw.ends_with(close_delim) {
        return None;
    }
    let pre_len = open_delim.len();
    let post_len = close_delim.len();
    if raw.len() < pre_len + post_len {
        return None;
    }

    let mut main = &raw[pre_len..raw.len() - post_len];
    let mut opening = raw[..pre_len].to_string();
    let mut closing = raw[raw.len() - post_len..].to_string();

    // Optionally Jinja tags may also have plus or minus whitespace-control
    // notation adjacent to the delimiters (e.g. `{{- foo -}}`).
    if matches!(main.chars().next(), Some('+' | '-')) {
        main = &main[1..];
        opening = raw[..pre_len + 1].to_string();
    }
    if matches!(main.chars().last(), Some('+' | '-')) {
        main = &main[..main.len() - 1];
        closing = raw[raw.len() - post_len - 1..].to_string();
    }

    // Extract leading and trailing whitespace from inner content
    let inner_len = main.len();
    let trimmed_start = main.trim_start();
    let leading_ws_len = inner_len - trimmed_start.len();
    let leading_ws = main[..leading_ws_len].to_string();

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

/// Build the (open, close) Jinja delimiter pairs for the current config.
///
/// Any delimiter that is not explicitly overridden keeps its Jinja default,
/// so custom variable delimiters can coexist with default block/comment tags.
/// Pairs are sorted so the longest open delimiter is matched first; that stops
/// a shorter prefix (e.g. `<`) from shadowing a longer one (e.g. `<%`) that
/// also starts the tag.
fn delimiter_pairs(config: &crate::core::config::FluffConfig) -> Vec<(String, String)> {
    // Read a single value from the `[templater:jinja]` config section. The
    // `TemplaterKind::Jinja` enum variant only exists with the `python`
    // feature, so look the value up through the raw config map instead; this
    // rule is only reached when the jinja (or dbt) templater is active.
    let jinja_value = |key: &str| -> Option<String> {
        config
            .raw
            .get("templater")
            .and_then(Value::as_map)
            .and_then(|templater| templater.get("jinja"))
            .and_then(Value::as_map)
            .and_then(|jinja| jinja.get(key))
            .and_then(Value::as_string)
            .map(ToString::to_string)
    };

    let defaults = [
        ("variable", "{{", "}}"),
        ("block", "{%", "%}"),
        ("comment", "{#", "#}"),
    ];
    let mut pairs: Vec<(String, String)> = defaults
        .iter()
        .map(|(kind, d_open, d_close)| {
            let open =
                jinja_value(&format!("{kind}_start_string")).unwrap_or_else(|| d_open.to_string());
            let close =
                jinja_value(&format!("{kind}_end_string")).unwrap_or_else(|| d_close.to_string());
            (open, close)
        })
        .collect();
    pairs.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));
    pairs
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
This rule is only active if the `jinja` templater (or one of its subclasses,
like the `dbt` templater) is used for the current file.

**Anti-pattern**

Jinja tags with either no whitespace or very long whitespace are hard to read.

```jinja
SELECT {{a}} from {{ref('foo')}}
```

**Best practice**

A single whitespace surrounding Jinja tags, alternatively longer gaps containing
newlines are acceptable.

```jinja
SELECT {{ a }} from {{ ref('foo') }};
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Jinja]
    }

    targets_templated!();

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // This rule only applies when we have a templated file
        let Some(templated_file) = &context.templated_file else {
            return Vec::new();
        };

        // Check if this is a templated file (not just a plain SQL file)
        if !templated_file.is_templated() {
            return Vec::new();
        }

        // We also only work with setups which use the jinja templater or a
        // derivative of that (like the dbt templater). Otherwise return empty.
        if !matches!(
            context.config.templater_kind().map(|kind| kind.as_str()),
            Ok("jinja" | "dbt")
        ) {
            return Vec::new();
        }

        // Read the configured Jinja delimiters so we can match tags that use
        // custom delimiters (e.g. Snowflake CLI's `<% var %>`).
        let delim_pairs = delimiter_pairs(context.config);

        let mut results = Vec::new();
        let mut all_source_fixes = Vec::new();

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
                TemplateSliceKind::Templated
                    | TemplateSliceKind::BlockStart
                    | TemplateSliceKind::BlockEnd
                    | TemplateSliceKind::BlockMid
                    | TemplateSliceKind::Comment
            ) {
                continue;
            }

            let raw = raw_slice.raw();

            // Match the tag against a delimiter pair (open and close together),
            // trying the longest open delimiter first.
            let Some((open_delim, close_delim)) = delim_pairs.iter().find(|(d_open, d_close)| {
                raw.starts_with(d_open.as_str()) && raw.ends_with(d_close.as_str())
            }) else {
                continue;
            };

            // Parse the whitespace structure
            let Some(components) = get_whitespace_ends(raw, open_delim, close_delim) else {
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

                all_source_fixes.push(SourceFix::new(
                    SmolStr::new(&fixed_tag),
                    source_slice.clone(),
                    templated_slice,
                ));

                // Create an anchor segment with the correct source position for
                // this violation. We use the source index as the templated_slice
                // start so that source_position() looks up the right location in
                // source_newlines.
                let position_marker = PositionMarker::new(
                    source_slice.clone(),
                    source_slice,
                    templated_file.clone(),
                    None,
                    None,
                );

                let anchor =
                    SegmentBuilder::token(context.tables.next_id(), raw, SyntaxKind::TemplateLoop)
                        .with_position(position_marker)
                        .finish();

                // Report violation with the correctly-positioned anchor
                results.push(LintResult::new(
                    Some(anchor),
                    vec![], // Fixes will be added below after collecting all
                    Some(description),
                    None,
                ));
            }
        }

        // If we have source fixes, create a single fix that contains all of them
        if !all_source_fixes.is_empty() && !results.is_empty() {
            // Find the first raw segment to use as an anchor for the fix
            let raw_segments = context.segment.get_raw_segments();
            if let Some(anchor_seg) = raw_segments.first() {
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
                .with_source_fixes(all_source_fixes)
                .with_position(anchor_seg.get_position_marker().cloned().unwrap())
                .finish();

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
        let result = get_whitespace_ends("{{ foo }}", "{{", "}}").unwrap();
        assert_eq!(result.opening, "{{");
        assert_eq!(result.leading_ws, " ");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, "}}");
    }

    #[test]
    fn test_get_whitespace_ends_no_whitespace() {
        let result = get_whitespace_ends("{{foo}}", "{{", "}}").unwrap();
        assert_eq!(result.opening, "{{");
        assert_eq!(result.leading_ws, "");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, "");
        assert_eq!(result.closing, "}}");
    }

    #[test]
    fn test_get_whitespace_ends_excessive_whitespace() {
        let result = get_whitespace_ends("{{   foo   }}", "{{", "}}").unwrap();
        assert_eq!(result.opening, "{{");
        assert_eq!(result.leading_ws, "   ");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, "   ");
        assert_eq!(result.closing, "}}");
    }

    #[test]
    fn test_get_whitespace_ends_block() {
        let result = get_whitespace_ends("{% if x %}", "{%", "%}").unwrap();
        assert_eq!(result.opening, "{%");
        assert_eq!(result.leading_ws, " ");
        assert_eq!(result.content, "if x");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, "%}");
    }

    #[test]
    fn test_get_whitespace_ends_comment() {
        let result = get_whitespace_ends("{# comment #}", "{#", "#}").unwrap();
        assert_eq!(result.opening, "{#");
        assert_eq!(result.leading_ws, " ");
        assert_eq!(result.content, "comment");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, "#}");
    }

    #[test]
    fn test_get_whitespace_ends_with_modifier() {
        let result = get_whitespace_ends("{{- foo -}}", "{{", "}}").unwrap();
        assert_eq!(result.opening, "{{-");
        assert_eq!(result.leading_ws, " ");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, "-}}");
    }

    #[test]
    fn test_get_whitespace_ends_custom_variable_delimiter() {
        // Snowflake CLI style `<% var %>` delimiters.
        let result = get_whitespace_ends("<%foo%>", "<%", "%>").unwrap();
        assert_eq!(result.opening, "<%");
        assert_eq!(result.leading_ws, "");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, "");
        assert_eq!(result.closing, "%>");
    }

    #[test]
    fn test_get_whitespace_ends_custom_delimiter_with_whitespace() {
        let result = get_whitespace_ends("<<  foo >>", "<<", ">>").unwrap();
        assert_eq!(result.opening, "<<");
        assert_eq!(result.leading_ws, "  ");
        assert_eq!(result.content, "foo");
        assert_eq!(result.trailing_ws, " ");
        assert_eq!(result.closing, ">>");
    }

    #[test]
    fn test_get_whitespace_ends_mismatched_delimiter() {
        // A tag whose ends don't match the supplied delimiter pair is ignored.
        assert!(get_whitespace_ends("<%foo%>", "{{", "}}").is_none());
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
