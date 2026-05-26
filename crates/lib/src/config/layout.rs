use hashbrown::HashMap;
use serde::Deserialize;

use super::raw::{RawConfig, Value};
use super::setting::{Merge, Setting};

/// Typed patch for a single layout type entry
/// (e.g. `[sqruff:layout:type:comma]`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LayoutTypeConfigPatch {
    pub spacing_before: Setting<String>,
    pub spacing_after: Setting<String>,
    pub spacing_within: Setting<String>,
    pub line_position: Setting<String>,
    pub keyword_line_position: Setting<String>,
    pub keyword_line_position_exclusions: Setting<String>,
    pub align_within: Setting<String>,
    pub align_scope: Setting<String>,
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
                entry_map.insert("spacing_before".into(), Value::String(v.into()));
            }
            if let Setting::Set(v) = tp.spacing_after {
                entry_map.insert("spacing_after".into(), Value::String(v.into()));
            }
            if let Setting::Set(v) = tp.spacing_within {
                entry_map.insert("spacing_within".into(), Value::String(v.into()));
            }
            if let Setting::Set(v) = tp.line_position {
                entry_map.insert("line_position".into(), Value::String(v.into()));
            }
            if let Setting::Set(v) = tp.keyword_line_position {
                entry_map.insert("keyword_line_position".into(), Value::String(v.into()));
            }
            if let Setting::Set(v) = tp.keyword_line_position_exclusions {
                entry_map.insert(
                    "keyword_line_position_exclusions".into(),
                    Value::String(v.into()),
                );
            }
            if let Setting::Set(v) = tp.align_within {
                entry_map.insert("align_within".into(), Value::String(v.into()));
            }
            if let Setting::Set(v) = tp.align_scope {
                entry_map.insert("align_scope".into(), Value::String(v.into()));
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
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutConfig {
    pub(super) values: HashMap<String, Value>,
}

impl LayoutConfig {
    pub(super) fn from_raw(raw: &RawConfig) -> Self {
        Self {
            values: raw["layout"].as_map().unwrap().clone(),
        }
    }

    pub(crate) fn type_configs(&self) -> HashMap<String, Value> {
        self.values["type"].as_map().unwrap().clone()
    }
}
