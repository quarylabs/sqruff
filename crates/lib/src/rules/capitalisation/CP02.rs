use ahash::AHashMap;

use super::CP01::RuleCP01;
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::identifers::identifiers_policy_applicable;

#[derive(Clone, Debug)]
pub struct RuleCP02 {
    base: RuleCP01,
    unquoted_identifiers_policy: &'static str,
}

impl Default for RuleCP02 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                cap_policy_name: "extended_capitalisation_policy".into(),
                description_elem: "Unquoted identifiers",
                ..Default::default()
            },
            unquoted_identifiers_policy: "all",
        }
    }
}

impl Rule for RuleCP02 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> ErasedRule {
        RuleCP02 {
            base: RuleCP01 {
                capitalisation_policy: config["extended_capitalisation_policy"]
                    .as_string()
                    .unwrap()
                    .into(),
                cap_policy_name: "extended_capitalisation_policy".into(),
                description_elem: "Unquoted identifiers",
                ..Default::default()
            },
            ..Default::default()
        }
        .erased()
    }

    fn name(&self) -> &'static str {
        "capitalisation.identifiers"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of unquoted identifiers."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, unquoted identifier `a` is in lower-case but `B` is in upper-case.

```sql
select
    a,
    B
from foo
```

**Best practice**

Ensure all unquoted identifiers are either in upper-case or in lower-case.

```sql
select
    a,
    b
from foo

-- Also good

select
    A,
    B
from foo
```
"#
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if matches!(context.dialect.name, "databricks" | "sparksql")
            && context
                .parent_stack
                .last()
                .map_or(false, |it| it.get_type() == "property_name_identifier")
            && context.segment.raw() == "enableChangeDataFeed"
        {
            return Vec::new();
        }

        if identifiers_policy_applicable(self.unquoted_identifiers_policy, &context.parent_stack) {
            self.base.eval(context)
        } else {
            vec![LintResult::new(None, Vec::new(), None, None, None)]
        }
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["naked_identifier", "properties_naked_identifier"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::RuleCP02;
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;

    #[test]
    fn test_pass_consistent_capitalisation_1() {
        let pass_str = "SELECT a, b";

        let actual = fix(pass_str, vec![RuleCP02::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_pass_consistent_capitalisation_2() {
        let pass_str = "SELECT A, B";

        let actual = fix(pass_str, vec![RuleCP02::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_pass_consistent_capitalisation_with_null() {
        let pass_str = "SELECT NULL, a";
        let actual = fix(pass_str, vec![RuleCP02::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_pass_consistent_capitalisation_with_single_letter_upper() {
        let pass_str = "SELECT A, Boo";
        let actual = fix(pass_str, vec![RuleCP02::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_pass_consistent_capitalisation_with_single_word_snake() {
        let pass_str = "SELECT Apple, Banana_split";
        let actual = fix(pass_str, vec![RuleCP02::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_pass_consistent_capitalisation_with_single_word_pascal() {
        let pass_str = "SELECT AppleFritter, Banana";
        let actual = fix(pass_str, vec![RuleCP02::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_pass_consistent_capitalisation_with_multiple_words_with_numbers() {
        let pass_str = "SELECT AppleFritter, Apple123fritter, Apple123Fritter";
        let actual = fix(pass_str, vec![RuleCP02::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_pass_consistent_capitalisation_with_leading_underscore() {
        let pass_str = "SELECT _a, b";
        let actual = fix(pass_str, vec![RuleCP02::default().erased()]);
        assert_eq!(pass_str, actual);
    }

    #[test]
    fn test_fail_inconsistent_capitalisation_lower_case() {
        // Test that fixes are consistent
        let fail_str = "SELECT a, B";
        let expected = "SELECT a, b";
        let actual = fix(fail_str, vec![RuleCP02::default().erased()]);
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_fail_inconsistent_capitalisation_2() {
        let fail_str = "SELECT B,   a";
        let expected = "SELECT B,   A";

        let actual = fix(fail_str, vec![RuleCP02::default().erased()]);
        assert_eq!(expected, actual);
    }
}
