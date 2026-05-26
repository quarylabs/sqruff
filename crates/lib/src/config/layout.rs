use hashbrown::HashMap;
use serde::{Deserialize, Deserializer, de};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::error::ConfigError;
use super::raw::{RawConfig, Value};
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

    fn from_value(seg_type: SyntaxKind, value: Value) -> Result<Self, ConfigError> {
        let map = value.as_map().ok_or_else(|| ConfigError::InvalidField {
            field: "layout.type",
            reason: format!(
                "expected map for layout type '{}'",
                syntax_kind_name(seg_type)
            ),
        })?;

        let mut patch = Self::default();
        for (key, value) in map {
            let string = value
                .as_string()
                .or_else(|| value.is_none().then_some("none"))
                .ok_or_else(|| ConfigError::InvalidField {
                    field: "layout.type",
                    reason: format!(
                        "expected string for layout option '{}' on '{}'",
                        key,
                        syntax_kind_name(seg_type)
                    ),
                })?;
            patch.apply_raw_value(seg_type, key, string)?;
        }
        Ok(patch)
    }

    fn apply_raw_value(
        &mut self,
        seg_type: SyntaxKind,
        key: &str,
        value: &str,
    ) -> Result<(), ConfigError> {
        match key {
            "spacing_before" => {
                self.spacing_before = Setting::Set(parse_spacing_spec("spacing_before", value)?)
            }
            "spacing_after" => {
                self.spacing_after = Setting::Set(parse_spacing_spec("spacing_after", value)?)
            }
            "spacing_within" => {
                self.spacing_within = Setting::Set(parse_spacing_spec("spacing_within", value)?)
            }
            "line_position" => {
                self.line_position =
                    Setting::Set(value.parse().map_err(|reason| ConfigError::InvalidField {
                        field: "line_position",
                        reason,
                    })?)
            }
            "keyword_line_position" => {
                self.keyword_line_position =
                    Setting::Set(parse_nullable_line_position(value).map_err(|err| {
                        ConfigError::InvalidField {
                            field: "keyword_line_position",
                            reason: err,
                        }
                    })?)
            }
            "keyword_line_position_exclusions" => {
                self.keyword_line_position_exclusions =
                    Setting::Set(parse_configured_syntax_set(value)?)
            }
            "align_within" => {
                self.align_within = Setting::Set(syntax_kind_from_name(value).ok_or_else(|| {
                    ConfigError::InvalidField {
                        field: "align_within",
                        reason: format!("invalid layout syntax kind '{value}'"),
                    }
                })?)
            }
            "align_scope" => {
                self.align_scope = Setting::Set(syntax_kind_from_name(value).ok_or_else(|| {
                    ConfigError::InvalidField {
                        field: "align_scope",
                        reason: format!("invalid layout syntax kind '{value}'"),
                    }
                })?)
            }
            _ => {
                return Err(ConfigError::InvalidField {
                    field: "layout.type",
                    reason: format!(
                        "unknown layout option '{}' on '{}'",
                        key,
                        syntax_kind_name(seg_type)
                    ),
                });
            }
        }
        Ok(())
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

impl LayoutConfigPatch {
    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        if self.types.is_empty() {
            return;
        }

        let layout = raw
            .entry("layout".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let layout_map = layout.as_map_mut().expect("layout must be a map");

        let type_entry = layout_map
            .entry("type".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let type_map = type_entry.as_map_mut().expect("layout.type must be a map");

        for (type_name, tp) in self.types {
            let entry = type_map
                .entry(type_name)
                .or_insert_with(|| Value::Map(HashMap::new()));
            let entry_map = entry
                .as_map_mut()
                .expect("layout type config must be a map");

            if let Setting::Set(v) = tp.spacing_before {
                entry_map.insert("spacing_before".into(), Value::String(v.as_str().into()));
            }
            if let Setting::Set(v) = tp.spacing_after {
                entry_map.insert("spacing_after".into(), Value::String(v.as_str().into()));
            }
            if let Setting::Set(v) = tp.spacing_within {
                entry_map.insert("spacing_within".into(), Value::String(v.as_str().into()));
            }
            if let Setting::Set(v) = tp.line_position {
                entry_map.insert(
                    "line_position".into(),
                    Value::String(v.to_config_string().into()),
                );
            }
            if let Setting::Set(Some(v)) = tp.keyword_line_position {
                entry_map.insert(
                    "keyword_line_position".into(),
                    Value::String(line_position_name(v).into()),
                );
            }
            if let Setting::Set(None) = tp.keyword_line_position {
                entry_map.insert("keyword_line_position".into(), Value::None);
            }
            if let Setting::Set(v) = tp.keyword_line_position_exclusions {
                entry_map.insert(
                    "keyword_line_position_exclusions".into(),
                    Value::String(syntax_set_to_string(&v).into()),
                );
            }
            if let Setting::Set(v) = tp.align_within {
                entry_map.insert(
                    "align_within".into(),
                    Value::String(syntax_kind_name(v).into()),
                );
            }
            if let Setting::Set(v) = tp.align_scope {
                entry_map.insert(
                    "align_scope".into(),
                    Value::String(syntax_kind_name(v).into()),
                );
            }
        }
    }
}

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
    pub(crate) fn from_raw(raw: &RawConfig) -> Result<Self, ConfigError> {
        let values = raw["layout"]["type"].as_map().cloned().unwrap_or_default();
        let mut types = HashMap::new();

        for (name, value) in values {
            let seg_type =
                syntax_kind_from_name(&name).ok_or_else(|| ConfigError::InvalidField {
                    field: "layout.type",
                    reason: format!("invalid layout syntax kind '{name}'"),
                })?;
            let patch = LayoutTypeConfigPatch::from_value(seg_type, value)?;
            types.insert(seg_type, patch.resolve(seg_type)?);
        }

        Ok(Self { types })
    }
}

fn parse_spacing_spec(field: &'static str, value: &str) -> Result<SpacingSpec, ConfigError> {
    value
        .parse()
        .map_err(|reason| ConfigError::InvalidField { field, reason })
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

fn parse_configured_syntax_set(raw: &str) -> Result<SyntaxSet, ConfigError> {
    parse_syntax_set_values(
        raw.split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
    )
    .map_err(|reason| ConfigError::InvalidField {
        field: "keyword_line_position_exclusions",
        reason,
    })
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

fn syntax_kind_name(kind: SyntaxKind) -> &'static str {
    kind.into()
}

fn line_position_name(position: LinePosition) -> &'static str {
    match position {
        LinePosition::Leading => "leading",
        LinePosition::Trailing => "trailing",
        LinePosition::Alone => "alone",
        LinePosition::Strict => "strict",
    }
}

fn syntax_set_to_string(value: &SyntaxSet) -> String {
    value
        .into_iter()
        .map(syntax_kind_name)
        .collect::<Vec<_>>()
        .join(",")
}
