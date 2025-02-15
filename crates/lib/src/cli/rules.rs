use crate::core::rules::base::ErasedRule;
use crate::rules::rules;

use anstyle::{AnsiColor, Style};

#[derive(Debug)]
pub struct RulesFormatter {
    rules: Vec<ErasedRule>,
    _nocolor: bool,
}

impl Default for RulesFormatter {
    fn default() -> Self {
        Self {
            rules: rules(),
            _nocolor: false,
        }
    }
}

impl RulesFormatter {
    pub fn new(nocolor: bool) -> Self {
        Self {
            rules: rules(),
            _nocolor: nocolor,
        }
    }

    pub fn rules_info(&self) {
        for rule in self.rules.clone() {
            let rule_info = format_rule(&rule);
            println!("{}", rule_info);
        }
    }
}

fn format_rule(rule: &ErasedRule) -> String {
    format!(
        "\x1b[34;1m{}\x1b[0m:\t[\x1b[34;1m{}\x1b[0m] {}\n\tgroups: {}",
        &rule.code(),
        &rule.name(),
        &rule.description(),
        format_groups(&rule)
    )
}

fn format_groups(rule: &ErasedRule) -> String {
    let g = rule
        .groups()
        .iter()
        .map(|group| group.as_ref())
        .collect::<Vec<&str>>()
        .join(", ");

    let style = Style::new().fg_color(Some(AnsiColor::Yellow.into()));

    format!("{style}{g}{style:#}")
}
