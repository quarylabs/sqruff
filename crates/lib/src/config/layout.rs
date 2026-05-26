use hashbrown::HashMap;
use serde::Deserialize;

use super::raw::{RawConfig, Value};

/// Typed patch for a single layout type entry
/// (e.g. `[sqruff:layout:type:comma]`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct LayoutTypeConfigPatch {
    pub spacing_before: Option<String>,
    pub spacing_after: Option<String>,
    pub spacing_within: Option<String>,
    pub line_position: Option<String>,
    pub keyword_line_position: Option<String>,
    pub keyword_line_position_exclusions: Option<String>,
}

/// Typed patch for the `[sqruff:layout]` section.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct LayoutConfigPatch {
    #[serde(rename = "type")]
    pub type_configs: HashMap<String, LayoutTypeConfigPatch>,
}

impl LayoutConfigPatch {
    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        if self.type_configs.is_empty() {
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

        for (type_name, tp) in self.type_configs {
            let entry = type_map
                .entry(type_name)
                .or_insert_with(|| Value::Map(HashMap::new()));
            let entry_map = entry.as_map_mut().expect("layout type config must be a map");

            if let Some(v) = tp.spacing_before {
                entry_map.insert("spacing_before".into(), Value::String(v.into()));
            }
            if let Some(v) = tp.spacing_after {
                entry_map.insert("spacing_after".into(), Value::String(v.into()));
            }
            if let Some(v) = tp.spacing_within {
                entry_map.insert("spacing_within".into(), Value::String(v.into()));
            }
            if let Some(v) = tp.line_position {
                entry_map.insert("line_position".into(), Value::String(v.into()));
            }
            if let Some(v) = tp.keyword_line_position {
                entry_map.insert("keyword_line_position".into(), Value::String(v.into()));
            }
            if let Some(v) = tp.keyword_line_position_exclusions {
                entry_map.insert(
                    "keyword_line_position_exclusions".into(),
                    Value::String(v.into()),
                );
            }
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
