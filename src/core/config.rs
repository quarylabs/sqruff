use crate::core::errors::SQLFluffUserError;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct RemovedConfig<'a> {
    old_path: Vec<&'static str>,
    warning: &'a str,
    new_path: Option<Vec<&'a str>>,
    translation_func: Option<fn(&'a str) -> &'a str>,
}

pub fn REMOVED_CONFIGS() -> [RemovedConfig<'static>; 12] {
    [
        RemovedConfig {
            old_path: vec!["rules", "max_line_length"],
            warning: "The max_line_length config has moved from sqlfluff:rules to the root sqlfluff level.",
            new_path: Some(vec!["max_line_length"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L003", "hanging_indents"],
            warning: "Hanging indents are no longer supported in SQLFluff from version 2.0.0 onwards. See https://docs.sqlfluff.com/en/stable/layout.html#hanging-indents",
            new_path: None,
            translation_func: None,
        },
        RemovedConfig {
            old_path: vec!["rules", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L002", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L003", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L004", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L016", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "indent_unit"],
            warning: "The indent_unit config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "indent_unit"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L007", "operator_new_lines"],
            warning: "Use the line_position config in the appropriate sqlfluff:layout section (e.g. sqlfluff:layout:type:binary_operator).",
            new_path: Some(vec!["layout", "type", "binary_operator", "line_position"]),
            translation_func: Some(|x| if x == "before" { "trailing" } else { "leading" }),
        },
        RemovedConfig {
            old_path: vec!["rules", "comma_style"],
            warning: "Use the line_position config in the appropriate sqlfluff:layout section (e.g. sqlfluff:layout:type:comma).",
            new_path: Some(vec!["layout", "type", "comma", "line_position"]),
            translation_func: Some(|x| x),
        },
        // L019 used to have a more specific version of the same /config itself.
        RemovedConfig {
            old_path: vec!["rules", "L019", "comma_style"],
            warning: "Use the line_position config in the appropriate sqlfluff:layout section (e.g. sqlfluff:layout:type:comma).",
            new_path: Some(vec!["layout", "type", "comma", "line_position"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L003", "lint_templated_tokens"],
            warning: "No longer used.",
            new_path: None,
            translation_func: None,
        },
    ]
}

pub fn split_comma_separated_string(raw_str: &str) -> Vec<String> {
    let mut res: Vec<String> = Vec::new();
    for s in raw_str.split(",") {
        if s.trim() != "" {
            res.push(s.trim().to_string());
        }
    }
    return res;
}

/// The class that actually gets passed around as a config object.
// TODO This is not a translation that is particularly accurate.
#[derive(Clone, Debug)]
pub struct FluffConfig {
    configs: Option<HashSet<String>>,
    extra_config_path: Option<String>,
}

impl FluffConfig {
    // TODO This is not a translation that is particularly accurate.
    pub fn new(configs: Option<HashSet<String>>, extra_config_path: Option<String>) -> Self {
        Self {
            configs,
            extra_config_path,
        }
    }

    /// Loads a config object just based on the root directory.
    // TODO This is not a translation that is particularly accurate.
    pub fn from_root(
        extra_config_path: Option<String>,
        ignore_local_config: bool,
        overrides: Option<HashMap<String, String>>,
    ) -> Result<FluffConfig, SQLFluffUserError> {
        Ok(FluffConfig::new(Some(HashSet::new()), extra_config_path))
    }

    /// Process a full raw file for inline config and update self.
    pub fn process_raw_file_for_config(&mut self, raw_str: &String) {
        // Scan the raw file for config commands
        for raw_line in raw_str.clone().lines() {
            if raw_line.to_string().starts_with("-- sqlfluff") {
                // Found a in-file config command
                self.process_inline_config(raw_line)
            }
        }
    }

    /// Process an inline config command and update self.
    pub fn process_inline_config(&mut self, config_line: &str) {
        panic!("Not implemented")
    }
}
