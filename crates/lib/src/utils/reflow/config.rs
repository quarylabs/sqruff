use std::str::FromStr;

use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::{FluffConfig, Value};
use crate::utils::reflow::depth_map::{DepthInfo, StackPositionType};
use crate::utils::reflow::rebreak::LinePosition;
use crate::utils::reflow::reindent::{IndentUnit, TrailingComments};

type ConfigDictType = HashMap<SyntaxKind, LayoutTypeConfig>;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
struct LayoutTypeConfig {
    spacing_before: Option<Spacing>,
    spacing_after: Option<Spacing>,
    spacing_within: Option<Spacing>,
    line_position: Option<LinePositionConfig>,
    keyword_line_position: Option<String>,
    keyword_line_position_exclusions: SyntaxSet,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct LinePositionConfig {
    position: LinePosition,
    strict: bool,
}

impl LinePositionConfig {
    pub const fn new(position: LinePosition, strict: bool) -> Self {
        Self { position, strict }
    }

    pub const fn position(self) -> LinePosition {
        self.position
    }

    pub const fn is_strict(self) -> bool {
        self.strict
    }
}

impl FromStr for LinePositionConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let position = parts
            .next()
            .ok_or_else(|| "line_position cannot be empty".to_string())?
            .parse::<LinePosition>()
            .map_err(|_| format!("Unexpected line_position value: {s}"))?;
        let strict = match parts.next() {
            Some("strict") => true,
            Some(other) => {
                return Err(format!(
                    "Unexpected line_position modifier '{other}' in '{s}'"
                ));
            }
            None => false,
        };

        if parts.next().is_some() {
            return Err(format!("Unexpected line_position value: {s}"));
        }

        Ok(Self::new(position, strict))
    }
}

/// Holds spacing config for a block and allows easy manipulation
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BlockConfig {
    pub spacing_before: Spacing,
    pub spacing_after: Spacing,
    pub spacing_within: Option<Spacing>,
    pub line_position: Option<LinePositionConfig>,
    pub keyword_line_position: Option<String>,
    pub keyword_line_position_exclusions: SyntaxSet,
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
            keyword_line_position: None,
            keyword_line_position_exclusions: SyntaxSet::EMPTY,
        }
    }

    /// Mutate the config based on additional information.
    fn incorporate(
        &mut self,
        before: Option<Spacing>,
        after: Option<Spacing>,
        within: Option<Spacing>,
        line_position: Option<LinePositionConfig>,
        config: Option<&LayoutTypeConfig>,
    ) {
        self.spacing_before = before
            .or_else(|| config.and_then(|c| c.spacing_before))
            .unwrap_or(self.spacing_before);

        self.spacing_after = after
            .or_else(|| config.and_then(|c| c.spacing_after))
            .unwrap_or(self.spacing_after);

        self.spacing_within = within.or_else(|| config.and_then(|c| c.spacing_within));
        self.line_position = line_position.or_else(|| config.and_then(|c| c.line_position));

        if let Some(keyword_line_position) = config.and_then(|c| c.keyword_line_position.clone()) {
            self.keyword_line_position = Some(keyword_line_position);
        }

        if let Some(keyword_line_position_exclusions) =
            config.map(|c| c.keyword_line_position_exclusions.clone())
        {
            self.keyword_line_position_exclusions = keyword_line_position_exclusions;
        }
    }
}

fn parse_configured_syntax_set(raw: &str) -> SyntaxSet {
    raw.split(',')
        .filter_map(|seg_type| {
            let seg_type = seg_type.trim();
            if seg_type.is_empty() || seg_type.eq_ignore_ascii_case("none") {
                None
            } else {
                parse_syntax_kind_alias(seg_type)
            }
        })
        .collect()
}

fn parse_syntax_kind_alias(seg_type: &str) -> Option<SyntaxKind> {
    match seg_type {
        "aggregate_order_by" => Some(SyntaxKind::AggregateOrderByClause),
        _ => seg_type.parse().ok(),
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
    pub fn line_position_for(&self, seg_type: SyntaxKind) -> Option<LinePositionConfig> {
        self.configs
            .get(&seg_type)
            .and_then(|cfg| cfg.line_position)
    }

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
                            .and_then(|conf| conf.spacing_before);
                        block_config.incorporate(before, None, None, None, None);
                    }
                }

                if parent_end {
                    for seg_type in configured_parent_types {
                        let after = self
                            .configs
                            .get(&seg_type)
                            .and_then(|conf| conf.spacing_after);
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

fn convert_to_config_dict(input: HashMap<String, Value>) -> ConfigDictType {
    let mut config_dict = ConfigDictType::new();

    for (key, value) in input {
        match value {
            Value::Map(map_value) => {
                let seg_type = key.parse().unwrap_or_else(|_| unimplemented!("{key}"));
                config_dict.insert(
                    seg_type,
                    LayoutTypeConfig::from_value_map(seg_type, map_value),
                );
            }
            _ => panic!("Expected a Value::Map, found another variant."),
        }
    }

    config_dict
}

impl LayoutTypeConfig {
    fn from_value_map(seg_type: SyntaxKind, map_value: HashMap<String, Value>) -> Self {
        Self {
            spacing_before: spacing_from_map(seg_type, &map_value, "spacing_before"),
            spacing_after: spacing_from_map(seg_type, &map_value, "spacing_after"),
            spacing_within: spacing_from_map(seg_type, &map_value, "spacing_within"),
            line_position: map_value
                .get("line_position")
                .map(string_value)
                .transpose()
                .unwrap()
                .map(|it| it.parse().unwrap()),
            keyword_line_position: map_value
                .get("keyword_line_position")
                .map(string_value)
                .transpose()
                .unwrap()
                .map(ToOwned::to_owned),
            keyword_line_position_exclusions: map_value
                .get("keyword_line_position_exclusions")
                .map(string_value)
                .transpose()
                .unwrap()
                .map(parse_configured_syntax_set)
                .unwrap_or(SyntaxSet::EMPTY),
        }
    }
}

fn spacing_from_map(
    seg_type: SyntaxKind,
    map_value: &HashMap<String, Value>,
    key: &str,
) -> Option<Spacing> {
    let spacing = map_value.get(key).map(string_value).transpose().unwrap()?;
    if spacing == "align" {
        Some(Spacing::Align {
            seg_type,
            within: map_value
                .get("align_within")
                .map(string_value)
                .transpose()
                .unwrap()
                .map(|it| it.parse().unwrap()),
            scope: map_value
                .get("align_scope")
                .map(string_value)
                .transpose()
                .unwrap()
                .map(|it| it.parse().unwrap()),
        })
    } else {
        Some(spacing.parse().unwrap())
    }
}

fn string_value(value: &Value) -> Result<&str, String> {
    value
        .as_string()
        .ok_or_else(|| "Expected a Value::String, found another variant.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_line_position_config() {
        let config: LinePositionConfig = "alone:strict".parse().unwrap();

        assert_eq!(config.position(), LinePosition::Alone);
        assert!(config.is_strict());
    }

    #[test]
    fn parses_align_spacing_from_layout_config() {
        let mut layout = HashMap::new();
        layout.insert("spacing_before".into(), Value::String("align".into()));
        layout.insert("align_within".into(), Value::String("select_clause".into()));
        layout.insert("align_scope".into(), Value::String("statement".into()));

        let config = LayoutTypeConfig::from_value_map(SyntaxKind::AliasExpression, layout);

        assert_eq!(
            config.spacing_before,
            Some(Spacing::Align {
                seg_type: SyntaxKind::AliasExpression,
                within: Some(SyntaxKind::SelectClause),
                scope: Some(SyntaxKind::Statement),
            })
        );
    }
}
