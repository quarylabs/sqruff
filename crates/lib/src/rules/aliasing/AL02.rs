use ahash::AHashMap;

use super::AL01::{Aliasing, RuleAL01};
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Clone)]
pub struct RuleAL02 {
    base: RuleAL01,
}

impl Default for RuleAL02 {
    fn default() -> Self {
        Self { base: RuleAL01::default().target_parent_types(&["select_clause_element"]) }
    }
}

impl RuleAL02 {
    pub fn aliasing(mut self, aliasing: Aliasing) -> Self {
        self.base = self.base.aliasing(aliasing);
        self
    }
}

impl Rule for RuleAL02 {
    fn from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleAL02::default().erased()
    }

    fn name(&self) -> &'static str {
        "aliasing.column"
    }

    fn description(&self) -> &'static str {
        "Implicit/explicit aliasing of columns."
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["alias_expression"].into()).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if FunctionalContext::new(context.clone())
            .segment()
            .children(None)
            .last()
            .unwrap()
            .get_raw()
            .unwrap()
            == "="
        {
            return Vec::new();
        }

        self.base.eval(context)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;
    use crate::rules::aliasing::AL01::Aliasing;
    use crate::rules::aliasing::AL02::RuleAL02;

    #[test]
    fn test_fail_explicit_column_default() {
        assert_eq!(
            fix("select 1 bar from table1 b".into(), vec![RuleAL02::default().erased()]),
            "select 1 AS bar from table1 b"
        );
    }

    #[test]
    fn test_fail_explicit_column_explicit() {
        let sql = "select 1 bar from table1 b";

        let result =
            fix(sql.to_string(), vec![RuleAL02::default().aliasing(Aliasing::Explicit).erased()]);

        assert_eq!(result, "select 1 AS bar from table1 b");
    }

    #[test]
    fn test_fail_explicit_column_implicit() {
        let sql = "select 1 AS bar from table1 b";

        let result =
            fix(sql.to_string(), vec![RuleAL02::default().aliasing(Aliasing::Implicit).erased()]);

        assert_eq!(result, "select 1 bar from table1 b");
    }

    #[test]
    fn test_fail_alias_ending_raw_equals() {
        let sql = "select col1 raw_equals";
        let result =
            fix(sql.to_string(), vec![RuleAL02::default().aliasing(Aliasing::Explicit).erased()]);

        assert_eq!(result, "select col1 AS raw_equals");
    }
}
