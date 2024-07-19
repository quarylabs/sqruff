use std::str::FromStr;

use ahash::{AHashMap, AHashSet};
use itertools::Itertools;

use crate::core::config::{FluffConfig, Value};
use crate::utils::reflow::depth_map::{DepthInfo, StackPositionType};
use crate::utils::reflow::reindent::{IndentUnit, TrailingComments};

type ConfigElementType = AHashMap<String, String>;
type ConfigDictType = AHashMap<String, ConfigElementType>;

/// Holds spacing config for a block and allows easy manipulation
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BlockConfig {
    pub spacing_before: &'static str,
    pub spacing_after: &'static str,
    pub spacing_within: Option<&'static str>,
    pub line_position: Option<&'static str>,
}

impl Default for BlockConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockConfig {
    pub fn new() -> Self {
        BlockConfig {
            spacing_before: "single",
            spacing_after: "single",
            spacing_within: None,
            line_position: None,
        }
    }

    fn convert_spacing(spacing: &str) -> &'static str {
        match spacing {
            "single" => "single",
            "touch" => "touch",
            "touch:inline" => "touch:inline",
            "any" => "any",
            _ => unreachable!(
                "Expected 'single', 'touch', 'touch:inline', 'any', found '{}'",
                spacing
            ),
        }
    }

    fn convert_line_position(line_position: &str) -> &'static str {
        match line_position {
            "alone" => "alone",
            "leading" => "leading",
            "trailing" => "trailing",
            "alone:strict" => "alone:strict",
            _ => unreachable!("Expected 'alone', 'leading' found '{}'", line_position),
        }
    }

    fn convert_space_within(space_within: &str) -> &'static str {
        match space_within {
            "alone" => "alone",
            "touch:inline" => "touch:inline",
            "touch" => "touch",
            "single:inline" => "single:inline",
            _ => unreachable!("Expected 'alone', 'touch:inline', 'touch' found '{}'", space_within),
        }
    }

    /// Mutate the config based on additional information
    pub fn incorporate(
        &mut self,
        before: Option<&'static str>,
        after: Option<&'static str>,
        within: Option<&'static str>,
        line_position: Option<&'static str>,
        config: Option<&ConfigElementType>,
    ) {
        let empty = AHashMap::new();
        let config = config.unwrap_or(&empty);

        self.spacing_before = before
            .or_else(|| {
                let before = config.get("spacing_before");
                match before {
                    Some(value) => Some(Self::convert_spacing(value)),
                    None => None,
                }
            })
            .unwrap_or(self.spacing_before);

        self.spacing_after = after
            .or_else(|| {
                let after = config.get("spacing_after");
                match after {
                    Some(value) => Some(Self::convert_spacing(value)),
                    None => None,
                }
            })
            .unwrap_or(self.spacing_after);

        self.spacing_within = within.or_else(|| {
            let space_within = config.get("spacing_within");
            match space_within {
                Some(value) => Some(Self::convert_space_within(value)),
                None => None,
            }
        });

        self.line_position = line_position.or_else(|| {
            let line_position = config.get("line_position");
            match line_position {
                Some(value) => Some(Self::convert_line_position(value)),
                None => None,
            }
        });
    }
}

/// An interface onto the configuration of how segments should reflow.
///
/// This acts as the primary translation engine between configuration
/// held either in dicts for testing, or in the FluffConfig in live
/// usage, and the configuration used during reflow operations.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct ReflowConfig {
    configs: ConfigDictType,
    config_types: AHashSet<String>,
    /// In production, these values are almost _always_ set because we
    /// use `.from_fluff_config`, but the defaults are here to aid in
    /// testing.
    pub(crate) indent_unit: IndentUnit,
    pub(crate) max_line_length: usize,
    pub(crate) hanging_indents: bool,
    pub(crate) allow_implicit_indents: bool,
    pub(crate) trailing_comments: TrailingComments,
}

impl ReflowConfig {
    pub fn get_block_config(
        &self,
        block_class_types: &AHashSet<&str>,
        depth_info: Option<&DepthInfo>,
    ) -> BlockConfig {
        let configured_types =
            block_class_types.iter().filter(|&&typ| self.config_types.contains(typ));

        let mut block_config = BlockConfig::new();

        if let Some(depth_info) = depth_info {
            let (mut parent_start, mut parent_end) = (true, true);

            for (idx, key) in depth_info.stack_hashes.iter().rev().enumerate() {
                let stack_position = &depth_info.stack_positions[key];

                if !matches!(
                    stack_position.type_,
                    Some(StackPositionType::Solo) | Some(StackPositionType::Start)
                ) {
                    parent_start = false;
                }

                if !matches!(
                    stack_position.type_,
                    Some(StackPositionType::Solo) | Some(StackPositionType::End)
                ) {
                    parent_end = false;
                }

                if !parent_start && !parent_end {
                    break;
                }

                let parent_classes =
                    &depth_info.stack_class_types[depth_info.stack_class_types.len() - 1 - idx];
                let parent_classes: AHashSet<String> =
                    parent_classes.iter().map(|s| s.to_string()).collect();

                let configured_parent_types =
                    self.config_types.intersection(&parent_classes).collect_vec();

                if parent_start {
                    for seg_type in &configured_parent_types {
                        let before = self
                            .configs
                            .get(seg_type.as_str())
                            .and_then(|conf| conf.get("spacing_before"))
                            .map(|it| it.as_str());
                        let before = match before {
                            Some("single") => Some("single"),
                            Some("touch") => Some("touch"),
                            Some("touch:inline") => Some("touch:inline"),
                            None => None,
                            Some(value) => {
                                unreachable!("Expected 'single' or 'touch', found '{}'", value)
                            }
                        };

                        block_config.incorporate(before, None, None, None, None);
                    }
                }

                if parent_end {
                    for seg_type in &configured_parent_types {
                        let after = self
                            .configs
                            .get(seg_type.as_str())
                            .and_then(|conf| conf.get("spacing_after"))
                            .map(|it| it.as_str());
                        let after = match after {
                            Some("single") => Some("single"),
                            Some("touch") => Some("touch"),
                            Some("touch:inline") => Some("touch:inline"),
                            None => None,
                            Some(value) => {
                                unreachable!("Expected 'single' or 'touch', found '{}'", value)
                            }
                        };

                        block_config.incorporate(None, after, None, None, None);
                    }
                }
            }
        }

        for &seg_type in configured_types {
            block_config.incorporate(None, None, None, None, self.configs.get(seg_type));
        }

        block_config
    }

    pub fn from_fluff_config(config: &FluffConfig) -> ReflowConfig {
        let configs = config.raw["layout"]["type"].as_map().unwrap().clone();
        let config_types = configs.keys().map(|x| x.to_string()).collect::<AHashSet<String>>();

        let trailing_comments = config.raw["indentation"]["trailing_comments"].as_string().unwrap();
        let trailing_comments = TrailingComments::from_str(trailing_comments).unwrap();

        let tab_space_size = config.raw["indentation"]["tab_space_size"].as_int().unwrap() as usize;
        let indent_unit = config.raw["indentation"]["indent_unit"].as_string().unwrap();
        let indent_unit = IndentUnit::from_type_and_size(indent_unit, tab_space_size);

        ReflowConfig {
            configs: convert_to_config_dict(configs),
            config_types,
            indent_unit,
            max_line_length: config.raw["core"]["max_line_length"].as_int().unwrap() as usize,
            hanging_indents: config.raw["indentation"]["hanging_indents"]
                .as_bool()
                .unwrap_or_default(),
            allow_implicit_indents: config.raw["indentation"]["allow_implicit_indents"]
                .as_bool()
                .unwrap(),
            trailing_comments,
        }
    }
}

fn convert_to_config_dict(input: AHashMap<String, Value>) -> ConfigDictType {
    let mut config_dict = ConfigDictType::new();

    for (key, value) in input {
        match value {
            Value::Map(map_value) => {
                let element = map_value
                    .into_iter()
                    .map(|(inner_key, inner_value)| {
                        if let Value::String(value_str) = inner_value {
                            (inner_key, value_str.into())
                        } else {
                            panic!("Expected a Value::String, found another variant.");
                        }
                    })
                    .collect::<ConfigElementType>();
                config_dict.insert(key, element);
            }
            _ => panic!("Expected a Value::Map, found another variant."),
        }
    }

    config_dict
}
