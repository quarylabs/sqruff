use itertools::{Itertools, chain};
use sqruff_lib_core::helpers::IndexMap;

use crate::core::rules::{ErasedRule, RuleManifest, RuleSet};

pub mod aliasing;
pub mod ambiguous;
pub mod capitalisation;
pub mod convention;
pub mod jinja;
pub mod layout;
pub mod references;
pub mod structure;

pub fn rules() -> Vec<ErasedRule> {
    chain!(
        aliasing::rules(),
        ambiguous::rules(),
        capitalisation::rules(),
        convention::rules(),
        jinja::rules(),
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
                rule_class: rule,
            },
        );
    }

    RuleSet { register }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rules::RuleGroups;

    #[test]
    fn no_rule_should_not_include_all_as_that_is_default() {
        rules().iter().for_each(|rule| {
            assert_eq!(*rule.groups().first().unwrap(), RuleGroups::All);
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
            if groups.contains(&RuleGroups::Core) {
                assert_eq!(groups.get(1).unwrap(), &RuleGroups::Core);
            }
        })
    }

    #[test]
    fn rule_skip_dialect_should_have_no_duplicates() {
        rules().iter().for_each(|rule| {
            let skips = rule.dialect_skip();
            assert_eq!(skips.len(), skips.iter().unique().count());
        })
    }

    #[test]
    fn rule_skip_dialect_should_be_alphabetical() {
        rules().iter().for_each(|rule| {
            let skips = rule.dialect_skip();
            for pair in skips.windows(2) {
                if pair[1].as_ref() < pair[0].as_ref() {
                    panic!("not in alphabetical order in rule {}", rule.code())
                }
            }
        })
    }

    #[test]
    fn documented_config_options_are_unique_per_rule() {
        rules().iter().for_each(|rule| {
            let options = rule.config_options();
            let names = options.iter().map(|option| option.name).collect_vec();
            assert_eq!(
                names.len(),
                names.iter().unique().count(),
                "duplicate config option in rule {}",
                rule.code()
            );
        })
    }

    #[test]
    fn documented_config_options_have_a_description() {
        rules().iter().for_each(|rule| {
            for option in rule.config_options() {
                assert!(
                    !option.description.is_empty(),
                    "config option `{}` of rule {} has no description",
                    option.name,
                    rule.code()
                );
            }
        })
    }
}

#[cfg(test)]
mod config_validation_tests {
    use super::get_ruleset;
    use crate::core::config::FluffConfig;

    /// Build the rule pack from a config source, returning the user-facing
    /// error when the configuration does not validate.
    fn rulepack_error(source: &str) -> Option<String> {
        let config = FluffConfig::from_source(source, None);
        get_ruleset()
            .get_rulepack(&config)
            .err()
            .map(|err| err.to_string())
    }

    #[test]
    fn valid_config_builds_a_rulepack() {
        assert_eq!(
            rulepack_error(
                r#"
[sqruff]
dialect = ansi

[sqruff:rules:capitalisation.keywords]
capitalisation_policy = upper
ignore_words = foo, BAR
"#
            ),
            None
        );
    }

    #[test]
    fn unknown_enum_value_is_rejected_with_the_accepted_values() {
        let err = rulepack_error(
            r#"
[sqruff]
dialect = ansi

[sqruff:rules:capitalisation.keywords]
capitalisation_policy = uppercase
"#,
        )
        .expect("expected an error");

        assert!(
            err.contains("rule CP01 (capitalisation.keywords)")
                && err.contains("Invalid value for `capitalisation_policy`")
                && err.contains("expected one of [consistent, upper, lower, capitalise]"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn pascal_is_not_accepted_by_a_plain_capitalisation_policy() {
        let err = rulepack_error(
            r#"
[sqruff]
dialect = ansi

[sqruff:rules:capitalisation.keywords]
capitalisation_policy = pascal
"#,
        )
        .expect("expected an error");

        assert!(err.contains("got `pascal`"), "unexpected error: {err}");
    }

    #[test]
    fn pascal_is_accepted_by_an_extended_capitalisation_policy() {
        assert_eq!(
            rulepack_error(
                r#"
[sqruff]
dialect = ansi

[sqruff:rules:capitalisation.identifiers]
extended_capitalisation_policy = pascal
"#
            ),
            None
        );
    }

    #[test]
    fn non_boolean_value_is_rejected() {
        let err = rulepack_error(
            r#"
[sqruff]
dialect = ansi

[sqruff:rules:layout.long_lines]
ignore_comment_lines = yes
"#,
        )
        .expect("expected an error");

        assert!(
            err.contains("rule LT05 (layout.long_lines)")
                && err.contains("expected a boolean (`true` or `false`), got `yes`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn invalid_regex_is_rejected() {
        let err = rulepack_error(
            r#"
[sqruff]
dialect = ansi

[sqruff:rules:capitalisation.keywords]
ignore_words_regex = ^[a-
"#,
        )
        .expect("expected an error");

        assert!(
            err.contains("is not a valid regular expression"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn negative_integer_is_rejected() {
        let err = rulepack_error(
            r#"
[sqruff]
dialect = ansi
rules = LT15

[sqruff:rules:layout.newlines]
maximum_empty_lines_between_statements = -1
"#,
        )
        .expect("expected an error");

        assert!(
            err.contains("expected a non-negative integer"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn shared_rules_section_values_are_inherited_by_rules() {
        // `[sqruff:rules]` holds defaults shared by several rules, so a bad
        // value there has to be reported too.
        let err = rulepack_error(
            r#"
[sqruff]
dialect = ansi

[sqruff:rules]
unquoted_identifiers_policy = everything
"#,
        )
        .expect("expected an error");

        assert!(
            err.contains("Invalid value for `unquoted_identifiers_policy`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn a_rules_own_section_overrides_the_shared_one() {
        // `references.keywords` pins `unquoted_identifiers_policy` to `aliases`,
        // so the shared value must not leak into it.
        assert_eq!(
            rulepack_error(
                r#"
[sqruff]
dialect = ansi
rules = RF04

[sqruff:rules]
unquoted_identifiers_policy = column_aliases
"#
            ),
            None
        );
    }
}
