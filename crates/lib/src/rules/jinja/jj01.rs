use ahash::AHashMap;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::fix::SourceFix;
use sqruff_lib_core::parser::segments::{ErasedSegment, NodeOrTokenKind, SegmentBuilder};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Default, Clone)]
pub struct RuleJJ01;

impl RuleJJ01 {
    fn get_whitespace_ends(s: &str) -> (String, String, String, String, String) {
        assert!(s.starts_with('{') && s.ends_with('}'));
        let mut main = &s[2..s.len() - 2];
        let mut pre = &s[..2];
        let mut post = &s[s.len() - 2..];
        let modifier_chars = ['+', '-'];
        if !main.is_empty() && modifier_chars.contains(&main.chars().next().unwrap()) {
            pre = &s[..3];
            main = &s[3..s.len() - 2];
        }
        if !main.is_empty() && modifier_chars.contains(&main.chars().last().unwrap()) {
            post = &s[s.len() - 3..];
            main = &main[..main.len() - 1];
        }
        let inner = main.trim();
        let pos = main.find(inner).unwrap_or(0);
        (
            pre.to_string(),
            main[..pos].to_string(),
            inner.to_string(),
            main[pos + inner.len()..].to_string(),
            post.to_string(),
        )
    }

    fn find_raw_at_src_idx(segment: &ErasedSegment, src_idx: usize) -> Option<ErasedSegment> {
        if segment.segments().is_empty() {
            return None;
        }
        for seg in segment.segments() {
            if let Some(pos_marker) = seg.get_position_marker() {
                let src_slice = pos_marker.source_slice.clone();
                if src_slice.end <= src_idx {
                    continue;
                }
                if seg.segments().is_empty() {
                    return Some(seg.clone());
                } else {
                    if let Some(res) = Self::find_raw_at_src_idx(seg, src_idx) {
                        return Some(res);
                    }
                }
            }
        }
        None
    }
}

impl Rule for RuleJJ01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleJJ01.erased())
    }

    fn name(&self) -> &'static str {
        "jinja.padding"
    }

    fn description(&self) -> &'static str {
        "Jinja tags should have a single whitespace on either side."
    }

    fn long_description(&self) -> &'static str {
        r#"Jinja tags should have a single whitespace on either side.

**Anti-pattern**

Jinja tags with either no whitespace or very long whitespace are hard to read.

```jinja
SELECT {{    a     }} from {{ref('foo')}}
```

**Best practice**

A single whitespace surrounding Jinja tags, alternatively longer gaps containing newlines are acceptable.

```jinja
SELECT {{ a }} from {{ ref('foo') }};
SELECT {{ a }} from {{
    ref('foo')
}};
```"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Jinja]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let pos_marker = match context.segment.get_position_marker() {
            Some(pm) => pm,
            None => return Vec::new(),
        };
        if pos_marker.is_literal() {
            return Vec::new();
        }
        let templater = context
            .config
            .get("templater", "core")
            .as_string()
            .unwrap_or("raw");
        if templater != "jinja" && templater != "dbt" {
            return Vec::new();
        }
        let templated_file = match &context.templated_file {
            Some(tf) => tf,
            None => return Vec::new(),
        };
        let mut results = Vec::new();
        for raw_slice in templated_file.raw_slices() {
            match raw_slice.slice_type() {
                "templated" | "block_start" | "block_end" => {}
                _ => continue,
            }
            let stripped = raw_slice.raw().trim();
            if stripped.is_empty() || !stripped.starts_with('{') || !stripped.ends_with('}') {
                continue;
            }
            let (tag_pre, ws_pre, inner, ws_post, tag_post) = Self::get_whitespace_ends(stripped);
            let mut pre_fix: Option<String> = None;
            let mut post_fix: Option<String> = None;
            if ws_pre.is_empty() || (ws_pre != " " && !ws_pre.contains('\n')) {
                pre_fix = Some(" ".into());
            }
            if ws_post.is_empty() || (ws_post != " " && !ws_post.contains('\n')) {
                post_fix = Some(" ".into());
            }
            if pre_fix.is_none() && post_fix.is_none() {
                continue;
            }
            let fixed = format!(
                "{}{}{}{}{}",
                tag_pre,
                pre_fix.clone().unwrap_or(ws_pre.clone()),
                inner,
                post_fix.clone().unwrap_or(ws_post.clone()),
                tag_post
            );
            let src_idx = raw_slice.source_idx;
            let position = raw_slice
                .raw()
                .find(stripped.chars().next().unwrap())
                .unwrap_or(0);
            let Some(raw_seg) = Self::find_raw_at_src_idx(&context.segment, src_idx) else {
                continue;
            };
            if !raw_seg.get_source_fixes().is_empty() {
                continue;
            }
            let source_fix = SourceFix::new(
                fixed.clone().into(),
                src_idx + position..src_idx + position + stripped.len(),
                raw_seg
                    .get_position_marker()
                    .unwrap()
                    .templated_slice
                    .clone(),
            );
            let mut edit_seg = SegmentBuilder::node(
                context.tables.next_id(),
                raw_seg.get_type(),
                context.dialect.name,
                vec![raw_seg.clone()],
            )
            .with_position(raw_seg.get_position_marker().unwrap().clone())
            .finish();
            if let NodeOrTokenKind::Node(node) = &mut edit_seg.make_mut().kind {
                node.source_fixes.push(source_fix);
            }
            let fix = LintFix::replace(raw_seg.clone(), vec![edit_seg], None);
            results.push(LintResult::new(
                Some(raw_seg),
                vec![fix],
                Some(format!(
                    "Jinja tags should have a single whitespace on either side: {}",
                    stripped
                )),
                None,
            ));
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
