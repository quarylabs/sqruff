use std::str::FromStr;

use glob::glob;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_with::{KeyValueMap, serde_as};
use sqruff_lib::core::config::{FluffConfig, IndentUnitType};
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib_core::dialects::init::DialectKind;

#[derive(Default)]
pub struct Args {
    file: Option<String>,
}

impl Args {
    fn parse_args(&mut self, mut iter: impl Iterator<Item = String>) {
        self.file = iter.find(|arg| arg != "--");
    }
}

fn value_to_i32(value: &JsonValue) -> Option<i32> {
    value
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<i32>().ok()))
}

fn apply_indent_overrides(config: &mut FluffConfig, rules: &serde_json::Map<String, JsonValue>) {
    if let Some(value) = rules.get("indent_unit").and_then(|value| value.as_str()) {
        config.indentation.indent_unit = Some(IndentUnitType::from_str(value).unwrap());
    }

    if let Some(value) = rules.get("tab_space_size").and_then(value_to_i32) {
        config.indentation.tab_space_size = Some(value);
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct TestFile {
    rule: String,
    #[serde_as(as = "KeyValueMap<_>")]
    #[serde(flatten)]
    cases: Vec<TestCase>,
}

#[derive(Debug, Deserialize)]
struct TestCase {
    #[serde(rename = "$key$")]
    name: String,
    ignored: Option<String>,
    #[serde(flatten)]
    kind: TestCaseKind,
    #[serde(default)]
    configs: JsonValue,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TestCaseKind {
    Pass { pass_str: String },
    Fix { fail_str: String, fix_str: String },
    Fail { fail_str: String },
}

fn main() {
    let mut args = Args::default();
    args.parse_args(std::env::args().skip(1));

    let pattern = args
        .file
        .as_deref()
        .map(|f| format!("test/fixtures/rules/std_rule_cases/{f}"))
        .unwrap_or_else(|| "test/fixtures/rules/std_rule_cases/*.yml".to_string());

    for path in glob(&pattern).unwrap() {
        let path = path.unwrap();
        println!("Processing file: {:?}", path);
        let input = std::fs::read_to_string(path).unwrap();

        let file: TestFile = serde_yaml::from_str(&input).unwrap();
        let file_rules = file
            .rule
            .split(',')
            .map(|rule| rule.trim().to_string())
            .collect::<Vec<String>>();
        let mut base_config = FluffConfig::default();
        base_config.core.rule_allowlist = Some(file_rules.clone());
        base_config.reload_reflow();

        for case in file.cases {
            println!("Processing case: {}", case.name);
            let dialect_name = case
                .configs
                .get("core")
                .and_then(|it| it.as_object())
                .and_then(|it| it.get("dialect"))
                .and_then(|it| it.as_str())
                .unwrap_or("ansi");

            let templater_name = case
                .configs
                .get("core")
                .and_then(|it| it.as_object())
                .and_then(|it| it.get("templater"))
                .and_then(|it| it.as_str());

            let dialect = DialectKind::from_str(dialect_name);

            if dialect.is_err() || case.ignored.is_some() {
                let message = case
                    .ignored
                    .unwrap_or_else(|| format!("ignored, dialect {dialect_name} is not supported"));
                println!("{message}");

                continue;
            }

            // Skip tests that use templaters not available in this build
            if matches!(templater_name, Some("jinja") | Some("dbt") | Some("python")) {
                println!("ignored, templater {templater_name:?} is not available in this build");
                continue;
            }

            let rule = &file.rule;
            let config = if case
                .configs
                .as_object()
                .map(|configs| configs.is_empty())
                .unwrap_or(true)
            {
                base_config.clone()
            } else {
                let mut config: FluffConfig =
                    serde_json::from_value(case.configs.clone()).unwrap_or_default();
                config.core.rule_allowlist = Some(file_rules.clone());

                if let Some(rules) = case.configs.get("rules").and_then(|it| it.as_object()) {
                    apply_indent_overrides(&mut config, rules);
                }

                config.reload_reflow();
                config
            };
            let mut linter = Linter::new(config, None, None, true);

            match case.kind {
                TestCaseKind::Pass { pass_str } => {
                    let result = linter.lint_string_wrapped(&pass_str, false);
                    let error_string = format!(
                        r#"
The following test test can be used to recreate the issue:

#[cfg(test)]
mod tests {{
    use sqruff_lib::core::{{config::FluffConfig, linter::core::Linter}};

    #[test]
    fn test_example() {{
        let config = FluffConfig::from_source("
[sqruff]
rules = {rule}
dialect = {dialect}
",
 None).unwrap();

        let mut linter = Linter::new(config, None, None, true);

        let pass_str = r"{pass_str}";

        let f = linter.lint_string_wrapped(&pass_str, false);
        assert_eq!(&f.violations, &[]);
    }}
}}
"#,
                        rule = rule,
                        dialect = dialect_name,
                        pass_str = pass_str
                    );

                    assert_eq!(&result.violations(), &[], "{}", error_string);
                }
                TestCaseKind::Fail { fail_str } => {
                    let file = linter.lint_string_wrapped(&fail_str, false);
                    assert_ne!(&file.violations(), &[])
                }
                TestCaseKind::Fix { fail_str, fix_str } => {
                    assert_ne!(
                        &fail_str, &fix_str,
                        "Fail and fix strings should not be equal"
                    );

                    let linted = linter.lint_string_wrapped(&fail_str, true);
                    let actual = linted.fix_string();

                    pretty_assertions::assert_eq!(actual, fix_str);
                }
            }
        }
    }
}
