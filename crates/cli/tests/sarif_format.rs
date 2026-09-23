use std::collections::HashSet;
use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

mod common;
use common::{manifest_dir, sqruff_path};

#[test]
fn sarif_output_contains_results_from_multiple_files() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("first.sql"), "SELECT 1 ,4").unwrap();
    fs::write(temp.path().join("second.sql"), "SELECT 2 ,5").unwrap();

    let mut command = Command::new(sqruff_path());
    command
        .current_dir(temp.path())
        .env("HOME", manifest_dir())
        .args([
            "lint",
            "--dialect",
            "ansi",
            "--format",
            "sarif",
            "first.sql",
            "second.sql",
        ]);

    let assert = command.assert().code(1);
    let output: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();

    assert_eq!(output["version"], "2.1.0");
    assert!(
        output["$schema"]
            .as_str()
            .unwrap()
            .ends_with("sarif-schema-2.1.0.json")
    );

    let run = &output["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "sqruff");
    assert_eq!(run["tool"]["driver"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(run["invocations"][0]["executionSuccessful"], true);

    let rules = run["tool"]["driver"]["rules"].as_array().unwrap();
    let rule_ids: HashSet<_> = rules
        .iter()
        .map(|rule| rule["id"].as_str().unwrap())
        .collect();
    assert_eq!(rule_ids.len(), rules.len());

    let results = run["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let paths: HashSet<_> = results
        .iter()
        .map(|result| {
            result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
        })
        .collect();
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|path| path.ends_with("first.sql")));
    assert!(paths.iter().any(|path| path.ends_with("second.sql")));

    for result in results {
        assert!(rule_ids.contains(result["ruleId"].as_str().unwrap()));
        let region = &result["locations"][0]["physicalLocation"]["region"];
        assert!(region["startLine"].as_u64().unwrap() >= 1);
        assert!(region["startColumn"].as_u64().unwrap() >= 1);
        assert!(region["endLine"].as_u64().unwrap() >= 1);
        assert!(region["endColumn"].as_u64().unwrap() >= 1);
    }
}
