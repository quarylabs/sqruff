use std::borrow::Cow;
use std::str::FromStr;

use glob::glob;
use hashbrown::HashMap;
use serde::Deserialize;
use sqruff_lib::api::{Engine, EngineOptions, ParseErrors, Source, SourceId, SqruffError};
use sqruff_lib::config::{ConfigLoader, ConfigPatch, FluffConfig, Setting};
use sqruff_lib::templaters::TemplaterKind;
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

#[derive(Debug, Deserialize)]
struct TestFile {
    rule: String,
    #[serde(flatten)]
    cases: HashMap<String, TestCase>,
}

#[derive(Debug, Deserialize)]
struct TestCase {
    ignored: Option<String>,
    #[serde(flatten)]
    kind: TestCaseKind,
    #[serde(default)]
    configs: ConfigPatch,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TestCaseKind {
    Pass { pass_str: String },
    Fix { fail_str: String, fix_str: String },
    Fail { fail_str: String },
}

fn patch_for_rule_case(rule: &str, case: &TestCase) -> ConfigPatch {
    let mut patch = case.configs.clone();

    patch.core.rules = Setting::Set(Some(
        rule.split(',')
            .map(|rule| rule.trim().to_string())
            .filter(|rule| !rule.is_empty())
            .collect(),
    ));

    patch.move_fixture_indentation_options();
    patch
}

fn config_for_rule_case(rule: &str, case: &TestCase) -> Result<FluffConfig, SqruffError> {
    ConfigLoader::new().load_patch(patch_for_rule_case(rule, case))
}

fn dialect_name(case: &TestCase) -> &str {
    match &case.configs.core.dialect {
        Setting::Set(Some(dialect)) => dialect,
        Setting::Set(None) | Setting::Unset => "ansi",
    }
}

fn unsupported_templater(case: &TestCase) -> Option<String> {
    let Setting::Set(templater) = &case.configs.core.templater else {
        return None;
    };

    TemplaterKind::from_name(templater)
        .err()
        .map(|error| format!("unsupported templater for this build: {error}"))
}

fn fixture_configs_yaml(input: &str, case_name: &str) -> String {
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(input) else {
        return "{}\n".to_string();
    };

    let Some(configs) = value
        .get(case_name)
        .and_then(|case| case.get("configs"))
        .cloned()
    else {
        return "{}\n".to_string();
    };

    serde_yaml::to_string(&configs).unwrap_or_else(|_| "{}\n".to_string())
}

fn raw_string_literal(value: &str) -> String {
    for hashes in 0..10 {
        let hashes = "#".repeat(hashes);
        if !value.contains(&format!("\"{hashes}")) {
            return format!("r{hashes}\"{value}\"{hashes}");
        }
    }

    format!("{value:?}")
}

fn main() {
    let mut args = Args::default();
    args.parse_args(std::env::args().skip(1).filter(|arg| arg != "rules"));

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
        for (case_name, case) in file.cases {
            println!("Processing case: {}", case_name);
            let dialect_name = dialect_name(&case).to_string();

            let dialect = DialectKind::from_str(&dialect_name);

            if dialect.is_err() || case.ignored.is_some() {
                let message = case
                    .ignored
                    .unwrap_or_else(|| format!("ignored, dialect {dialect_name} is not supported"));
                println!("{message}");

                continue;
            }

            if let Some(message) = unsupported_templater(&case) {
                println!("{message}");
                continue;
            }

            let rule = &file.rule;
            let config = match config_for_rule_case(rule, &case) {
                Ok(config) => config,
                Err(error) => {
                    if std::env::var("SQRUFF_SKIP_UNSUPPORTED_TEMPLATERS").is_ok() {
                        println!("Skipping case '{}': {}", case_name, error);
                        continue;
                    }
                    panic!("Invalid config in case '{}': {}", case_name, error);
                }
            };

            let engine = Engine::new(
                config,
                EngineOptions {
                    parse_errors: ParseErrors::Include,
                },
            )
            .unwrap();

            match case.kind {
                TestCaseKind::Pass { pass_str } => {
                    let result = engine
                        .check_source(Source {
                            id: SourceId::Virtual(case_name.clone()),
                            text: Cow::Borrowed(&pass_str),
                        })
                        .unwrap();
                    let configs_yaml =
                        raw_string_literal(&fixture_configs_yaml(&input, &case_name));
                    let pass_str_literal = raw_string_literal(&pass_str);
                    let error_string = format!(
                        r#"
The following test test can be used to recreate the issue:

#[cfg(test)]
mod tests {{
    use sqruff_lib::{{
        api::{{Engine, EngineOptions, ParseErrors, Source, SourceId}},
        config::{{ConfigLoader, ConfigPatch, Setting}},
    }};

    #[test]
    fn test_example() {{
        let mut patch: ConfigPatch = serde_yaml::from_str({configs_yaml}).unwrap();
        patch.core.rules = Setting::Set(Some(
            {rule_literal}
                .split(',')
                .map(str::trim)
                .filter(|rule| !rule.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        ));
        patch.move_fixture_indentation_options();

        let config = ConfigLoader::new().load_patch(patch).unwrap();

        let engine = Engine::new(config, EngineOptions {{ parse_errors: ParseErrors::Include }}).unwrap();

        let pass_str = {pass_str_literal};

        let f = engine.check_source(Source {{
            id: SourceId::Virtual("test_example".into()),
            text: pass_str.into(),
        }}).unwrap();
        assert_eq!(&f.diagnostics, &[]);
    }}
}}
"#,
                        configs_yaml = configs_yaml,
                        rule_literal = raw_string_literal(rule),
                        pass_str_literal = pass_str_literal
                    );

                    assert_eq!(&result.diagnostics, &[], "{}", error_string);
                }
                TestCaseKind::Fail { fail_str } => {
                    let file = engine
                        .check_source(Source {
                            id: SourceId::Virtual(case_name.clone()),
                            text: Cow::Borrowed(&fail_str),
                        })
                        .unwrap();
                    assert_ne!(&file.diagnostics, &[])
                }
                TestCaseKind::Fix { fail_str, fix_str } => {
                    assert_ne!(
                        &fail_str, &fix_str,
                        "Fail and fix strings should not be equal"
                    );

                    let linted = engine
                        .fix_source(Source {
                            id: SourceId::Virtual(case_name.clone()),
                            text: Cow::Borrowed(&fail_str),
                        })
                        .unwrap();
                    let actual = linted.fixed_source.unwrap();

                    pretty_assertions::assert_eq!(actual, fix_str);
                }
            }
        }
    }
}
