use std::str::FromStr;

use ahash::AHashMap;
use glob::glob;
use serde::Deserialize;
use serde_with::{KeyValueMap, serde_as};
use sqruff_lib::core::config::{FluffConfig, Value};
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib_core::dialects::init::DialectKind;

#[derive(Default)]
pub struct Args {
    list: bool,
    ignored: bool,
    no_capture: bool,
}

impl Args {
    fn parse_args(&mut self, iter: impl Iterator<Item = String>) {
        for arg in iter {
            if arg == "--" {
                continue;
            }

            match arg.as_str() {
                "--list" => self.list = true,
                "--ignored" => self.ignored = true,
                "--no-capture" => self.no_capture = true,
                _ => {}
            }
        }
    }
}

static INDENT_CONFIG: &[&str] = &["indent_unit", "tab_space_size"];

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
    configs: AHashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TestCaseKind {
    Pass { pass_str: String },
    Fix { fail_str: String, fix_str: String },
    Fail { fail_str: String },
}

// FIXME: Simplify FluffConfig handling. It's quite chaotic right now.
fn main() {
    let mut args = Args::default();
    args.parse_args(std::env::args().skip(1));

    // FIXME: improve support for nextest
    if args.list {
        if !args.ignored {
            println!("rules: test");
        }

        return;
    }

    let mut linter = Linter::new(FluffConfig::default(), None, None, false);
    let mut core = AHashMap::new();
    core.insert(
        "core".to_string(),
        linter.config_mut().raw.get("core").unwrap().clone(),
    );

    for path in glob("test/fixtures/rules/std_rule_cases/*.yml").unwrap() {
        let path = path.unwrap();
        let input = std::fs::read_to_string(path).unwrap();

        let file: TestFile = serde_yaml::from_str(&input).unwrap();
        core.get_mut("core").unwrap().as_map_mut().unwrap().insert(
            "rule_allowlist".into(),
            Value::Array(vec![Value::String(file.rule.clone().into())]),
        );

        linter.config_mut().raw.extend(core.clone());
        linter.config_mut().reload_reflow();

        for case in file.cases {
            let dialect_name = case
                .configs
                .get("core")
                .and_then(|it| it.as_map())
                .and_then(|it| it.get("dialect"))
                .and_then(|it| it.as_string())
                .unwrap_or("ansi");

            let dialect = DialectKind::from_str(dialect_name);
            if !args.no_capture {
                print!("test {}::{}", file.rule, case.name);
            }

            if dialect.is_err() || case.ignored.is_some() {
                if !args.no_capture {
                    let message = case.ignored.unwrap_or_else(|| {
                        format!("ignored, dialect {dialect_name} is not supported")
                    });
                    println!(" ignored, {message}");
                }

                continue;
            }

            let template = case
                .configs
                .get("core")
                .and_then(|it| it.as_map())
                .and_then(|it| it.get("templater"))
                .and_then(|it| it.as_string());
            if let Some(template) = template {
                println!(
                    "templater not yet supported ignored, {} templating is not supported",
                    template
                );
                continue;
            }

            if !args.no_capture {
                println!();
            }

            let has_config = !case.configs.is_empty();
            let rule = case.configs.get("rules").and_then(|it| it.as_string());
            if has_config {
                *linter.config_mut() = FluffConfig::new(case.configs.clone(), None, None);
                linter.config_mut().raw.extend(core.clone());

                if let Some(core) = case.configs.get("core").and_then(|it| it.as_map()) {
                    linter
                        .config_mut()
                        .raw
                        .get_mut("core")
                        .unwrap()
                        .as_map_mut()
                        .unwrap()
                        .extend(core.clone());
                }

                for (config, value) in &case
                    .configs
                    .get("rules")
                    .cloned()
                    .unwrap_or_default()
                    .as_map()
                    .cloned()
                    .unwrap_or_default()
                {
                    if INDENT_CONFIG.contains(&config.as_str()) {
                        linter
                            .config_mut()
                            .raw
                            .get_mut("indentation")
                            .unwrap()
                            .as_map_mut()
                            .unwrap()
                            .insert(config.clone(), value.clone());
                    }
                }

                linter.config_mut().reload_reflow();
            }

            match case.kind {
                TestCaseKind::Pass { pass_str } => {
                    let f = linter.lint_string_wrapped(&pass_str, false);
                    let error_string = format!(
                        r#"
The following test test can be used to recreate the issue:

#[cfg(test)]
mod tests {{
    use crate::core::{{config::FluffConfig, linter::core::Linter}};

    #[test]
    fn test_example() {{
        let config = FluffConfig::from_source("
[sqruff]
rules = {rule}
dialect = {dialect}
",
 None);

        let mut linter = Linter::new(config, None, None, false);

        let pass_str = r"{pass_str}";

        let f = linter.lint_string_wrapped(&pass_str, false);
        assert_eq!(&f.paths[0].files[0].violations, &[]);
    }}
}}
"#,
                        rule = rule.unwrap_or(""),
                        dialect = dialect_name,
                        pass_str = pass_str
                    );

                    assert_eq!(&f.paths[0].files[0].violations, &[], "{}", error_string);
                }
                TestCaseKind::Fail { fail_str } => {
                    let f = linter.lint_string_wrapped(&fail_str, false);
                    assert_ne!(&f.paths[0].files[0].violations, &[])
                }
                TestCaseKind::Fix { fail_str, fix_str } => {
                    let f = std::mem::take(
                        &mut linter.lint_string_wrapped(&fail_str, true).paths[0].files[0],
                    )
                    .fix_string();

                    pretty_assertions::assert_eq!(f, fix_str);
                }
            }

            if has_config {
                *linter.config_mut() = FluffConfig::default();
                linter.config_mut().raw.extend(core.clone());
                linter.config_mut().reload_reflow();
            }
        }
    }
}
