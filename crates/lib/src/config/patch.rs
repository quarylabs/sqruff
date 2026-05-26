use hashbrown::HashMap;

use super::de;
use super::layout::LayoutConfigPatch;
use super::raw::{RawConfig, Value, insert_config_path, merge_configs};
use super::rules::RuleConfigsPatch;
use super::templater::TemplaterConfigPatch;

// ── CoreConfigPatch ──────────────────────────────────────────────────────────

/// Typed patch for the `[sqruff]` / core section.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct CoreConfigPatch {
    pub dialect: Option<String>,
    pub max_line_length: Option<usize>,
    pub nocolor: Option<bool>,
    pub verbose: Option<i32>,
    pub disable_noqa: Option<bool>,
    #[serde(default, deserialize_with = "de::opt_csv")]
    pub rules: Option<Vec<String>>,
    #[serde(default, deserialize_with = "de::opt_csv")]
    pub exclude_rules: Option<Vec<String>>,
    pub templater: Option<String>,
    #[serde(default, deserialize_with = "de::opt_csv")]
    pub sql_file_exts: Option<Vec<String>>,
}

impl CoreConfigPatch {
    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        let core = raw
            .entry("core".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let map = core.as_map_mut().expect("core must be a map");

        if let Some(v) = self.dialect {
            map.insert("dialect".into(), Value::String(v.into()));
        }
        if let Some(v) = self.max_line_length {
            map.insert("max_line_length".into(), Value::Int(v as i32));
        }
        if let Some(v) = self.nocolor {
            map.insert("nocolor".into(), Value::Bool(v));
        }
        if let Some(v) = self.verbose {
            map.insert("verbose".into(), Value::Int(v));
        }
        if let Some(v) = self.disable_noqa {
            map.insert("disable_noqa".into(), Value::Bool(v));
        }
        if let Some(rules) = self.rules {
            map.insert("rules".into(), Value::String(rules.join(",").into()));
        }
        if let Some(exclude) = self.exclude_rules {
            map.insert(
                "exclude_rules".into(),
                Value::String(exclude.join(",").into()),
            );
        }
        if let Some(v) = self.templater {
            map.insert("templater".into(), Value::String(v.into()));
        }
        if let Some(exts) = self.sql_file_exts {
            map.insert(
                "sql_file_exts".into(),
                Value::String(exts.join(",").into()),
            );
        }
    }
}

// ── IndentationConfigPatch ───────────────────────────────────────────────────

/// Typed patch for the `[sqruff:indentation]` section.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct IndentationConfigPatch {
    pub indent_unit: Option<String>,
    pub tab_space_size: Option<usize>,
    pub hanging_indents: Option<bool>,
    pub allow_implicit_indents: Option<bool>,
    pub trailing_comments: Option<String>,
}

impl IndentationConfigPatch {
    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        let indentation = raw
            .entry("indentation".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let map = indentation.as_map_mut().expect("indentation must be a map");

        if let Some(v) = self.indent_unit {
            map.insert("indent_unit".into(), Value::String(v.into()));
        }
        if let Some(v) = self.tab_space_size {
            map.insert("tab_space_size".into(), Value::Int(v as i32));
        }
        if let Some(v) = self.hanging_indents {
            map.insert("hanging_indents".into(), Value::Bool(v));
        }
        if let Some(v) = self.allow_implicit_indents {
            map.insert("allow_implicit_indents".into(), Value::Bool(v));
        }
        if let Some(v) = self.trailing_comments {
            map.insert("trailing_comments".into(), Value::String(v.into()));
        }
    }
}

// ── DialectConfigPatch ───────────────────────────────────────────────────────

/// Typed patch for `[sqruff:dialect:<name>]` sections.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct DialectConfigPatch {
    #[serde(flatten)]
    pub dialects: HashMap<String, Value>,
}

impl DialectConfigPatch {
    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        if self.dialects.is_empty() {
            return;
        }
        let dialect = raw
            .entry("dialect".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let dialect_map = dialect.as_map_mut().expect("dialect must be a map");
        dialect_map.extend(self.dialects);
    }
}

// ── ConfigPatch ──────────────────────────────────────────────────────────────

/// Top-level typed config patch.
///
/// Implements [`serde::Deserialize`] so it can be constructed from structured
/// formats (JSON, YAML, …) as well as programmatically.  The `raw` escape
/// hatch (populated via [`set_value`] / [`set_string`] / [`from_sections`])
/// is applied last and overrides the typed fields, preserving full backward
/// compatibility with callers that build patches using the old key-path API.
///
/// [`set_value`]: ConfigPatch::set_value
/// [`set_string`]: ConfigPatch::set_string
/// [`from_sections`]: ConfigPatch::from_sections
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfigPatch {
    pub core: CoreConfigPatch,
    pub indentation: IndentationConfigPatch,
    pub layout: LayoutConfigPatch,
    pub templater: TemplaterConfigPatch,
    pub rules: RuleConfigsPatch,
    #[serde(rename = "dialect")]
    pub dialects: DialectConfigPatch,

    /// Untyped overrides written via the key-path API.  Applied on top of the
    /// typed fields so that `set_value` always takes precedence.
    #[serde(skip)]
    raw: RawConfig,
}

impl ConfigPatch {
    /// Construct a patch from an existing map of top-level config sections.
    ///
    /// Backward-compatible entry point for callers that build raw config maps
    /// directly (e.g. test helpers).
    pub fn from_sections(sections: HashMap<String, Value>) -> Self {
        Self {
            raw: sections,
            ..Default::default()
        }
    }

    /// Set a string value at the given nested path, creating intermediate maps
    /// as needed.
    pub fn set_string(&mut self, path: &[&str], value: &str) {
        self.set_value(path, Value::String(value.into()));
    }

    /// Set an arbitrary value at the given nested path, creating intermediate
    /// maps as needed.
    pub fn set_value(&mut self, path: &[&str], value: Value) {
        let path: Vec<String> = path.iter().map(|s| s.to_string()).collect();
        insert_config_path(&mut self.raw, &path, value);
    }

    /// Return the value at the given nested path from the untyped `raw` map,
    /// if it exists.
    pub fn value(&self, path: &[&str]) -> Option<&Value> {
        let (first, rest) = path.split_first()?;
        let mut current = self.raw.get(*first)?;
        for key in rest {
            current = current.as_map()?.get(*key)?;
        }
        Some(current)
    }
}

impl From<ConfigPatch> for RawConfig {
    fn from(patch: ConfigPatch) -> Self {
        // 1. Convert typed fields to raw representation.
        let mut typed_raw = RawConfig::new();
        patch.core.merge_into_raw(&mut typed_raw);
        patch.indentation.merge_into_raw(&mut typed_raw);
        patch.layout.merge_into_raw(&mut typed_raw);
        patch.templater.merge_into_raw(&mut typed_raw);
        patch.rules.merge_into_raw(&mut typed_raw);
        patch.dialects.merge_into_raw(&mut typed_raw);

        // 2. Apply untyped overrides on top (they take precedence).
        merge_configs(typed_raw, patch.raw)
    }
}
