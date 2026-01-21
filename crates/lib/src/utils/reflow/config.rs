use std::str::FromStr;

use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::FluffConfig;
use crate::utils::reflow::depth_map::{DepthInfo, StackPositionType};
use crate::utils::reflow::rebreak::LinePositionConfig;
use crate::utils::reflow::reindent::{IndentUnit, TrailingComments};

/// Pre-computed spacing configuration for a segment type
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct SpacingConfig {
    pub spacing_before: Option<Spacing>,
    pub spacing_after: Option<Spacing>,
    pub spacing_within: Option<Spacing>,
}

/// Holds spacing config for a block and allows easy manipulation
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BlockConfig {
    pub spacing_before: Spacing,
    pub spacing_after: Spacing,
    pub spacing_within: Option<Spacing>,
    pub line_position: Option<LinePositionConfig>,
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

    /// Mutate the config based on additional information
    pub fn incorporate(
        &mut self,
        before: Option<Spacing>,
        after: Option<Spacing>,
        within: Option<Spacing>,
        line_position: Option<LinePositionConfig>,
        spacing_config: Option<&SpacingConfig>,
    ) {
        let empty = SpacingConfig::default();
        let spacing_config = spacing_config.unwrap_or(&empty);

        self.spacing_before = before
            .or(spacing_config.spacing_before)
            .unwrap_or(self.spacing_before);

        self.spacing_after = after
            .or(spacing_config.spacing_after)
            .unwrap_or(self.spacing_after);

        self.spacing_within = within.or(spacing_config.spacing_within);

        // line_position is now passed directly as a typed value
        if line_position.is_some() {
            self.line_position = line_position;
        }
    }
}

/// An interface onto the configuration of how segments should reflow.
///
/// This acts as the primary translation engine between configuration
/// held either in dicts for testing, or in the typed config in live
/// usage, and the configuration used during reflow operations.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct ReflowConfig {
    /// Pre-computed spacing configs for each segment type
    spacings: AHashMap<SyntaxKind, SpacingConfig>,
    config_types: SyntaxSet,
    /// Pre-computed line position configs for each segment type
    line_positions: AHashMap<SyntaxKind, LinePositionConfig>,
    /// In production, these values are almost _always_ set because we
    /// use `.from_typed`, but the defaults are here to aid in
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
            // Bare "align" is treated as "single" and will be converted to Align
            // later by the from_typed logic when align_within is configured
            "align" => Self::Single,
            s => {
                if let Some(rest) = s.strip_prefix("align:") {
                    let mut args = rest.split(':');

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
                            .spacings
                            .get(&seg_type)
                            .and_then(|conf| conf.spacing_before);

                        block_config.incorporate(before, None, None, None, None);
                    }
                }

                if parent_end {
                    for seg_type in configured_parent_types {
                        let after = self
                            .spacings
                            .get(&seg_type)
                            .and_then(|conf| conf.spacing_after);

                        block_config.incorporate(None, after, None, None, None);
                    }
                }
            }
        }

        for seg_type in configured_types {
            // Get line_position from the pre-computed map
            let line_position = self.line_positions.get(&seg_type).copied();
            block_config.incorporate(
                None,
                None,
                None,
                line_position,
                self.spacings.get(&seg_type),
            );
        }

        block_config
    }

    pub fn from_typed(typed: &FluffConfig) -> ReflowConfig {
        let config_types = typed.layout.types.keys().copied().collect::<SyntaxSet>();

        // Pre-compute spacing configs for each segment type
        let spacings: AHashMap<SyntaxKind, SpacingConfig> = typed
            .layout
            .types
            .iter()
            .map(|(&seg_type, value)| {
                // Use typed align_within and align_scope directly
                let align_within = value.align_within;
                let align_scope = value.align_scope;

                // Convert Spacing::Single to Spacing::Align if align_within is configured
                let convert_align = |spacing: Spacing| -> Spacing {
                    if spacing == Spacing::Single && align_within.is_some() {
                        Spacing::Align {
                            seg_type,
                            within: align_within,
                            scope: align_scope,
                        }
                    } else {
                        spacing
                    }
                };

                (
                    seg_type,
                    SpacingConfig {
                        spacing_before: value.spacing_before.map(convert_align),
                        spacing_after: value.spacing_after.map(convert_align),
                        spacing_within: value.spacing_within,
                    },
                )
            })
            .collect();

        // Pre-compute line positions for each configured type
        let line_positions: AHashMap<SyntaxKind, LinePositionConfig> = typed
            .layout
            .types
            .iter()
            .filter_map(|(&seg_type, value)| value.line_position.map(|lp| (seg_type, lp)))
            .collect();

        ReflowConfig {
            spacings,
            config_types,
            line_positions,
            indent_unit: typed.indentation.computed_indent_unit,
            max_line_length: typed.core.max_line_length as usize,
            hanging_indents: typed.indentation.hanging_indents.unwrap_or_default(),
            allow_implicit_indents: typed.indentation.allow_implicit_indents,
            trailing_comments: typed.indentation.computed_trailing_comments,
        }
    }

    /// Get the line position config for a segment type
    pub fn get_line_position(&self, seg_type: SyntaxKind) -> Option<LinePositionConfig> {
        self.line_positions.get(&seg_type).copied()
    }
}
