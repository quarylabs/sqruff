use sqruff_lib::core::config::FluffConfig;

use crate::formatters::rules::RulesFormatter;

pub(crate) fn rules_info(config: FluffConfig) {
    RulesFormatter::new(config.core.nocolor.unwrap_or_default()).rules_info();
}
