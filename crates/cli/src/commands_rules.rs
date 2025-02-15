use sqruff_lib::cli::rules::RulesFormatter;

use crate::commands::RuleArgs;

pub(crate) fn rules_info(_args: RuleArgs) {
    RulesFormatter::new(false).rules_info();
}
