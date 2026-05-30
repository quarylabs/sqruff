use std::str::FromStr;

use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::config::{
    ConfigError, CoreConfig, FluffConfig, IndentationConfig, LayoutConfig, LayoutTypeConfig,
};
use crate::utils::reflow::depth_map::{DepthInfo, StackPositionType};
use crate::utils::reflow::rebreak::LinePosition;
use crate::utils::reflow::reindent::{IndentUnit, TrailingComments};

type ConfigDictType = hashbrown::HashMap<SyntaxKind, LayoutTypeConfig>;

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

    pub(crate) fn to_config_string(self) -> String {
        let position: &str = self.position.as_ref();
        if self.strict {
            format!("{position}:strict")
        } else {
            position.to_string()
        }
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
    pub keyword_line_position: Option<LinePosition>,
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

        if let Some(keyword_line_position) = config.and_then(|c| c.keyword_line_position) {
            self.keyword_line_position = Some(keyword_line_position);
        }

        if let Some(keyword_line_position_exclusions) =
            config.map(|c| c.keyword_line_position_exclusions.clone())
        {
            self.keyword_line_position_exclusions = keyword_line_position_exclusions;
        }
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
pub enum SpacingSpec {
    Single,
    Touch,
    TouchInline,
    SingleInline,
    Any,
    Align,
}

impl SpacingSpec {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Touch => "touch",
            Self::TouchInline => "touch:inline",
            Self::SingleInline => "single:inline",
            Self::Any => "any",
            Self::Align => "align",
        }
    }

    pub(crate) fn resolve(
        self,
        seg_type: SyntaxKind,
        align_within: Option<SyntaxKind>,
        align_scope: Option<SyntaxKind>,
    ) -> Result<Spacing, ConfigError> {
        Ok(match self {
            Self::Single => Spacing::Single,
            Self::Touch => Spacing::Touch,
            Self::TouchInline => Spacing::TouchInline,
            Self::SingleInline => Spacing::SingleInline,
            Self::Any => Spacing::Any,
            Self::Align => Spacing::Align {
                seg_type,
                within: align_within,
                scope: align_scope,
            },
        })
    }
}

impl FromStr for SpacingSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "single" => Ok(Self::Single),
            "touch" => Ok(Self::Touch),
            "touch:inline" => Ok(Self::TouchInline),
            "single:inline" => Ok(Self::SingleInline),
            "any" => Ok(Self::Any),
            "align" => Ok(Self::Align),
            _ => Err(format!("invalid spacing spec '{s}'")),
        }
    }
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
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "single" => Self::Single,
            "touch" => Self::Touch,
            "touch:inline" => Self::TouchInline,
            "single:inline" => Self::SingleInline,
            "any" => Self::Any,
            s => {
                if let Some(rest) = s.strip_prefix("align:") {
                    let mut args = rest.split(':');
                    let seg_type = args
                        .next()
                        .ok_or_else(|| format!("missing align segment type in '{s}'"))?
                        .parse()
                        .map_err(|_| format!("invalid layout syntax kind in '{s}'"))?;
                    let within = args
                        .next()
                        .map(str::parse)
                        .transpose()
                        .map_err(|_| format!("invalid align_within syntax kind in '{s}'"))?;
                    let scope = args
                        .next()
                        .map(str::parse)
                        .transpose()
                        .map_err(|_| format!("invalid align_scope syntax kind in '{s}'"))?;

                    if args.next().is_some() {
                        return Err(format!("too many align arguments in '{s}'"));
                    }

                    Spacing::Align {
                        seg_type,
                        within,
                        scope,
                    }
                } else {
                    return Err(format!("invalid spacing spec '{s}'"));
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

    pub fn from_fluff_config(config: &FluffConfig) -> Result<ReflowConfig, ConfigError> {
        Self::from_config_parts(config.layout(), config.indentation(), config.core())
    }

    pub fn from_config_parts(
        layout: &LayoutConfig,
        indentation: &IndentationConfig,
        core: &CoreConfig,
    ) -> Result<Self, ConfigError> {
        let configs = layout.types.clone();
        let config_types = configs.keys().copied().collect::<SyntaxSet>();

        let trailing_comments = indentation.trailing_comments();
        let trailing_comments = TrailingComments::from_str(trailing_comments).map_err(|_| {
            ConfigError::InvalidField {
                field: "trailing_comments",
                reason: format!("invalid trailing_comments value '{trailing_comments}'"),
            }
        })?;

        let tab_space_size = indentation.tab_space_size();
        let indent_unit = indentation.indent_unit();
        let indent_unit = IndentUnit::from_type_and_size(indent_unit, tab_space_size);

        Ok(ReflowConfig {
            configs,
            config_types,
            indent_unit,
            max_line_length: core.max_line_length(),
            hanging_indents: indentation.hanging_indents(),
            allow_implicit_indents: indentation.allow_implicit_indents(),
            trailing_comments,
        })
    }
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
        let spacing = SpacingSpec::Align
            .resolve(
                SyntaxKind::AliasExpression,
                Some(SyntaxKind::SelectClause),
                Some(SyntaxKind::Statement),
            )
            .unwrap();

        assert_eq!(
            spacing,
            Spacing::Align {
                seg_type: SyntaxKind::AliasExpression,
                within: Some(SyntaxKind::SelectClause),
                scope: Some(SyntaxKind::Statement),
            }
        );
    }
}
