use itertools::{chain, Itertools};

use crate::core::rules::base::{ErasedRule, RuleManifest, RuleSet};
use crate::helpers::IndexMap;

pub mod aliasing;
pub mod ambiguous;
pub mod capitalisation;
pub mod convention;
pub mod layout;
pub mod references;
pub mod structure;

pub fn rules() -> Vec<ErasedRule> {
    chain!(
        aliasing::rules(),
        ambiguous::rules(),
        capitalisation::rules(),
        convention::rules(),
        layout::rules(),
        references::rules(),
        structure::rules()
    )
    .collect_vec()
}

pub fn get_ruleset() -> RuleSet {
    let mut register = IndexMap::default();

    let rules = rules();
    register.reserve(rules.len());

    for rule in rules {
        register.insert(
            rule.code(),
            RuleManifest {
                code: rule.code(),
                name: rule.name(),
                description: rule.description(),
                groups: rule.groups(),
                aliases: <_>::default(),
                rule_class: rule,
            },
        );
    }

    RuleSet { _name: "standard".into(), _config_info: <_>::default(), register }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rules::base::RuleGroups;

    #[test]
    fn no_rule_should_not_include_all_as_that_is_default() {
        rules().iter().for_each(|rule| {
            assert_eq!(*rule.groups().get(0).unwrap(), RuleGroups::All);
        });
    }

    #[test]
    fn no_should_contain_duplicate_groups() {
        rules().iter().for_each(|rule| {
            let groups = rule.groups();
            assert_eq!(groups.len(), groups.iter().unique().count());
        });
    }

    #[test]
    fn if_rule_contains_core_is_second_in_list() {
        rules().iter().for_each(|rule| {
            let groups = rule.groups();
            if groups.into_iter().find(|&&rule| rule == RuleGroups::Core).is_some() {
                assert_eq!(groups.get(1).unwrap(), &RuleGroups::Core);
            }
        })
    }
}
