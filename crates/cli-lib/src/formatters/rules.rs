use super::utils::*;

use anstyle::{AnsiColor, Style};
use sqruff_lib::{core::rules::ErasedRule, rules::rules};
use std::borrow::Cow;

const BLUE: Style = AnsiColor::Blue.on_default();
const YELLOW: Style = AnsiColor::Yellow.on_default();

#[derive(Debug)]
pub(crate) struct RulesFormatter {
    rules: Vec<ErasedRule>,
    plain_output: bool,
}

impl Default for RulesFormatter {
    fn default() -> Self {
        Self {
            rules: rules(),
            plain_output: false,
        }
    }
}

impl RulesFormatter {
    pub(crate) fn new(nocolor: bool) -> Self {
        Self {
            rules: rules(),
            plain_output: should_produce_plain_output(nocolor),
        }
    }

    fn colorize<'a>(&self, s: &'a str, style: Style) -> Cow<'a, str> {
        colorize_helper(self.plain_output, s, style)
    }

    fn format_groups(&self, rule: &ErasedRule) -> String {
        rule.groups()
            .iter()
            .map(|group| group.as_ref())
            .collect::<Vec<&str>>()
            .join(", ")
    }

    fn format_rule(&self, rule: &ErasedRule) -> String {
        let group = self.format_groups(rule);
        let code = self.colorize(rule.code(), BLUE);
        let name = self.colorize(rule.name(), BLUE);
        let decription = &rule.description();
        let groups = self.colorize(group.as_str(), YELLOW);

        format!("{code}:\t[{name}] {decription}\n\tgroups: {groups}")
    }

    pub(crate) fn rules_info(&self) {
        println!("==== sqruff - rules ====");
        for rule in self.rules.clone() {
            let rule_info = self.format_rule(&rule);
            println!("{rule_info}");
        }
    }
}
