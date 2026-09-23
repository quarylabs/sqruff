use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Serialize;
use sqruff_lib::Formatter;
use sqruff_lib::core::linter::linted_file::LintedFile;

const SARIF_SCHEMA: &str = "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json";
const RULES_URL: &str = "https://playground.quary.dev/docs/reference/rules/";

#[derive(Default)]
pub(crate) struct SarifFormatter {
    state: Mutex<SarifState>,
}

#[derive(Default)]
struct SarifState {
    rules: BTreeMap<String, SarifRule>,
    results: Vec<SarifResult>,
}

#[derive(Clone, Serialize)]
struct SarifText {
    text: String,
}

#[derive(Clone, Serialize)]
struct SarifReportingConfiguration {
    level: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule {
    id: String,
    name: String,
    short_description: SarifText,
    full_description: SarifText,
    default_configuration: SarifReportingConfiguration,
    help_uri: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifArtifactLocation {
    uri: String,
    uri_base_id: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    level: String,
    message: SarifText,
    locations: Vec<SarifLocation>,
}

impl SarifResult {
    fn sort_key(&self) -> (&str, usize, usize, &str, &str) {
        let location = &self.locations[0].physical_location;
        (
            &location.artifact_location.uri,
            location.region.start_line,
            location.region.start_column,
            &self.rule_id,
            &self.message.text,
        )
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    information_uri: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifInvocation {
    execution_successful: bool,
}

#[derive(Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
    invocations: Vec<SarifInvocation>,
}

#[derive(Serialize)]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

impl Formatter for SarifFormatter {
    fn dispatch_file_violations(&self, linted_file: &LintedFile) {
        let mut state = self.state.lock().unwrap();

        for violation in linted_file.violations() {
            let rule_id = violation.rule_code().to_string();
            let rule_name = violation.rule_name().to_string();
            let level = if violation.warning { "note" } else { "error" };

            state.rules.entry(rule_id.clone()).or_insert_with(|| {
                let anchor = rule_name.to_ascii_lowercase().replace('.', "");
                SarifRule {
                    id: rule_id.clone(),
                    name: rule_name.clone(),
                    short_description: SarifText {
                        text: rule_name.clone(),
                    },
                    full_description: SarifText {
                        text: violation.description.clone(),
                    },
                    default_configuration: SarifReportingConfiguration {
                        level: level.to_string(),
                    },
                    help_uri: format!("{RULES_URL}#{anchor}"),
                }
            });

            let start_line = violation.line_no.max(1);
            let start_column = violation.line_pos.max(1);
            let (mut end_line, mut end_column) =
                linted_file.source_position(violation.source_slice.end);
            if (end_line, end_column) < (start_line, start_column) {
                (end_line, end_column) = (start_line, start_column);
            }

            state.results.push(SarifResult {
                rule_id: rule_id.clone(),
                level: level.to_string(),
                message: SarifText {
                    text: format!("{rule_id}: {}", violation.description),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri: linted_file.path().to_string(),
                            uri_base_id: "%SRCROOT%",
                        },
                        region: SarifRegion {
                            start_line,
                            start_column,
                            end_line,
                            end_column,
                        },
                    },
                }],
            });
        }
    }

    fn dispatch_file_skip(&self, _fname: &str, _reason: &str) {}

    fn completion_message(&self, _count: usize) {
        let state = self.state.lock().unwrap();
        let rules = state.rules.values().cloned().collect();
        let mut results = state.results.clone();
        drop(state);
        results.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

        let log = SarifLog {
            schema: SARIF_SCHEMA,
            version: "2.1.0",
            runs: vec![SarifRun {
                tool: SarifTool {
                    driver: SarifDriver {
                        name: "sqruff",
                        version: env!("CARGO_PKG_VERSION"),
                        information_uri: "https://github.com/quarylabs/sqruff",
                        rules,
                    },
                },
                results,
                invocations: vec![SarifInvocation {
                    execution_successful: true,
                }],
            }],
        };

        println!("{}", serde_json::to_string_pretty(&log).unwrap());
    }
}
