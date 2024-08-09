use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::{ErasedSegment, TokenData};
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Clone)]
pub struct RuleCV03 {
    select_clause_trailing_comma: String,
}

impl Default for RuleCV03 {
    fn default() -> Self {
        RuleCV03 { select_clause_trailing_comma: "require".to_string() }
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

    fn eval(&self, rule_cx: RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(rule_cx.clone()).segment();
        let children = segment.children(None);

        let last_content: ErasedSegment =
            children.clone().last().cloned().filter(|sp: &ErasedSegment| sp.is_code()).unwrap();

        let mut fixes = Vec::new();

        if self.select_clause_trailing_comma == "forbid" {
            if last_content.is_type(SyntaxKind::Comma) {
                if last_content.get_position_marker().is_none() {
                    fixes = vec![LintFix::delete(last_content.clone())];
                } else {
                    let comma_pos = last_content.get_position_marker().unwrap().source_position();

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
                    None,
                    "Trailing comma in select statement forbidden".to_owned().into(),
                    None,
                )];
            }
        } else if self.select_clause_trailing_comma == "require"
            && !last_content.is_type(SyntaxKind::Comma)
        {
            let new_comma = TokenData::symbol(rule_cx.tables.next_id(), ",");

            let fix: Vec<LintFix> = vec![LintFix::replace(
                last_content.clone(),
                vec![last_content.clone(), new_comma],
                None,
            )];

            return vec![LintResult::new(
                Some(last_content),
                fix,
                None,
                "Trailing comma in select statement required".to_owned().into(),
                None,
            )];
        }
        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectClause]) }).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::convention::cv03::RuleCV03;

    fn rules() -> Vec<ErasedRule> {
        rules_with_config("require".to_owned())
    }

    fn rules_with_config(select_clause: String) -> Vec<ErasedRule> {
        vec![RuleCV03 { select_clause_trailing_comma: select_clause }.erased()]
    }

    #[test]
    fn test_require_pass() {
        let pass_str = "SELECT a, b, FROM foo";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_require_fail() {
        let fail_str = "SELECT a, b FROM foo";
        let fix_str = "SELECT a, b, FROM foo";

        let result = fix(fail_str, rules());
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_default_fail() {
        let fail_str = "SELECT a, b FROM foo";
        let fix_str = "SELECT a, b, FROM foo";

        let result = fix(fail_str, vec![RuleCV03::default().erased()]);
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_forbid_pass() {
        let pass_str = "SELECT a, b FROM foo";

        let violations = lint(
            pass_str.into(),
            "ansi".into(),
            rules_with_config("forbid".to_owned()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_forbid_fail() {
        let fail_str = "SELECT a, b, FROM foo";
        let fix_str = "SELECT a, b FROM foo";

        let result = fix(fail_str, rules_with_config("forbid".to_owned()));
        assert_eq!(fix_str, result);
    }

    #[test]
    #[ignore]
    fn test_fail_templated() {
        let fail_str = r#"
        SELECT
        {% for col in ['a', 'b', 'c'] %}
            {{col}},
        {% endfor %}
    FROM tbl
    "#;
        let fix_str = r#"
        SELECT
        {% for col in ['a', 'b', 'c'] %}
            {{col}},
        {% endfor %}
    FROM tbl"#;

        let result = fix(fail_str, rules_with_config("forbid".to_owned()));
        assert_eq!(fix_str, result);
    }
}
