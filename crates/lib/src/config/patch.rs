use std::fmt;

use hashbrown::HashMap;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::de;
use super::layout::LayoutConfigPatch;
use super::raw::{RawConfig, Value, insert_config_path, merge_configs};
use super::rules::RuleConfigsPatch;
use super::setting::{Merge, NullableSetting, Setting};
use super::templater::TemplaterConfigPatch;

// ── CoreConfigPatch ──────────────────────────────────────────────────────────

/// Typed patch for the `[sqruff]` / core section.
#[derive(Debug, Clone, Default)]
pub struct CoreConfigPatch {
    pub dialect: NullableSetting<String>,
    pub max_line_length: Setting<usize>,
    pub nocolor: Setting<bool>,
    pub verbose: Setting<u8>,
    pub output_line_length: Setting<usize>,
    pub runaway_limit: Setting<usize>,
    pub disable_noqa: Setting<bool>,
    pub warn_unused_ignores: Setting<bool>,
    pub ignore_templated_areas: Setting<bool>,
    pub fix_even_unparsable: Setting<bool>,
    pub large_file_skip_char_limit: Setting<usize>,
    pub large_file_skip_byte_limit: Setting<usize>,
    pub encoding: Setting<String>,
    pub ignore: NullableSetting<Vec<String>>,
    pub warnings: NullableSetting<Vec<String>>,
    pub rules: NullableSetting<Vec<String>>,
    pub exclude_rules: NullableSetting<Vec<String>>,
    pub templater: Setting<String>,
    pub sql_file_exts: Setting<Vec<String>>,
}

impl<'de> Deserialize<'de> for CoreConfigPatch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(CoreConfigPatchVisitor)
    }
}

struct CoreConfigPatchVisitor;

impl<'de> Visitor<'de> for CoreConfigPatchVisitor {
    type Value = CoreConfigPatch;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a core config patch")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        const FIELDS: &[&str] = &[
            "dialect",
            "max_line_length",
            "nocolor",
            "verbose",
            "output_line_length",
            "runaway_limit",
            "disable_noqa",
            "warn_unused_ignores",
            "ignore_templated_areas",
            "fix_even_unparsable",
            "large_file_skip_char_limit",
            "large_file_skip_byte_limit",
            "encoding",
            "ignore",
            "warnings",
            "rules",
            "exclude_rules",
            "templater",
            "sql_file_exts",
        ];

        let mut patch = CoreConfigPatch::default();

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "dialect" => patch.dialect = Setting::Set(map.next_value()?),
                "max_line_length" => patch.max_line_length = next_usize_setting(&mut map)?,
                "nocolor" => patch.nocolor = map.next_value()?,
                "verbose" => patch.verbose = map.next_value()?,
                "output_line_length" => patch.output_line_length = next_usize_setting(&mut map)?,
                "runaway_limit" => patch.runaway_limit = next_usize_setting(&mut map)?,
                "disable_noqa" => patch.disable_noqa = map.next_value()?,
                "warn_unused_ignores" => patch.warn_unused_ignores = map.next_value()?,
                "ignore_templated_areas" => {
                    patch.ignore_templated_areas = map.next_value()?;
                }
                "fix_even_unparsable" => patch.fix_even_unparsable = map.next_value()?,
                "large_file_skip_char_limit" => {
                    patch.large_file_skip_char_limit = next_usize_setting(&mut map)?;
                }
                "large_file_skip_byte_limit" => {
                    patch.large_file_skip_byte_limit = next_usize_setting(&mut map)?;
                }
                "encoding" => patch.encoding = map.next_value()?,
                "ignore" => patch.ignore = map.next_value()?,
                "warnings" => patch.warnings = map.next_value()?,
                "rules" => patch.rules = map.next_value()?,
                "exclude_rules" => patch.exclude_rules = map.next_value()?,
                "templater" => {
                    if let OptionalString::Some(value) = map.next_value()? {
                        patch.templater = Setting::Set(value);
                    }
                }
                "sql_file_exts" => patch.sql_file_exts = map.next_value()?,
                _ => return Err(serde::de::Error::unknown_field(&key, FIELDS)),
            }
        }

        Ok(patch)
    }
}

fn next_usize_setting<'de, M>(map: &mut M) -> Result<Setting<usize>, M::Error>
where
    M: MapAccess<'de>,
{
    let value = map.next_value::<i64>()?;
    Ok(Setting::Set(value.max(0) as usize))
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionalString {
    Some(String),
    Ignored(IgnoredAny),
}

impl CoreConfigPatch {
    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        let core = raw
            .entry("core".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let map = core.as_map_mut().expect("core must be a map");

        match self.dialect {
            Setting::Unset => {}
            Setting::Set(Some(v)) => {
                map.insert("dialect".into(), Value::String(v.into()));
            }
            Setting::Set(None) => {
                map.insert("dialect".into(), Value::None);
            }
        }
        if let Setting::Set(v) = self.max_line_length {
            map.insert("max_line_length".into(), Value::Int(v as i32));
        }
        if let Setting::Set(v) = self.nocolor {
            map.insert("nocolor".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.verbose {
            map.insert("verbose".into(), Value::Int(i32::from(v)));
        }
        if let Setting::Set(v) = self.output_line_length {
            map.insert("output_line_length".into(), Value::Int(v as i32));
        }
        if let Setting::Set(v) = self.runaway_limit {
            map.insert("runaway_limit".into(), Value::Int(v as i32));
        }
        if let Setting::Set(v) = self.disable_noqa {
            map.insert("disable_noqa".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.warn_unused_ignores {
            map.insert("warn_unused_ignores".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.ignore_templated_areas {
            map.insert("ignore_templated_areas".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.fix_even_unparsable {
            map.insert("fix_even_unparsable".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.large_file_skip_char_limit {
            map.insert("large_file_skip_char_limit".into(), Value::Int(v as i32));
        }
        if let Setting::Set(v) = self.large_file_skip_byte_limit {
            map.insert("large_file_skip_byte_limit".into(), Value::Int(v as i32));
        }
        if let Setting::Set(v) = self.encoding {
            map.insert("encoding".into(), Value::String(v.into()));
        }
        merge_nullable_csv(map, "ignore", self.ignore);
        merge_nullable_csv(map, "warnings", self.warnings);
        match self.rules {
            Setting::Unset => {}
            Setting::Set(Some(rules)) => {
                map.insert("rules".into(), Value::String(rules.join(",").into()));
            }
            Setting::Set(None) => {
                map.insert("rules".into(), Value::None);
            }
        }
        match self.exclude_rules {
            Setting::Unset => {}
            Setting::Set(Some(exclude)) => {
                map.insert(
                    "exclude_rules".into(),
                    Value::String(exclude.join(",").into()),
                );
            }
            Setting::Set(None) => {
                map.insert("exclude_rules".into(), Value::None);
            }
        }
        if let Setting::Set(v) = self.templater {
            map.insert("templater".into(), Value::String(v.into()));
        }
        if let Setting::Set(exts) = self.sql_file_exts {
            map.insert("sql_file_exts".into(), Value::String(exts.join(",").into()));
        }
    }
}

impl Merge for CoreConfigPatch {
    fn merge(&mut self, other: Self) {
        self.dialect.merge(other.dialect);
        self.max_line_length.merge(other.max_line_length);
        self.nocolor.merge(other.nocolor);
        self.verbose.merge(other.verbose);
        self.output_line_length.merge(other.output_line_length);
        self.runaway_limit.merge(other.runaway_limit);
        self.disable_noqa.merge(other.disable_noqa);
        self.warn_unused_ignores.merge(other.warn_unused_ignores);
        self.ignore_templated_areas
            .merge(other.ignore_templated_areas);
        self.fix_even_unparsable.merge(other.fix_even_unparsable);
        self.large_file_skip_char_limit
            .merge(other.large_file_skip_char_limit);
        self.large_file_skip_byte_limit
            .merge(other.large_file_skip_byte_limit);
        self.encoding.merge(other.encoding);
        self.ignore.merge(other.ignore);
        self.warnings.merge(other.warnings);
        self.rules.merge(other.rules);
        self.exclude_rules.merge(other.exclude_rules);
        self.templater.merge(other.templater);
        self.sql_file_exts.merge(other.sql_file_exts);
    }
}

fn merge_nullable_csv(
    map: &mut HashMap<String, Value>,
    key: &'static str,
    value: NullableSetting<Vec<String>>,
) {
    match value {
        Setting::Unset => {}
        Setting::Set(Some(values)) => {
            map.insert(key.into(), Value::String(values.join(",").into()));
        }
        Setting::Set(None) => {
            map.insert(key.into(), Value::None);
        }
    }
}

// ── IndentationConfigPatch ───────────────────────────────────────────────────

/// Typed patch for the `[sqruff:indentation]` section.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct IndentationConfigPatch {
    pub indent_unit: Setting<String>,
    pub tab_space_size: Setting<usize>,
    pub indented_joins: Setting<bool>,
    pub indented_ctes: Setting<bool>,
    pub indented_using_on: Setting<bool>,
    pub indented_on_contents: Setting<bool>,
    pub indented_then: Setting<bool>,
    pub indented_then_contents: Setting<bool>,
    pub hanging_indents: Setting<bool>,
    pub allow_implicit_indents: Setting<bool>,
    pub template_blocks_indent: Setting<bool>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub skip_indentation_in: Setting<Vec<String>>,
    pub trailing_comments: Setting<String>,
}

impl IndentationConfigPatch {
    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        let indentation = raw
            .entry("indentation".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let map = indentation.as_map_mut().expect("indentation must be a map");

        if let Setting::Set(v) = self.indent_unit {
            map.insert("indent_unit".into(), Value::String(v.into()));
        }
        if let Setting::Set(v) = self.tab_space_size {
            map.insert("tab_space_size".into(), Value::Int(v as i32));
        }
        if let Setting::Set(v) = self.indented_joins {
            map.insert("indented_joins".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.indented_ctes {
            map.insert("indented_ctes".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.indented_using_on {
            map.insert("indented_using_on".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.indented_on_contents {
            map.insert("indented_on_contents".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.indented_then {
            map.insert("indented_then".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.indented_then_contents {
            map.insert("indented_then_contents".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.hanging_indents {
            map.insert("hanging_indents".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.allow_implicit_indents {
            map.insert("allow_implicit_indents".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.template_blocks_indent {
            map.insert("template_blocks_indent".into(), Value::Bool(v));
        }
        if let Setting::Set(v) = self.skip_indentation_in {
            map.insert(
                "skip_indentation_in".into(),
                Value::String(v.join(",").into()),
            );
        }
        if let Setting::Set(v) = self.trailing_comments {
            map.insert("trailing_comments".into(), Value::String(v.into()));
        }
    }
}

impl Merge for IndentationConfigPatch {
    fn merge(&mut self, other: Self) {
        self.indent_unit.merge(other.indent_unit);
        self.tab_space_size.merge(other.tab_space_size);
        self.indented_joins.merge(other.indented_joins);
        self.indented_ctes.merge(other.indented_ctes);
        self.indented_using_on.merge(other.indented_using_on);
        self.indented_on_contents.merge(other.indented_on_contents);
        self.indented_then.merge(other.indented_then);
        self.indented_then_contents
            .merge(other.indented_then_contents);
        self.hanging_indents.merge(other.hanging_indents);
        self.allow_implicit_indents
            .merge(other.allow_implicit_indents);
        self.template_blocks_indent
            .merge(other.template_blocks_indent);
        self.skip_indentation_in.merge(other.skip_indentation_in);
        self.trailing_comments.merge(other.trailing_comments);
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

impl Merge for DialectConfigPatch {
    fn merge(&mut self, other: Self) {
        self.dialects.extend(other.dialects);
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

    /// SQLFluff rule fixtures historically place some indentation options
    /// under `rules`; move those into the typed indentation patch.
    pub fn move_fixture_indentation_options(&mut self) {
        if let Some(value) = self.rules.configs.remove("indent_unit") {
            if let Some(value) = value.as_string() {
                self.indentation.indent_unit = Setting::Set(value.to_string());
            }
        }

        if let Some(value) = self.rules.configs.remove("tab_space_size") {
            if let Some(value) = value.as_int() {
                self.indentation.tab_space_size = Setting::Set(value.max(0) as usize);
            }
        }
    }
}

impl Merge for ConfigPatch {
    fn merge(&mut self, other: Self) {
        self.core.merge(other.core);
        self.indentation.merge(other.indentation);
        self.layout.merge(other.layout);
        self.templater.merge(other.templater);
        self.rules.merge(other.rules);
        self.dialects.merge(other.dialects);
        self.raw = merge_configs(std::mem::take(&mut self.raw), other.raw);
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
