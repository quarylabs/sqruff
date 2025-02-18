use sqruff_lib::cli::rules::RulesFormatter;
use sqruff_lib::core::config::FluffConfig;

// use crate::commands::RuleArgs;

pub(crate) fn rules_info(config: FluffConfig) {
    RulesFormatter::new(config.get("nocolor", "core").as_bool().unwrap_or_default()).rules_info();
}
