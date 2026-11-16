use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Clone)]
pub struct RuleCV03 {
    select_clause_trailing_comma: String,
}

impl Default for RuleCV03 {
    fn default() -> Self {
        RuleCV03 {
            select_clause_trailing_comma: "require".to_string(),
        }
    }
}

impl Rule for RuleCV03 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV03 {
            select_clause_trailing_comma: _config
                .get("select_clause_trailing_comma")
                .unwrap()
                .as_string()
                .unwrap()
                .to_owned(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "convention.select_trailing_comma"
    }

    fn description(&self) -> &'static str {
        "Trailing commas within select clause"
    }

    fn long_description(&self) -> &'static str {
        r#"
Set `select_clause_trailing_comma` to `require_multiline_forbid_single_line`
to require trailing commas only for multiline SELECT clauses. This uses the
rendered SQL from the SELECT keyword through the last target, ignoring trailing
comments, whitespace and commas when determining whether a clause is multiline.

**Anti-pattern**

In this example, the last selected column has a trailing comma.

```sql
SELECT
    a,
    b,
FROM foo
```

**Best practice**

Remove the trailing comma.

```sql
SELECT
    a,
    b
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Convention]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn eval(&self, rule_cx: &RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(rule_cx).segment();
        let children = segment.children_all();

        let last_content = match children.into_iter().rev().find(|seg| seg.is_code()) {
            Some(seg) => seg,
            None => return Vec::new(),
        };

        // Resolve the policy per clause without changing the configured mode.
        let comma_policy: String =
            if self.select_clause_trailing_comma == "require_multiline_forbid_single_line" {
                let code_segments: Vec<_> = rule_cx
                    .segment
                    .get_raw_segments()
                    .into_iter()
                    .filter(|seg| seg.is_code() && !seg.is_type(SyntaxKind::Comma))
                    .collect();

                let (first_code, last_code) = match (code_segments.first(), code_segments.last()) {
                    (Some(first_code), Some(last_code)) => (first_code, last_code),
                    _ => return Vec::new(),
                };

                let (Some(first_marker), Some(last_marker)) = (
                    first_code.get_position_marker(),
                    last_code.get_position_marker(),
                ) else {
                    return Vec::new();
                };

                let start_line = first_marker.working_line_no;
                let end_line = last_marker.working_loc_after(last_code.raw().as_str()).0;

                if start_line != end_line {
                    "require"
                } else {
                    "forbid"
                }
                .to_string()
            } else {
                self.select_clause_trailing_comma.clone()
            };

        let mut fixes = Vec::new();

        if comma_policy == "forbid" {
            if last_content.is_type(SyntaxKind::Comma) {
                if last_content.get_position_marker().is_none() {
                    fixes = vec![LintFix::delete(last_content.clone())];
                } else {
                    let comma_pos = last_content
                        .get_position_marker()
                        .unwrap()
                        .source_position();

                    for seg in rule_cx.segment.segments() {
                        if seg.is_type(SyntaxKind::Comma) {
                            if seg.get_position_marker().is_none() {
                                continue;
                            }
                        } else if seg
                            .get_position_marker()
                            .map(|marker| marker.source_position() == comma_pos)
                            .unwrap_or(false)
                        {
                            if seg != &last_content {
                                break;
                            }
                        } else {
                            fixes = vec![LintFix::delete(last_content.clone())];
                        }
                    }
                }

                return vec![LintResult::new(
                    Some(last_content),
                    fixes,
                    "Trailing comma in select statement forbidden"
                        .to_owned()
                        .into(),
                    None,
                )];
            }
        } else if comma_policy == "require" && !last_content.is_type(SyntaxKind::Comma) {
            let new_comma = SegmentBuilder::comma(rule_cx.tables.next_id());

            let fix: Vec<LintFix> = vec![LintFix::replace(
                last_content.clone(),
                vec![last_content.clone(), new_comma],
                None,
            )];

            return vec![LintResult::new(
                Some(last_content),
                fix,
                "Trailing comma in select statement required"
                    .to_owned()
                    .into(),
                None,
            )];
        }
        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectClause]) }).into()
    }
}
