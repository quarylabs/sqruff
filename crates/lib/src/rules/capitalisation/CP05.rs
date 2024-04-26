use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::capitalize;

#[derive(Debug, Clone)]
pub struct RuleCP05 {
    extended_capitalisation_policy: String,
}

impl RuleCP05 {
    fn handle_segment(&self, seg: ErasedSegment, _context: &RuleContext) -> LintResult {
        if seg.get_raw().unwrap().is_empty() {
            return LintResult::new(None, Vec::new(), None, None, None);
        }

        let mut fixed_raw = seg.get_raw().unwrap();
        fixed_raw = match self.extended_capitalisation_policy.as_str() {
            "upper" => fixed_raw.to_uppercase(),
            "lower" => fixed_raw.to_lowercase(),
            "capitalise" => capitalize(&fixed_raw),
            "pascal" => regex::Regex::new(r"(?:\b|_)([a-z])")
                .unwrap()
                .replace_all(&fixed_raw, |caps: &regex::Captures| caps[1].to_uppercase())
                .to_string(),
            _ => fixed_raw,
        };

        return if fixed_raw == seg.get_raw().unwrap() {
            LintResult::new(None, Vec::new(), None, None, None)
        } else {
            let consistency = if self.extended_capitalisation_policy == "consistent" {
                "consistently "
            } else {
                ""
            };
            let policy = match self.extended_capitalisation_policy.as_str() {
                concrete_policy @ ("upper" | "lower") => format!("{} case.", concrete_policy),
                "capitalise" => "capitalised.".to_string(),
                "pascal" => "pascal case.".to_string(),
                _ => "".to_string(),
            };

            LintResult::new(
                seg.clone().into(),
                vec![LintFix::replace(seg.clone(), vec![seg.edit(fixed_raw.into(), None)], None)],
                None,
                format!("{} must be {}{}", "Datatypes", consistency, policy).into(),
                None,
            )
        };
    }
}

impl Rule for RuleCP05 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        todo!()
    }

    fn name(&self) -> &'static str {
        "capitalisation.types"
    }

    fn description(&self) -> &'static str {
        todo!()
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

                results.push(self.handle_segment(seg.clone(), &context));
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
