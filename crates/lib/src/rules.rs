use ahash::AHashMap;

use crate::core::rules::base::{RuleManifest, RuleSet};

pub mod aliasing;
pub mod convention;
pub mod l001;
pub mod layout;
pub mod structure;

pub fn get_ruleset() -> RuleSet {
    let mut register = AHashMap::default();

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

    RuleSet { name: "standard".into(), config_info: <_>::default(), register }
}
