#![cfg(feature = "parser")]

use assert_cmd::Command;
use serde_json::Value;

mod common;
use common::sqruff_path;

fn parse_output(format: &str, include_meta: bool) -> Value {
    let mut command = Command::new(sqruff_path());
    command.args(["parse", "--format", format, "--dialect", "ansi"]);
    if include_meta {
        command.arg("--include-meta");
    }
    let output = command
        .arg("-")
        .write_stdin("SELECT\n    col1\nFROM table1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    match format {
        "json" => serde_json::from_slice(&output).unwrap(),
        "yaml" => {
            let yaml: serde_yaml::Value = serde_yaml::from_slice(&output).unwrap();
            serde_json::to_value(yaml).unwrap()
        }
        _ => unreachable!(),
    }
}

fn find_object_with_key<'a>(
    value: &'a Value,
    key: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    match value {
        Value::Object(object) => {
            if object.contains_key(key) {
                return Some(object);
            }
            object
                .values()
                .find_map(|value| find_object_with_key(value, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|value| find_object_with_key(value, key)),
        _ => None,
    }
}

fn contains_key(value: &Value, key: &str) -> bool {
    find_object_with_key(value, key).is_some()
}

#[test]
fn parse_include_meta_adds_positions_to_json_and_yaml() {
    for format in ["json", "yaml"] {
        let parsed = parse_output(format, true);
        assert_eq!(parsed["start_line_no"], 1);
        assert_eq!(parsed["start_line_pos"], 1);
        assert_eq!(parsed["start_file_pos"], 0);
        assert!(parsed["end_line_no"].as_u64().unwrap() >= 3);
        assert!(parsed["end_file_pos"].as_u64().unwrap() > 0);

        let indent = find_object_with_key(&parsed, "indent")
            .expect("include-meta output should contain an indent segment");
        for field in [
            "start_line_no",
            "start_line_pos",
            "start_file_pos",
            "end_line_no",
            "end_line_pos",
            "end_file_pos",
        ] {
            assert!(indent.contains_key(field), "missing {field} in {format}");
        }
    }
}

#[test]
fn parse_without_include_meta_omits_positions_and_meta_segments() {
    for format in ["json", "yaml"] {
        let parsed = parse_output(format, false);
        assert!(!contains_key(&parsed, "start_line_no"));
        assert!(!contains_key(&parsed, "indent"));
        assert!(!contains_key(&parsed, "dedent"));
    }
}
