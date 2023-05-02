use crate::core::config::FluffConfig;
use crate::utils::reflow::depth_map::DepthInfo;
use std::collections::{HashMap, HashSet};
type ConfigElementType = HashMap<String, String>;
type ConfigDictType = HashMap<String, ConfigElementType>;

/// Holds spacing config for a block and allows easy manipulation
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BlockConfig {
    spacing_before: String,
    spacing_after: String,
    spacing_within: Option<String>,
    line_position: Option<String>,
}

impl BlockConfig {
    pub fn new() -> Self {
        BlockConfig {
            spacing_before: "single".to_string(),
            spacing_after: "single".to_string(),
            spacing_within: None,
            line_position: None,
        }
    }

    /// Mutate the config based on additional information
    pub fn incorporate(
        &mut self,
        before: Option<&str>,
        after: Option<&str>,
        within: Option<&str>,
        line_position: Option<&str>,
        config: Option<&ConfigElementType>,
    ) {
        let empty_config: ConfigElementType = HashMap::new();
        let config = config.unwrap_or(&empty_config);
        self.spacing_before = before
            .or(config.get("spacing_before").map(|s| s.as_str()))
            .unwrap_or(&self.spacing_before)
            .to_string();
        self.spacing_after = after
            .or(config.get("spacing_after").map(|s| s.as_str()))
            .unwrap_or(&self.spacing_after)
            .to_string();
        self.spacing_within = within
            .or(config.get("spacing_within").map(|s| s.as_str()))
            .map(|s| s.to_string());
        self.line_position = line_position
            .or(config.get("line_position").map(|s| s.as_str()))
            .map(|s| s.to_string());
    }
}

/// An interface onto the configuration of how segments should reflow.
///
/// This acts as the primary translation engine between configuration
/// held either in dicts for testing, or in the FluffConfig in live
/// usage, and the configuration used during reflow operations.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ReflowConfig {
    _config_dict: ConfigDictType,
    config_types: HashSet<String>,
    /// In production, these values are almost _always_ set because we
    /// use `.from_fluff_config`, but the defaults are here to aid in
    /// testing.
    tab_space_size: usize,
    indent_unit: String,
    max_line_length: usize,
    hanging_indents: bool,
    skip_indentation_in: HashSet<String>,
    allow_implicit_indents: bool,
    trailing_comments: String,
}

impl Default for ReflowConfig {
    fn default() -> Self {
        ReflowConfig {
            _config_dict: HashMap::new(),
            config_types: HashSet::new(),
            tab_space_size: 4,
            indent_unit: "    ".to_string(),
            max_line_length: 80,
            hanging_indents: false,
            skip_indentation_in: HashSet::new(),
            allow_implicit_indents: false,
            trailing_comments: "before".to_string(),
        }
    }
}

impl ReflowConfig {
    pub fn get_block_config(
        &self,
        block_class_types: Vec<String>,
        depth_info: Option<DepthInfo>,
    ) -> BlockConfig {
        panic!("Not implemented yet");
    }

    pub fn from_fluff_config(config: FluffConfig) -> ReflowConfig {
        panic!("Not implemented yet");
    }
}
