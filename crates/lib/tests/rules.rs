use std::str::FromStr;

use ahash::AHashMap;
use glob::glob;
use serde::Deserialize;
use serde_with::{serde_as, KeyValueMap};
use sqruff_lib::core::config::{FluffConfig, Value};
use sqruff_lib::core::dialects::init::DialectKind;
use sqruff_lib::core::linter::linter::Linter;

#[derive(Default)]
pub struct Args {
    list: bool,
    ignored: bool,
    no_capture: bool,
}

impl Args {
    fn parse_args(&mut self, mut iter: impl Iterator<Item = String>) {
        while let Some(arg) = iter.next() {
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

    let mut linter = Linter::new(FluffConfig::default(), None, None);
    let mut core = AHashMap::new();
    core.insert("core".to_string(), Value::Map(<_>::default()));

    for path in glob("test/fixtures/rules/std_rule_cases/*.yml").unwrap() {
        let path = path.unwrap();
        let input = std::fs::read_to_string(path).unwrap();

        let file: TestFile = serde_yaml::from_str(&input).unwrap();
        core.get_mut("core").unwrap().as_map_mut().unwrap().insert(
            "rule_allowlist".into(),
            Value::Array(vec![Value::String(file.rule.clone().into())]),
        );

        linter.config_mut().raw.extend(core.clone());

        for case in file.cases {
            let dialect_name = case
                .configs
                .get("core")
                .and_then(|it| it.as_map())
                .and_then(|it| it.get("dialect"))
                .and_then(|it| it.as_string())
                .unwrap_or("ansi");
            let dialect_name_to_str = dialect_name.to_string();

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
            if !args.no_capture {
                println!();
            }

            let has_config = !case.configs.is_empty();

            if has_config {
                *linter.config_mut() = FluffConfig::new(case.configs, None, None);
                linter.config_mut().raw.extend(core.clone());
            }

            let rule_pack = linter.get_rulepack().rules();

            match case.kind {
                TestCaseKind::Pass { pass_str } => {
                    let f = linter.lint_string_wrapped(&pass_str, None, None, rule_pack.clone());
                    assert_eq!(
                        &f.paths[0].files[0].violations,
                        &[],
                        "query: {pass_str}
prepared test:

#[cfg(test)]
mod tests {{
    use pretty_assertions::assert_eq;

    use crate::api::simple::{{lint}};
    use crate::core::rules::base::{{Erased, ErasedRule}};
    use super::*;

    // Note some of the config may need pulling
    fn rules() -> Vec<ErasedRule> {{
        vec![Rule{}.erased()]
    }}

    #[test]
    fn {}() {{
        let sql = \"{}\";

        let violations = lint(sql.into(), \"{}\".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }}
}}",
                        rule_pack.first().unwrap().code(),
                        case.name,
                        pass_str,
                        dialect_name_to_str
                    );
                }
                TestCaseKind::Fail { fail_str } => {
                    let f = linter.lint_string_wrapped(&fail_str, None, None, rule_pack);
                    assert_ne!(&f.paths[0].files[0].violations, &[]);
                }
                TestCaseKind::Fix { fail_str, fix_str } => {
                    let f =
                        linter.lint_string_wrapped(&fail_str, None, Some(true), rule_pack).paths[0]
                            .files[0]
                            .fix_string();
                    assert_eq!(f, fix_str);
                }
            }

            if has_config {
                *linter.config_mut() = FluffConfig::default();
                linter.config_mut().raw.extend(core.clone());
            }
        }
    }
}
