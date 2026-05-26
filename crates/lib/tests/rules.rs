use std::borrow::Cow;
use std::str::FromStr;

use glob::glob;
use hashbrown::HashMap;
use serde::Deserialize;
use serde_with::{KeyValueMap, serde_as};
use sqruff_lib::api::{Engine, EngineOptions, ParseErrors, Source, SourceId, SqruffError};
use sqruff_lib::config::{ConfigPatch, FluffConfig, Value};
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
    configs: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TestCaseKind {
    Pass { pass_str: String },
    Fix { fail_str: String, fix_str: String },
    Fail { fail_str: String },
}

fn config_for_rule_case(rule: &str, case: &TestCase) -> Result<FluffConfig, SqruffError> {
    let mut patch = ConfigPatch::from_sections(case.configs.clone());

    // Set the rule under test using the public config spelling; normal config
    // normalization will expand this into the internal allowlist representation.
    patch.set_string(&["core", "rules"], rule);

    // SQLFluff rule fixtures sometimes put indentation options under `rules`;
    // Sqruff's engine expects these under `indentation`.
    for key in INDENT_CONFIG {
        if let Some(value) = patch.value(&["rules", key]).cloned() {
            patch.set_value(&["indentation", key], value);
        }
    }

    FluffConfig::builder().patch(patch).build()
}

fn unsupported_templater(case: &TestCase) -> Option<String> {
    let templater = case
        .configs
        .get("core")
        .and_then(Value::as_map)
        .and_then(|core| core.get("templater"))
        .and_then(Value::as_string)?;

    TemplaterKind::from_name(templater)
        .err()
        .map(|error| format!("unsupported templater for this build: {error}"))
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
        for case in file.cases {
            println!("Processing case: {}", case.name);
            let dialect_name = case
                .configs
                .get("core")
                .and_then(|it| it.as_map())
                .and_then(|it| it.get("dialect"))
                .and_then(|it| it.as_string())
                .unwrap_or("ansi");

            let dialect = DialectKind::from_str(dialect_name);

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
                        println!("Skipping case '{}': {}", case.name, error);
                        continue;
                    }
                    panic!("Invalid config in case '{}': {}", case.name, error);
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
                            id: SourceId::Virtual(case.name.clone()),
                            text: Cow::Borrowed(&pass_str),
                        })
                        .unwrap();
                    let error_string = format!(
                        r#"
The following test test can be used to recreate the issue:

#[cfg(test)]
mod tests {{
    use sqruff_lib::{{api::{{Engine, EngineOptions, ParseErrors, Source, SourceId}}, config::FluffConfig}};

    #[test]
    fn test_example() {{
        let config = FluffConfig::try_from_source("
[sqruff]
rules = {rule}
dialect = {dialect}
",
 None).unwrap();

        let engine = Engine::new(config, EngineOptions {{ parse_errors: ParseErrors::Include }}).unwrap();

        let pass_str = r"{pass_str}";

        let f = engine.check_source(Source {{
            id: SourceId::Virtual("test_example".into()),
            text: pass_str.into(),
        }}).unwrap();
        assert_eq!(&f.diagnostics, &[]);
    }}
}}
"#,
                        rule = rule,
                        dialect = dialect_name,
                        pass_str = pass_str
                    );

                    assert_eq!(&result.diagnostics, &[], "{}", error_string);
                }
                TestCaseKind::Fail { fail_str } => {
                    let file = engine
                        .check_source(Source {
                            id: SourceId::Virtual(case.name.clone()),
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
                            id: SourceId::Virtual(case.name.clone()),
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
