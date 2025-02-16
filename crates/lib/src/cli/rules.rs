use std::borrow::Cow;

use crate::core::rules::base::ErasedRule;
use crate::rules::rules;

use anstyle::{AnsiColor, Style};

const BLUE: Style = AnsiColor::Blue.on_default();
const YELLOW: Style = AnsiColor::Yellow.on_default();

#[derive(Debug)]
pub struct RulesFormatter {
    rules: Vec<ErasedRule>,
    nocolor: bool,
}

impl Default for RulesFormatter {
    fn default() -> Self {
        Self {
            rules: rules(),
            nocolor: false,
        }
    }
}

impl RulesFormatter {
    pub fn new(nocolor: bool) -> Self {
        Self {
            rules: rules(),
            nocolor,
        }
    }

    fn colorize<'a>(&self, s: &'a str, style: Style) -> Cow<'a, str> {
        Self::colorize_helper(self.nocolor, s, style)
    }

    fn colorize_helper(nocolor: bool, s: &str, style: Style) -> Cow<'_, str> {
        if nocolor {
            s.into()
        } else {
            format!("{style}{s}{style:#}").into()
        }
    }

    fn format_groups(&self, rule: &ErasedRule) -> String {
        rule.groups()
            .iter()
            .map(|group| group.as_ref())
            .collect::<Vec<&str>>()
            .join(", ")
    }

    fn format_rule(&self, rule: &ErasedRule) -> String {
        let group = self.format_groups(&rule);
        let code = self.colorize(&rule.code(), BLUE);
        let name = self.colorize(&rule.name(), BLUE);
        let decription = &rule.description();
        let groups = self.colorize(&group.as_str(), YELLOW);

        format!("{code}:\t[{name}] {decription}\n\tgroups: {groups}")
    }

    pub fn rules_info(&self) {
        println!("==== sqruff - rules ====");
        for rule in self.rules.clone() {
            let rule_info = self.format_rule(&rule);
            println!("{rule_info}");
        }
    }
}
