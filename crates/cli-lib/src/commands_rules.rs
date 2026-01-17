use sqruff_lib::core::config::FluffConfig;

use crate::formatters::rules::RulesFormatter;

pub(crate) fn rules_info(config: FluffConfig) {
    RulesFormatter::new(config.core.nocolor).rules_info();
}
