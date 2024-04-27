use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::capitalize;

#[derive(Debug, Clone)]
pub struct RuleCP01 {
    capitalisation_policy: String,
    skip_literals: bool,
    exclude_parent_types: &'static [&'static str],
}

impl Default for RuleCP01 {
    fn default() -> Self {
        Self {
            capitalisation_policy: "consistent".into(),
            skip_literals: true,
            exclude_parent_types: &["data_type", "datetime_type_identifier", "primitive_type"],
        }
    }
}

impl Rule for RuleCP01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        todo!()
    }

    fn name(&self) -> &'static str {
        "capitalisation.keywords"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of keywords."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let parent = context.parent_stack.last().unwrap();
        if self.skip_literals || self.exclude_parent_types.iter().all(|it| parent.is_type(it)) {
            return vec![LintResult::new(None, Vec::new(), None, None, None)];
        }

        if parent.get_type() == "function_name" && parent.segments().len() != 1 {
            return vec![LintResult::new(None, Vec::new(), None, None, None)];
        }

        vec![handle_segment(&self.capitalisation_policy, context.segment.clone(), &context)]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["keyword", "binary_operator", "date_part"].into()).into()
    }
}

pub fn handle_segment(
    mut extended_capitalisation_policy: &str,
    seg: ErasedSegment,
    _context: &RuleContext,
) -> LintResult {
    if seg.get_raw().unwrap().is_empty() {
        return LintResult::new(None, Vec::new(), None, None, None);
    }
    dbg!(extended_capitalisation_policy);
    if extended_capitalisation_policy == "consistent" {
        extended_capitalisation_policy = "upper";
    }

    let mut fixed_raw = seg.get_raw().unwrap();
    fixed_raw = match extended_capitalisation_policy {
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
        let consistency =
            if extended_capitalisation_policy == "consistent" { "consistently " } else { "" };
        let policy = match extended_capitalisation_policy {
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::RuleCP01;
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;

    #[test]
    fn test_fail_inconsistent_capitalisation_1() {
        let fail_str = "SeLeCt 1;";
        let fix_str = "SELECT 1;";

        let actual = fix(fail_str.into(), vec![RuleCP01::default().erased()]);
        assert_eq!(fix_str, actual);
    }
}
