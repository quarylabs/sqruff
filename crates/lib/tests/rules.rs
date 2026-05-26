use std::path::Path;
use std::str::FromStr;

use glob::glob;
use hashbrown::HashMap;
use rayon::prelude::*;
use serde::Deserialize;
use serde_with::{KeyValueMap, serde_as};
use sqruff_lib::api::{Mode, ParseErrors};
use sqruff_lib::core::config::{FluffConfig, Value};
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

// FIXME: Simplify FluffConfig handling. It's quite chaotic right now.
fn main() {
    let mut args = Args::default();
    args.parse_args(std::env::args().skip(1));

    let pattern = args
        .file
        .as_deref()
        .map(|f| format!("test/fixtures/rules/std_rule_cases/{f}"))
        .unwrap_or_else(|| "test/fixtures/rules/std_rule_cases/*.yml".to_string());

    let mut paths = glob(&pattern)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    paths.sort();

    let verbose = std::env::var_os("SQRUFF_RULE_TEST_VERBOSE").is_some();
    paths
        .par_iter()
        .for_each_init(RuleTestState::new, |state, path| {
            process_file(state, path, verbose)
        });
}

// FIXME: Simplify FluffConfig handling. It's quite chaotic right now.
struct RuleTestState {
    linter: Linter,
    core: HashMap<String, Value>,
}

impl RuleTestState {
    fn new() -> Self {
        let mut linter = Linter::new(FluffConfig::default(), None, ParseErrors::Include).unwrap();
        let mut core = HashMap::new();
        core.insert(
            "core".to_string(),
            linter.config_mut().raw.get("core").unwrap().clone(),
        );

        Self { linter, core }
    }
}

fn process_file(state: &mut RuleTestState, path: &Path, verbose: bool) {
    if verbose {
        println!("Processing file: {:?}", path);
    }
    let input = std::fs::read_to_string(path).unwrap();

    let file: TestFile = serde_yaml::from_str(&input).unwrap();
    let file_rules = file
        .rule
        .split(",")
        .map(|x| Value::String(x.into()))
        .collect::<Vec<Value>>();

    state
        .core
        .get_mut("core")
        .unwrap()
        .as_map_mut()
        .unwrap()
        .insert("rule_allowlist".into(), Value::Array(file_rules));

    state.linter.config_mut().raw.extend(state.core.clone());
    state.linter.config_mut().reload_reflow();

    for case in file.cases {
        if verbose {
            println!("Processing case: {}", case.name);
        }
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

        let has_config = !case.configs.is_empty();
        let rule = &file.rule;
        if has_config {
            *state.linter.config_mut() = FluffConfig::new(case.configs.clone(), None, None);
            state.linter.config_mut().raw.extend(state.core.clone());

            if let Some(core) = case.configs.get("core").and_then(|it| it.as_map()) {
                state
                    .linter
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
                    state
                        .linter
                        .config_mut()
                        .raw
                        .get_mut("indentation")
                        .unwrap()
                        .as_map_mut()
                        .unwrap()
                        .insert(config.clone(), value.clone());
                }
            }

            state.linter.config_mut().reload_reflow();

            // Recreate linter with proper templater after all config is set up
            let templater = match Linter::get_templater(state.linter.config()) {
                Ok(t) => t,
                Err(e) => {
                    if std::env::var("SQRUFF_SKIP_UNSUPPORTED_TEMPLATERS").is_ok() {
                        println!("Skipping case '{}': {}", case.name, e);
                        *state.linter.config_mut() = FluffConfig::default();
                        state.linter.config_mut().raw.extend(state.core.clone());
                        state.linter.config_mut().reload_reflow();
                        continue;
                    } else {
                        panic!(
                            "Unsupported templater in case '{}': {}. \
                             Set SQRUFF_SKIP_UNSUPPORTED_TEMPLATERS=1 to skip these tests.",
                            case.name, e
                        );
                    }
                }
            };
            state.linter = Linter::new(
                state.linter.config().clone(),
                Some(templater),
                ParseErrors::Include,
            )
            .unwrap();
        }

        match case.kind {
            TestCaseKind::Pass { pass_str } => {
                let result = state
                    .linter
                    .lint_string_wrapped(&pass_str, Mode::Check)
                    .unwrap();
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
 None);

        let mut linter = Linter::new(config, None, ParseErrors::Include);

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
                let file = state
                    .linter
                    .lint_string_wrapped(&fail_str, Mode::Check)
                    .unwrap();
                assert_ne!(&file.violations(), &[])
            }
            TestCaseKind::Fix { fail_str, fix_str } => {
                assert_ne!(
                    &fail_str, &fix_str,
                    "Fail and fix strings should not be equal"
                );

                let linted = state
                    .linter
                    .lint_string_wrapped(&fail_str, Mode::Fix)
                    .unwrap();
                let actual = linted.fix_string();

                pretty_assertions::assert_eq!(actual, fix_str);
            }
        }

        if has_config {
            *state.linter.config_mut() = FluffConfig::default();
            state.linter.config_mut().raw.extend(state.core.clone());
            state.linter.config_mut().reload_reflow();

            // Recreate linter with default templater to avoid leaking
            // the custom templater (e.g. placeholder) into subsequent tests.
            let templater = Linter::get_templater(state.linter.config())
                .expect("Default config should have a valid templater");
            state.linter = Linter::new(
                state.linter.config().clone(),
                Some(templater),
                ParseErrors::Include,
            )
            .unwrap();
        }
    }
}
