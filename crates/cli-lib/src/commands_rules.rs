use sqruff_lib::config::FluffConfig;

use crate::formatters::rules::RulesFormatter;

pub(crate) fn rules_info(config: FluffConfig) {
    RulesFormatter::new(config.no_color()).rules_info();
}
