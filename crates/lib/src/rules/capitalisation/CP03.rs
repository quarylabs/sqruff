use ahash::AHashMap;

use super::CP01::RuleCP01;
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Clone)]
pub struct RuleCP03 {
    base: RuleCP01,
}

impl Default for RuleCP03 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                skip_literals: false,
                exclude_parent_types: &[],
                ..Default::default()
            },
        }
    }
}

impl Rule for RuleCP03 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCP03 {
            base: RuleCP01 {
                capitalisation_policy: _config["extended_capitalisation_policy"]
                    .as_string()
                    .unwrap()
                    .into(),
                description_elem: "Function names",
                ..Default::default()
            },
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "capitalisation.functions"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of function names."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the two `SUM` functions don’t have the same capitalisation.

```sql
SELECT
    sum(a) AS aa,
    SUM(b) AS bb
FROM foo
```

**Best practice**

Make the case consistent.


```sql
SELECT
    sum(a) AS aa,
    sum(b) AS bb
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Capitalisation]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        self.base.eval(context)
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["function_name_identifier", "bare_function"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::RuleCP03;
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;
    use crate::rules::capitalisation::CP01::RuleCP01;

    #[test]
    fn test_fail_inconsistent_function_capitalisation_1() {
        let fail_str = "SELECT MAX(id), min(id) from table;";
        let fix_str = "SELECT MAX(id), MIN(id) from table;";

        let actual = fix(fail_str, vec![RuleCP03::default().erased()]);
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_inconsistent_function_capitalisation_2() {
        let fail_str = "SELECT MAX(id), min(id) from table;";
        let fix_str = "SELECT max(id), min(id) from table;";

        let actual = fix(
            fail_str,
            vec![
                RuleCP03 {
                    base: RuleCP01 { capitalisation_policy: "lower".into(), ..Default::default() },
                }
                .erased(),
            ],
        );

        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_bare_functions_3() {
        let fail_str = "SELECT current_timestamp, min(a) from table;";
        let fix_str = "SELECT Current_Timestamp, Min(a) from table;";

        let actual = fix(
            fail_str,
            vec![
                RuleCP03 {
                    base: RuleCP01 { capitalisation_policy: "pascal".into(), ..Default::default() },
                }
                .erased(),
            ],
        );

        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_capitalization_after_comma() {
        let fail_str = "SELECT FLOOR(dt) ,count(*) FROM test;";
        let fix_str = "SELECT FLOOR(dt) ,COUNT(*) FROM test;";

        let actual = fix(fail_str, vec![RuleCP03::default().erased()]);
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_pass_fully_qualified_function_mixed_functions() {
        let pass_str = "SELECT COUNT(*), project1.foo(value1) AS value2;";

        let actual = fix(pass_str, vec![RuleCP03::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_pass_fully_qualified_function_pascal_case() {
        let pass_str = "SELECT project1.FoO(value1) AS value2";

        let actual = fix(pass_str, vec![RuleCP03::default().erased()]);
        assert_eq!(pass_str, actual);
    }
}
