use std::str::FromStr;

use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::{FluffConfig, Value};
use crate::utils::reflow::depth_map::{DepthInfo, StackPositionType};
use crate::utils::reflow::reindent::{IndentUnit, TrailingComments};

type ConfigElementType = AHashMap<String, String>;
type ConfigDictType = AHashMap<SyntaxKind, ConfigElementType>;

/// Holds spacing config for a block and allows easy manipulation
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BlockConfig {
    pub spacing_before: Spacing,
    pub spacing_after: Spacing,
    pub spacing_within: Option<Spacing>,
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
            spacing_before: Spacing::Single,
            spacing_after: Spacing::Single,
            spacing_within: None,
            line_position: None,
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

    /// Mutate the config based on additional information
    pub fn incorporate(
        &mut self,
        before: Option<Spacing>,
        after: Option<Spacing>,
        within: Option<Spacing>,
        line_position: Option<&'static str>,
        config: Option<&ConfigElementType>,
    ) {
        let empty = AHashMap::new();
        let config = config.unwrap_or(&empty);

        self.spacing_before = before
            .or_else(|| config.get("spacing_before").map(|it| it.parse().unwrap()))
            .unwrap_or(self.spacing_before);

        self.spacing_after = after
            .or_else(|| config.get("spacing_after").map(|it| it.parse().unwrap()))
            .unwrap_or(self.spacing_after);

        self.spacing_within =
            within.or_else(|| config.get("spacing_within").map(|it| it.parse().unwrap()));

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
    config_types: SyntaxSet,
    /// In production, these values are almost _always_ set because we
    /// use `.from_fluff_config`, but the defaults are here to aid in
    /// testing.
    pub(crate) indent_unit: IndentUnit,
    pub(crate) max_line_length: usize,
    pub(crate) hanging_indents: bool,
    pub(crate) allow_implicit_indents: bool,
    pub(crate) trailing_comments: TrailingComments,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Spacing {
    Single,
    Touch,
    TouchInline,
    SingleInline,
    Any,
    Align {
        seg_type: SyntaxKind,
        within: Option<SyntaxKind>,
        scope: Option<SyntaxKind>,
    },
}

impl FromStr for Spacing {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "single" => Self::Single,
            "touch" => Self::Touch,
            "touch:inline" => Self::TouchInline,
            "single:inline" => Self::SingleInline,
            "any" => Self::Any,
            s => {
                if let Some(rest) = s.strip_prefix("align") {
                    let mut args = rest.split(':');
                    args.next();

                    let seg_type = args.next().map(|it| it.parse().unwrap()).unwrap();
                    let within = args.next().map(|it| it.parse().unwrap());
                    let scope = args.next().map(|it| it.parse().unwrap());

                    Spacing::Align {
                        seg_type,
                        within,
                        scope,
                    }
                } else {
                    unimplemented!("{s}")
                }
            }
        })
    }
}

impl ReflowConfig {
    pub fn get_block_config(
        &self,
        block_class_types: &SyntaxSet,
        depth_info: Option<&DepthInfo>,
    ) -> BlockConfig {
        let configured_types = block_class_types.clone().intersection(&self.config_types);

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

                let configured_parent_types =
                    self.config_types.clone().intersection(parent_classes);

                if parent_start {
                    for seg_type in configured_parent_types.clone() {
                        let before = self
                            .configs
                            .get(&seg_type)
                            .and_then(|conf| conf.get("spacing_before"))
                            .map(|it| it.as_str());
                        let before = before.map(|it| it.parse().unwrap());

                        block_config.incorporate(before, None, None, None, None);
                    }
                }

                if parent_end {
                    for seg_type in configured_parent_types {
                        let after = self
                            .configs
                            .get(&seg_type)
                            .and_then(|conf| conf.get("spacing_after"))
                            .map(|it| it.as_str());

                        let after = after.map(|it| it.parse().unwrap());
                        block_config.incorporate(None, after, None, None, None);
                    }
                }
            }
        }

        for seg_type in configured_types {
            block_config.incorporate(None, None, None, None, self.configs.get(&seg_type));
        }

        block_config
    }

    pub fn from_fluff_config(config: &FluffConfig) -> ReflowConfig {
        let configs = config.raw["layout"]["type"].as_map().unwrap().clone();
        let config_types = configs
            .keys()
            .map(|x| x.parse().unwrap_or_else(|_| unimplemented!("{x}")))
            .collect::<SyntaxSet>();

        let trailing_comments = config.raw["indentation"]["trailing_comments"]
            .as_string()
            .unwrap();
        let trailing_comments = TrailingComments::from_str(trailing_comments).unwrap();

        let tab_space_size = config.raw["indentation"]["tab_space_size"]
            .as_int()
            .unwrap() as usize;
        let indent_unit = config.raw["indentation"]["indent_unit"]
            .as_string()
            .unwrap();
        let indent_unit = IndentUnit::from_type_and_size(indent_unit, tab_space_size);

        let mut configs = convert_to_config_dict(configs);
        let keys: Vec<_> = configs.keys().copied().collect();

        for seg_type in keys {
            for key in ["spacing_before", "spacing_after"] {
                if configs[&seg_type].get(key).map(String::as_str) == Some("align") {
                    let mut new_key = format!("align:{}", seg_type.as_str());
                    if let Some(align_within) = configs[&seg_type].get("align_within") {
                        new_key.push_str(&format!(":{align_within}"));

                        if let Some(align_scope) = configs[&seg_type].get("align_scope") {
                            new_key.push_str(&format!(":{align_scope}"));
                        }
                    }

                    *configs.get_mut(&seg_type).unwrap().get_mut(key).unwrap() = new_key;
                }
            }
        }

        ReflowConfig {
            configs,
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
                config_dict.insert(
                    key.parse().unwrap_or_else(|_| unimplemented!("{key}")),
                    element,
                );
            }
            _ => panic!("Expected a Value::Map, found another variant."),
        }
    }

    config_dict
}
