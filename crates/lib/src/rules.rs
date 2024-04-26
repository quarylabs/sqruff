use crate::core::rules::base::{RuleManifest, RuleSet};
use crate::helpers::IndexMap;

pub mod aliasing;
pub mod ambiguous;
pub mod capitalisation;
pub mod convention;
pub mod l001;
pub mod layout;
pub mod structure;

pub fn get_ruleset() -> RuleSet {
    let mut register = IndexMap::default();

    let rules = layout::get_rules();
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
