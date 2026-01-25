use std::io::Write;
use std::path::Path;

use minijinja::{Environment, context};
use serde::Serialize;
use sqruff_lib::core::rules::ErasedRule;
use sqruff_lib::rules::rules;
use sqruff_lib::templaters::TEMPLATERS;

use crate::commands::Cli;

#[cfg(feature = "codegen-docs")]
const RULES_TEMPLATE: &str = include_str!("docs/generate_rule_docs_template.md");
#[cfg(feature = "codegen-docs")]
const TEMPLATERS_TEMPLATE: &str = include_str!("docs/generate_templater_docs_template.md");

#[cfg(feature = "codegen-docs")]
pub(crate) fn codegen_docs() {
    let docs_dir = Path::new("docs").join("reference");
    std::fs::create_dir_all(&docs_dir).unwrap();

    // CLI Docs
    let markdown: String = clap_markdown::help_markdown::<Cli>();
    let file_cli = std::fs::File::create(docs_dir.join("cli.md")).unwrap();
    let mut writer = std::io::BufWriter::new(file_cli);
    writer.write_all(markdown.as_bytes()).unwrap();

    // Rules Docs
    let mut env = Environment::new();
    env.add_template("rules", RULES_TEMPLATE).unwrap();

    let tmpl = env.get_template("rules").unwrap();
    let rules = rules();
    let rules = rules.into_iter().map(Rule::from).collect::<Vec<_>>();
    let file_rules = std::fs::File::create(docs_dir.join("rules.md")).unwrap();
    let mut writer = std::io::BufWriter::new(file_rules);
    writer
        .write_all(tmpl.render(context!(rules => rules)).unwrap().as_bytes())
        .unwrap();

    // Templaters Docs
    let mut env = Environment::new();
    env.add_template("templaters", TEMPLATERS_TEMPLATE).unwrap();

    let tmpl = env.get_template("templaters").unwrap();
    let templaters = TEMPLATERS
        .into_iter()
        .map(Templater::from)
        .collect::<Vec<_>>();
    let file_templaters = std::fs::File::create(docs_dir.join("templaters.md")).unwrap();
    let mut writer = std::io::BufWriter::new(file_templaters);
    writer
        .write_all(
            tmpl.render(context!(templaters => templaters))
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
}

#[derive(Debug, Clone, Serialize)]
struct Templater {
    name: &'static str,
    description: &'static str,
}

impl From<&'static dyn sqruff_lib::templaters::Templater> for Templater {
    fn from(value: &'static dyn sqruff_lib::templaters::Templater) -> Self {
        Templater {
            name: value.name(),
            description: value.description(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Rule {
    pub name: &'static str,
    pub name_no_periods: String,
    pub code: &'static str,
    pub description: &'static str,
    pub fixable: bool,
    pub long_description: &'static str,
    pub groups: Vec<&'static str>,
    pub has_dialects: bool,
    pub dialects: Vec<&'static str>,
}

impl From<ErasedRule> for Rule {
    fn from(value: ErasedRule) -> Self {
        Rule {
            name: value.name(),
            name_no_periods: value.name().replace('.', ""),
            code: value.code(),
            fixable: value.is_fix_compatible(),
            description: value.description(),
            long_description: value.long_description(),
            groups: value.groups().iter().map(|g| g.as_ref()).collect(),
            has_dialects: !value.dialect_skip().is_empty(),
            dialects: value
                .dialect_skip()
                .iter()
                .map(|dialect| dialect.as_ref())
                .collect(),
        }
    }
}
