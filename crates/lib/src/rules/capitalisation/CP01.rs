use ahash::{AHashMap, AHashSet};
use itertools::Itertools;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::capitalize;

fn is_capitalizable(character: char) -> bool {
    character.to_lowercase().ne(character.to_uppercase())
}

#[derive(Debug, Clone)]
pub struct RuleCP01 {
    pub(crate) capitalisation_policy: String,
    pub(crate) cap_policy_name: String,
    pub(crate) skip_literals: bool,
    pub(crate) exclude_parent_types: &'static [&'static str],
}

impl Default for RuleCP01 {
    fn default() -> Self {
        Self {
            capitalisation_policy: "consistent".into(),
            cap_policy_name: "capitalisation_policy".into(),
            skip_literals: true,
            exclude_parent_types: &[
                "data_type",
                "datetime_type_identifier",
                "primitive_type",
                "naked_identifier",
            ],
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

        if (self.skip_literals && context.segment.is_type("literal"))
            || !self.exclude_parent_types.is_empty()
                && self.exclude_parent_types.iter().all(|it| parent.is_type(it))
        {
            return vec![LintResult::new(None, Vec::new(), None, None, None)];
        }

        if parent.get_type() == "function_name" && parent.segments().len() != 1 {
            return vec![LintResult::new(None, Vec::new(), None, None, None)];
        }

        vec![handle_segment(
            &self.capitalisation_policy,
            &self.cap_policy_name,
            context.segment.clone(),
            &context,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["keyword", "binary_operator", "date_part"].into()).into()
    }
}

#[derive(Clone, Default)]
struct RefutedCases(AHashSet<&'static str>);

#[derive(Clone)]
struct LatestPossibleCase(String);

pub fn handle_segment(
    extended_capitalisation_policy: &str,
    cap_policy_name: &str,
    seg: ErasedSegment,
    context: &RuleContext,
) -> LintResult {
    if seg.get_raw().unwrap().is_empty() {
        return LintResult::new(None, Vec::new(), None, None, None);
    }

    let mut refuted_cases =
        context.memory.borrow().get::<RefutedCases>().cloned().unwrap_or_default().0;

    let mut first_letter_is_lowercase = false;
    for ch in seg.get_raw().unwrap().chars() {
        if is_capitalizable(ch) {
            first_letter_is_lowercase = Some(ch).into_iter().ne(ch.to_uppercase());
            break;
        }
        first_letter_is_lowercase = false;
    }

    if first_letter_is_lowercase {
        refuted_cases.extend(["upper", "capitalise", "pascal"]);
        if seg.get_raw().unwrap() != seg.get_raw().unwrap().to_lowercase() {
            refuted_cases.insert("lower");
        }
    } else {
        refuted_cases.insert("lower");

        let segment_raw = seg.get_raw().unwrap();
        if segment_raw != segment_raw.to_uppercase() {
            refuted_cases.insert("upper");
        }
        if segment_raw
            != segment_raw.to_uppercase().chars().next().unwrap().to_string()
                + segment_raw[1..].to_lowercase().as_str()
        {
            refuted_cases.insert("capitalise");
        }
        if !segment_raw.chars().all(|c| c.is_alphanumeric()) {
            refuted_cases.insert("pascal");
        }
    }

    context.memory.borrow_mut().insert(RefutedCases(refuted_cases.clone()));

    let concrete_policy = if extended_capitalisation_policy == "consistent" {
        let cap_policy_opts = match cap_policy_name {
            "capitalisation_policy" => ["upper", "lower", "capitalise"].as_slice(),
            "extended_capitalisation_policy" => {
                ["upper", "lower", "pascal", "capitalise"].as_slice()
            }
            _ => unimplemented!(),
        };

        let possible_cases =
            cap_policy_opts.iter().filter(|&it| !refuted_cases.contains(it)).collect_vec();

        dbg!(&refuted_cases);
        dbg!(&cap_policy_opts);
        dbg!(&possible_cases);

        if !possible_cases.is_empty() {
            context.memory.borrow_mut().insert(LatestPossibleCase(possible_cases[0].to_string()));
            return LintResult::new(None, Vec::new(), None, None, None);
        } else {
            context
                .memory
                .borrow()
                .get::<LatestPossibleCase>()
                .cloned()
                .unwrap_or_else(|| LatestPossibleCase("upper".into()))
                .0
        }
    } else {
        extended_capitalisation_policy.to_string()
    };

    let concrete_policy = concrete_policy.as_str();

    dbg!(concrete_policy);

    let mut fixed_raw = seg.get_raw().unwrap();
    fixed_raw = match concrete_policy {
        "upper" => fixed_raw.to_uppercase(),
        "lower" => fixed_raw.to_lowercase(),
        "capitalise" => capitalize(&fixed_raw),
        "pascal" => {
            let re = lazy_regex::regex!(r"([^a-zA-Z0-9]+|^)([a-zA-Z0-9])([a-zA-Z0-9]*)");
            re.replace_all(&fixed_raw, |caps: &regex::Captures| {
                let mut replacement_string = String::from(&caps[1]);
                let capitalized = caps[2].to_uppercase();
                replacement_string.push_str(&capitalized);
                replacement_string.push_str(&caps[3]);
                replacement_string
            })
            .into()
        }
        _ => fixed_raw,
    };

    if fixed_raw == seg.get_raw().unwrap() {
        LintResult::new(None, Vec::new(), None, None, None)
    } else {
        let consistency = if concrete_policy == "consistent" { "consistently " } else { "" };
        let policy = match concrete_policy {
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
    }
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

    #[test]
    fn test_fail_inconsistent_capitalisation_2() {
        let fail_str = "SeLeCt 1 from blah;";
        let fix_str = "SELECT 1 FROM blah;";

        let actual = fix(fail_str.into(), vec![RuleCP01::default().erased()]);
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_capitalisation_policy_lower() {
        let fail_str = "SELECT * FROM MOO ORDER BY dt DESC;";
        let fix_str = "select * from MOO order by dt desc;";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP01 { capitalisation_policy: "lower".into(), ..Default::default() }.erased()],
        );
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_capitalisation_policy_upper() {
        let fail_str = "select * from MOO order by dt desc;";
        let fix_str = "SELECT * FROM MOO ORDER BY dt DESC;";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP01 { capitalisation_policy: "upper".into(), ..Default::default() }.erased()],
        );

        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_capitalisation_policy_capitalise() {
        let fail_str = "SELECT * FROM MOO ORDER BY dt DESC;";
        let fix_str = "Select * From MOO Order By dt Desc;";

        let actual = fix(
            fail_str.into(),
            vec![
                RuleCP01 { capitalisation_policy: "capitalise".into(), ..Default::default() }
                    .erased(),
            ],
        );

        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_date_part_inconsistent_capitalisation() {
        let fail_str = "SELECT dt + interval 2 day, interval 3 HOUR;";
        let fix_str = "SELECT dt + INTERVAL 2 DAY, INTERVAL 3 HOUR;";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP01 { capitalisation_policy: "upper".into(), ..Default::default() }.erased()],
        );

        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_date_part_capitalisation_policy_lower() {
        let fail_str = "SELECT dt + interval 2 day, interval 3 HOUR;";
        let fix_str = "select dt + interval 2 day, interval 3 hour;";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP01 { capitalisation_policy: "lower".into(), ..Default::default() }.erased()],
        );

        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_date_part_capitalisation_policy_upper() {
        let fail_str = "SELECT dt + interval 2 day, interval 3 HOUR;";
        let fix_str = "SELECT dt + INTERVAL 2 DAY, INTERVAL 3 HOUR;";

        let actual = fix(
            fail_str.into(),
            vec![RuleCP01 { capitalisation_policy: "upper".into(), ..Default::default() }.erased()],
        );

        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_pass_date_part_consistent_capitalisation() {
        let pass_str = "SELECT dt + INTERVAL 2 DAY, INTERVAL 3 HOUR;";
        let expected_str = "SELECT dt + INTERVAL 2 DAY, INTERVAL 3 HOUR;";

        let actual = fix(pass_str.into(), vec![RuleCP01::default().erased()]);

        assert_eq!(expected_str, actual);
    }

    #[test]
    fn test_pass_data_type_inconsistent_capitalisation() {
        let pass_str = "CREATE TABLE table1 (account_id bigint);";
        let expected_str = "CREATE TABLE table1 (account_id bigint);";

        let actual = fix(
            pass_str.into(),
            vec![RuleCP01 { capitalisation_policy: "upper".into(), ..Default::default() }.erased()],
        );

        assert_eq!(expected_str, actual);
    }
}
