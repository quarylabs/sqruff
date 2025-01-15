use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
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
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
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

    fn eval(&self, rule_cx: &RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(rule_cx).segment();
        let children = segment.children(None);

        let last_content: ErasedSegment = children
            .clone()
            .last()
            .cloned()
            .filter(|sp: &ErasedSegment| sp.is_code())
            .unwrap();

        let mut fixes = Vec::new();

        if self.select_clause_trailing_comma == "forbid" {
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
                        } else if seg.get_position_marker().unwrap().source_position() == comma_pos
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
        } else if self.select_clause_trailing_comma == "require"
            && !last_content.is_type(SyntaxKind::Comma)
        {
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
