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
        let children = segment.children_all();

        let last_content = match children.into_iter().rev().find(|seg| seg.is_code()) {
            Some(seg) => seg,
            None => return Vec::new(),
        };

        let mut fixes = Vec::new();

        if self.select_clause_trailing_comma == "forbid" {
            if last_content.is_type(SyntaxKind::Comma) {
                // The last content is a comma. Before we try and remove it, we
                // should check that it's safe. One edge case is that it's a trailing
                // comma in a loop, but that if we try and remove it, we also break
                // the previous examples. We should check that this comma doesn't
                // share a source position with any other commas in the same select.
                match last_content.get_position_marker() {
                    // If there isn't a source position, then it's safe to remove,
                    // it's a recent addition.
                    None => fixes = vec![LintFix::delete(last_content.clone())],
                    Some(marker) => {
                        // NOTE: SQLFluff compares `pos_marker.source_position()`
                        // here, but sqruff's `source_position()` is derived from
                        // the templated slice, so commas expanded from a single
                        // source comma in a loop report different positions. The
                        // `source_slice` is the actual range in the source file,
                        // which is shared by all those commas, so we compare that
                        // instead to detect the unsafe case.
                        let comma_slice = marker.source_slice.clone();
                        let mut safe = true;

                        for seg in rule_cx.segment.segments() {
                            if seg.is_type(SyntaxKind::Comma) {
                                let Some(seg_marker) = seg.get_position_marker() else {
                                    continue;
                                };
                                // NOTE: Compare by identity (`is`), not value:
                                // templated commas in a loop share a raw, type and
                                // source slice, so a value comparison would treat
                                // them all as equal to `last_content`.
                                if seg_marker.source_slice == comma_slice && !seg.is(&last_content)
                                {
                                    // Not safe to fix
                                    safe = false;
                                    break;
                                }
                            }
                        }

                        // No matching commas found. It's safe.
                        if safe {
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

// The templated safety-valve case (a trailing comma produced inside a Jinja
// loop, where all the commas share a single source position) requires the
// Jinja templater, which is only available behind the `python` feature. The
// standard YAML rule-case harness also can't express "the fix leaves the file
// unchanged", so the behaviour is verified here instead.
#[cfg(all(test, feature = "python"))]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    const TEMPLATED_TRAILING_COMMA: &str = "SELECT
    {% for col in ['a', 'b', 'c'] %}
        {{col}},
    {% endfor %}
FROM tbl
";

    fn forbid_jinja_linter() -> Linter {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
rules = CV03
dialect = ansi
templater = jinja

[sqruff:rules:convention.select_trailing_comma]
select_clause_trailing_comma = forbid
"#,
            None,
        );

        Linter::new(config, None, None, true).unwrap()
    }

    #[test]
    fn test_cv03_templated_trailing_comma_is_flagged() {
        let mut linter = forbid_jinja_linter();
        let linted = linter
            .lint_string_wrapped(TEMPLATED_TRAILING_COMMA, false)
            .unwrap();

        assert_ne!(linted.violations(), &[]);
    }

    #[test]
    fn test_cv03_templated_trailing_comma_is_not_fixed() {
        // The trailing comma is shared across loop iterations, so removing it is
        // not safe: the fix must be suppressed and the file left unchanged.
        let mut linter = forbid_jinja_linter();
        let linted = linter
            .lint_string_wrapped(TEMPLATED_TRAILING_COMMA, true)
            .unwrap();

        assert_eq!(linted.fix_string(), TEMPLATED_TRAILING_COMMA);
    }
}
