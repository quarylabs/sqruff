use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::SyntaxKind;
use crate::utils::reflow::sequence::{ReflowSequence, TargetSide};

#[derive(Debug, Default, Clone)]
pub struct RuleLT11;

impl Rule for RuleLT11 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT11.erased())
    }
    fn name(&self) -> &'static str {
        "layout.set_operators"
    }

    fn description(&self) -> &'static str {
        "Set operators should be surrounded by newlines."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `UNION ALL` is not on a line itself.

```sql
SELECT 'a' AS col UNION ALL
SELECT 'b' AS col
```

**Best practice**

Place `UNION ALL` on its own line.

```sql
SELECT 'a' AS col
UNION ALL
SELECT 'b' AS col
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }
    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack.first().unwrap().clone(),
            TargetSide::Both,
            context.config.unwrap(),
        )
        .rebreak()
        .results()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new([SyntaxKind::SetOperator].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::RuleLT11;
    use crate::api::simple::fix;
    use crate::core::rules::base::{Erased, ErasedRule};

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT11.erased()]
    }

    #[test]
    fn test_fail_simple_fix_union_all_before() {
        let sql = "SELECT 'a' UNION ALL\nSELECT 'b'";

        let result = fix(sql, rules());
        assert_eq!(result, "SELECT 'a'\nUNION ALL\nSELECT 'b'");
    }
}
