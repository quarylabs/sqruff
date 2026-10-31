use sqruff_lib::core::config::FluffConfig;

use crate::formatters::rules::RulesFormatter;

pub(crate) fn rules_info(config: FluffConfig) {
    RulesFormatter::new(
        config.get("nocolor", "core").as_bool(),
        config
            .get("output_line_length", "core")
            .as_int()
            .unwrap_or(80),
    )
    .rules_info();
}
