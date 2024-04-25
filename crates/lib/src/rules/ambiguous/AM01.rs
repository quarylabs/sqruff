use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Clone)]
pub struct RuleAM01;

impl Rule for RuleAM01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleAM01 {}.erased()
    }

    fn name(&self) -> &'static str {
        "ambiguous.distinct"
    }

    fn description(&self) -> &'static str {
        "Ambiguous use of 'DISTINCT' in a 'SELECT' statement with 'GROUP BY'."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(context.clone()).segment();

        if !segment.children(Some(|it| it.is_type("groupby_clause"))).is_empty() {
            let distinct = segment
                .children(Some(|it| it.is_type("select_clause")))
                .children(Some(|it| it.is_type("select_clause_modifier")))
                .children(Some(|it| it.is_type("keyword")))
                .select(
                    Some(|it| it.is_type("keyword") && it.get_raw_upper().unwrap() == "DISTINCT"),
                    None,
                    None,
                    None,
                );

            if !distinct.is_empty() {
                return vec![LintResult::new(
                    distinct[0].clone().into(),
                    Vec::new(),
                    None,
                    None,
                    None,
                )];
            }
        }

        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["select_statement"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::lint;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::ambiguous::AM01::RuleAM01;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleAM01.erased()]
    }

    #[test]
    fn test_pass_only_group_by() {
        let violations =
            lint("select a from b group by a".into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_distinct_and_group_by() {
        let violations =
            lint("select distinct a from b group by a".into(), "ansi".into(), rules(), None, None)
                .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Ambiguous use of 'DISTINCT' in a 'SELECT' statement with 'GROUP BY'."
        );
        assert_eq!(violations.len(), 1);
    }
}
