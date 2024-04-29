use ahash::AHashMap;

use super::CP01::handle_segment;
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Default, Clone)]
pub struct RuleCP05 {
    extended_capitalisation_policy: String,
}

impl Rule for RuleCP05 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> ErasedRule {
        RuleCP05 {
            extended_capitalisation_policy: config["extended_capitalisation_policy"]
                .as_string()
                .unwrap()
                .to_string(),
        }
        .erased()
    }

    fn name(&self) -> &'static str {
        "capitalisation.types"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of datatypes."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut results = Vec::new();

        if context.segment.is_type("primitive_type")
            || context.segment.is_type("datetime_type_identifier")
            || context.segment.is_type("data_type")
        {
            for seg in context.segment.segments() {
                if seg.is_type("symbol") && seg.is_type("symbol") && seg.is_type("symbol")
                    || seg.get_raw_segments().is_empty()
                {
                    continue;
                }

                results.push(handle_segment(
                    &self.extended_capitalisation_policy,
                    "extended_capitalisation_policy",
                    seg.clone(),
                    &context,
                ));
            }
        }

        results
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            ["data_type_identifier", "primitive_type", "datetime_type_identifier", "data_type"]
                .into(),
        )
        .into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::RuleCP05;
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;

    #[test]
    fn test_fail_data_type_inconsistent_capitalisation_1() {
        let fail_str = "CREATE TABLE table1 (account_id BiGinT);";
        let fix_str = "CREATE TABLE table1 (account_id BIGINT);";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "upper".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_inconsistent_capitalisation_2() {
        let fail_str = "CREATE TABLE table1 (account_id BiGinT);";
        let fix_str = "CREATE TABLE table1 (account_id bigint);";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "lower".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_inconsistent_capitalisation_3() {
        let fail_str = "CREATE TABLE table1 (account_id BiGinT);";
        let fix_str = "CREATE TABLE table1 (account_id Bigint);";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "capitalise".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_capitalisation_policy_lower() {
        let fail_str = "CREATE TABLE table1 (account_id BIGINT);";
        let fix_str = "CREATE TABLE table1 (account_id bigint);";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "lower".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_capitalisation_policy_lower_2() {
        let fail_str = "CREATE TABLE table1 (account_id BIGINT, column_two varchar(255));";
        let fix_str = "CREATE TABLE table1 (account_id bigint, column_two varchar(255));";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "lower".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_capitalisation_policy_upper() {
        let fail_str = "CREATE TABLE table1 (account_id bigint);";
        let fix_str = "CREATE TABLE table1 (account_id BIGINT);";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "upper".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_capitalisation_policy_upper_2() {
        let fail_str = "CREATE TABLE table1 (account_id BIGINT, column_two varchar(255));";
        let fix_str = "CREATE TABLE table1 (account_id BIGINT, column_two VARCHAR(255));";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "upper".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_capitalisation_policy_capitalise() {
        let fail_str = "CREATE TABLE table1 (account_id BIGINT);";
        let fix_str = "CREATE TABLE table1 (account_id Bigint);";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "capitalise".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_capitalisation_policy_keywords_1() {
        let fail_str = "CREATE TABLE table1 (account_id BIGINT, column_two timestamp);";
        let fix_str = "CREATE TABLE table1 (account_id BIGINT, column_two TIMESTAMP);";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "upper".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_data_type_capitalisation_policy_keywords_2() {
        let fail_str =
            "CREATE TABLE table1 (account_id BIGINT, column_two timestamp with time zone);";
        let fix_str =
            "CREATE TABLE table1 (account_id BIGINT, column_two TIMESTAMP WITH TIME ZONE);";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP05 { extended_capitalisation_policy: "upper".into() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }
}
