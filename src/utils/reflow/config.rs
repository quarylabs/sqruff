use crate::core::config::FluffConfig;
use std::collections::HashMap;

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

pub struct ReflowConfig {}

impl ReflowConfig {
    pub fn from_fluff_config(config: FluffConfig) -> ReflowConfig {
        panic!("Not implemented yet");
    }
}
