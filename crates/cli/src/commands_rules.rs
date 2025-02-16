use sqruff_lib::cli::rules::RulesFormatter;

use crate::commands::RuleArgs;

pub(crate) fn rules_info(args: RuleArgs) {
    RulesFormatter::new(args.nocolor).rules_info();
}
