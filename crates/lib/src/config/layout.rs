use hashbrown::HashMap;
use serde::{Deserialize, Deserializer, de};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::error::ConfigError;
use super::setting::{Merge, NullableSetting, Setting};
use crate::utils::reflow::config::{LinePositionConfig, Spacing, SpacingSpec};
use crate::utils::reflow::rebreak::LinePosition;

/// Typed patch for a single layout type entry
/// (e.g. `[sqruff:layout:type:comma]`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LayoutTypeConfigPatch {
    #[serde(default, deserialize_with = "super::de::setting_from_str")]
    pub spacing_before: Setting<SpacingSpec>,
    #[serde(default, deserialize_with = "super::de::setting_from_str")]
    pub spacing_after: Setting<SpacingSpec>,
    #[serde(default, deserialize_with = "super::de::setting_from_str")]
    pub spacing_within: Setting<SpacingSpec>,
    #[serde(default, deserialize_with = "setting_syntax_kind")]
    pub align_within: Setting<SyntaxKind>,
    #[serde(default, deserialize_with = "setting_syntax_kind")]
    pub align_scope: Setting<SyntaxKind>,
    #[serde(default, deserialize_with = "super::de::setting_from_str")]
    pub line_position: Setting<LinePositionConfig>,
    #[serde(default, deserialize_with = "nullable_setting_line_position")]
    pub keyword_line_position: NullableSetting<LinePosition>,
    #[serde(default, deserialize_with = "setting_syntax_set")]
    pub keyword_line_position_exclusions: Setting<SyntaxSet>,
}

impl LayoutTypeConfigPatch {
    pub fn resolve(self, seg_type: SyntaxKind) -> Result<LayoutTypeConfig, ConfigError> {
        let align_within = self.align_within.into_option();
        let align_scope = self.align_scope.into_option();

        Ok(LayoutTypeConfig {
            spacing_before: self
                .spacing_before
                .into_option()
                .map(|spec| spec.resolve(seg_type, align_within, align_scope))
                .transpose()?,
            spacing_after: self
                .spacing_after
                .into_option()
                .map(|spec| spec.resolve(seg_type, align_within, align_scope))
                .transpose()?,
            spacing_within: self
                .spacing_within
                .into_option()
                .map(|spec| spec.resolve(seg_type, align_within, align_scope))
                .transpose()?,
            line_position: self.line_position.into_option(),
            keyword_line_position: self.keyword_line_position.into_option().flatten(),
            keyword_line_position_exclusions: self
                .keyword_line_position_exclusions
                .into_option()
                .unwrap_or(SyntaxSet::EMPTY),
        })
    }
}

impl Merge for LayoutTypeConfigPatch {
    fn merge(&mut self, other: Self) {
        self.spacing_before.merge(other.spacing_before);
        self.spacing_after.merge(other.spacing_after);
        self.spacing_within.merge(other.spacing_within);
        self.line_position.merge(other.line_position);
        self.keyword_line_position
            .merge(other.keyword_line_position);
        self.keyword_line_position_exclusions
            .merge(other.keyword_line_position_exclusions);
        self.align_within.merge(other.align_within);
        self.align_scope.merge(other.align_scope);
    }
}

/// Typed patch for the `[sqruff:layout]` section.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct LayoutConfigPatch {
    #[serde(rename = "type")]
    pub types: HashMap<String, LayoutTypeConfigPatch>,
}

impl LayoutConfigPatch {}

impl Merge for LayoutConfigPatch {
    fn merge(&mut self, other: Self) {
        for (key, value) in other.types {
            self.types.entry(key).or_default().merge(value);
        }
    }
}

/// Resolved layout configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutConfig {
    pub types: HashMap<SyntaxKind, LayoutTypeConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutTypeConfig {
    pub spacing_before: Option<Spacing>,
    pub spacing_after: Option<Spacing>,
    pub spacing_within: Option<Spacing>,
    pub line_position: Option<LinePositionConfig>,
    pub keyword_line_position: Option<LinePosition>,
    pub keyword_line_position_exclusions: SyntaxSet,
}

impl LayoutConfig {
    pub(crate) fn from_patch(patch: &LayoutConfigPatch) -> Result<Self, ConfigError> {
        let mut types = HashMap::new();

        for (name, value) in &patch.types {
            let seg_type =
                syntax_kind_from_name(name).ok_or_else(|| ConfigError::InvalidField {
                    field: "layout.type",
                    reason: format!("invalid layout syntax kind '{name}'"),
                })?;
            types.insert(seg_type, value.clone().resolve(seg_type)?);
        }

        Ok(Self { types })
    }
}

fn setting_syntax_set<'de, D>(d: D) -> Result<Setting<SyntaxSet>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Option::<StringOrVec>::deserialize(d)?;
    parse_syntax_set_values(values.map(StringOrVec::into_vec).unwrap_or_default())
        .map(Setting::Set)
        .map_err(de::Error::custom)
}

fn setting_syntax_kind<'de, D>(d: D) -> Result<Setting<SyntaxKind>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(d)?;
    syntax_kind_from_name(&value)
        .map(Setting::Set)
        .ok_or_else(|| de::Error::custom(format!("invalid layout syntax kind '{value}'")))
}

fn nullable_setting_line_position<'de, D>(d: D) -> Result<NullableSetting<LinePosition>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(d)?;
    match value {
        Some(value) => parse_nullable_line_position(&value)
            .map(Setting::Set)
            .map_err(de::Error::custom),
        None => Ok(Setting::Set(None)),
    }
}

fn parse_nullable_line_position(value: &str) -> Result<Option<LinePosition>, String> {
    if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("none") {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|err| format!("invalid line position '{}': {err}", value))
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    Str(String),
    Vec(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringOrVec::Str(value) => value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            StringOrVec::Vec(values) => values,
        }
    }
}

fn parse_syntax_set_values(values: Vec<String>) -> Result<SyntaxSet, String> {
    let mut syntax_set = SyntaxSet::EMPTY;
    for value in values {
        if value.eq_ignore_ascii_case("none") {
            continue;
        }
        let kind = syntax_kind_from_name(&value)
            .ok_or_else(|| format!("invalid layout syntax kind '{value}'"))?;
        syntax_set.insert(kind);
    }
    Ok(syntax_set)
}

fn syntax_kind_from_name(seg_type: &str) -> Option<SyntaxKind> {
    match seg_type {
        "aggregate_order_by" => Some(SyntaxKind::AggregateOrderByClause),
        _ => seg_type.parse().ok(),
    }
}
