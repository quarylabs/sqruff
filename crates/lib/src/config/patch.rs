use std::fmt;
use std::str::FromStr;

use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::DialectConfigs;

use super::ConfigError;
use super::de;
use super::layout::LayoutConfigPatch;
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

impl CoreConfigPatch {}

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
    pub indented_joins_on: Setting<bool>,
    pub hanging_indents: Setting<bool>,
    pub allow_implicit_indents: Setting<bool>,
    pub template_blocks_indent: Setting<bool>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub skip_indentation_in: Setting<Vec<String>>,
    pub trailing_comments: Setting<String>,
}

impl IndentationConfigPatch {}

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
        self.indented_joins_on.merge(other.indented_joins_on);
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
#[serde(default, deny_unknown_fields)]
pub struct DialectConfigPatch {
    pub postgres: PostgresDialectConfigPatch,
}

impl DialectConfigPatch {
    pub(crate) fn merge_section(
        &mut self,
        dialect: String,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        match dialect.as_str() {
            "postgres" => self
                .postgres
                .merge(de::deserialize_section(section_name, values)?),
            _ if values.is_empty() => {}
            _ => return Err(ConfigError::UnknownSection(section_name.to_string())),
        }
        Ok(())
    }

    pub(crate) fn resolve(&self) -> DialectConfigs {
        let mut configs = DialectConfigs::default();
        configs.postgres.pg_trgm = self.postgres.pg_trgm.clone().into_option().unwrap_or(false);
        configs.postgres.pgvector = self
            .postgres
            .pgvector
            .clone()
            .into_option()
            .unwrap_or(false);
        configs
    }
}

impl Merge for DialectConfigPatch {
    fn merge(&mut self, other: Self) {
        self.postgres.merge(other.postgres);
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PostgresDialectConfigPatch {
    pub pg_trgm: Setting<bool>,
    pub pgvector: Setting<bool>,
}

impl Merge for PostgresDialectConfigPatch {
    fn merge(&mut self, other: Self) {
        self.pg_trgm.merge(other.pg_trgm);
        self.pgvector.merge(other.pgvector);
    }
}

// ── ConfigPatch ──────────────────────────────────────────────────────────────

/// Top-level typed config patch.
///
/// Implements [`serde::Deserialize`] so it can be constructed from structured
/// formats (JSON, YAML, …) as well as programmatically.
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
}

impl ConfigPatch {
    pub fn set_dialect(&mut self, dialect: Option<DialectKind>) {
        self.core.dialect = Setting::Set(dialect.map(|dialect| dialect.as_ref().to_string()));
    }

    pub fn set_rules(&mut self, rules: Option<Vec<String>>) {
        self.core.rules = Setting::Set(rules);
    }

    pub fn set_exclude_rules(&mut self, exclude_rules: Option<Vec<String>>) {
        self.core.exclude_rules = Setting::Set(exclude_rules);
    }

    pub fn set_core_templater(&mut self, templater: impl Into<String>) {
        self.core.templater = Setting::Set(templater.into());
    }

    /// SQLFluff rule fixtures historically place some indentation options
    /// under `rules`; move those into the typed indentation patch.
    pub fn move_fixture_indentation_options(&mut self) {
        if let Setting::Set(value) = std::mem::take(&mut self.rules.indent_unit) {
            self.indentation.indent_unit = Setting::Set(value);
        }

        if let Setting::Set(value) = std::mem::take(&mut self.rules.tab_space_size) {
            self.indentation.tab_space_size = Setting::Set(value);
        }
    }

    pub(crate) fn set_inline(&mut self, path: &[&str], raw_value: &str) -> Result<(), ConfigError> {
        match path {
            ["core", key] => self.set_core_inline(key, raw_value),
            ["rules", section, key] => self.rules.merge_rule_section(
                (*section).to_string(),
                &format!("inline rules section '{section}'"),
                &one_value_map(key, raw_value),
            ),
            _ => Err(ConfigError::UnknownSection(path.join(":"))),
        }
    }

    fn set_core_inline(&mut self, key: &str, raw_value: &str) -> Result<(), ConfigError> {
        match key {
            "dialect" => {
                self.core.dialect = Setting::Set(optional_string(raw_value));
            }
            "rules" => self.core.rules = Setting::Set(optional_csv(raw_value)),
            "exclude_rules" => self.core.exclude_rules = Setting::Set(optional_csv(raw_value)),
            "templater" => self.core.templater = Setting::Set(raw_value.to_string()),
            "nocolor" => self.core.nocolor = Setting::Set(parse_bool(raw_value)?),
            "verbose" => self.core.verbose = Setting::Set(parse_u8(raw_value)?),
            "output_line_length" => {
                self.core.output_line_length = Setting::Set(parse_usize(raw_value)?)
            }
            "runaway_limit" => self.core.runaway_limit = Setting::Set(parse_usize(raw_value)?),
            "disable_noqa" => self.core.disable_noqa = Setting::Set(parse_bool(raw_value)?),
            "warn_unused_ignores" => {
                self.core.warn_unused_ignores = Setting::Set(parse_bool(raw_value)?)
            }
            "ignore_templated_areas" => {
                self.core.ignore_templated_areas = Setting::Set(parse_bool(raw_value)?)
            }
            "fix_even_unparsable" => {
                self.core.fix_even_unparsable = Setting::Set(parse_bool(raw_value)?)
            }
            "large_file_skip_char_limit" => {
                self.core.large_file_skip_char_limit = Setting::Set(parse_usize(raw_value)?)
            }
            "large_file_skip_byte_limit" => {
                self.core.large_file_skip_byte_limit = Setting::Set(parse_usize(raw_value)?)
            }
            "encoding" => self.core.encoding = Setting::Set(raw_value.to_string()),
            "ignore" => self.core.ignore = Setting::Set(optional_csv(raw_value)),
            "warnings" => self.core.warnings = Setting::Set(optional_csv(raw_value)),
            "sql_file_exts" => self.core.sql_file_exts = Setting::Set(csv(raw_value)),
            "max_line_length" => self.core.max_line_length = Setting::Set(parse_usize(raw_value)?),
            _ => {
                return Err(ConfigError::InvalidField {
                    field: "core",
                    reason: format!("unknown core option '{key}'"),
                });
            }
        }
        Ok(())
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
    }
}

fn one_value_map(key: &str, value: &str) -> std::collections::HashMap<String, Option<String>> {
    std::iter::once((key.to_string(), Some(value.to_string()))).collect()
}

fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none")).then(|| value.to_string())
}

fn optional_csv(value: &str) -> Option<Vec<String>> {
    optional_string(value).map(|value| csv(&value))
}

fn csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_bool(value: &str) -> Result<bool, ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigError::InvalidField {
            field: "core",
            reason: format!("invalid bool '{value}'"),
        }),
    }
}

fn parse_usize(value: &str) -> Result<usize, ConfigError> {
    i64::from_str(value.trim())
        .map(|value| value.max(0) as usize)
        .map_err(|_| ConfigError::InvalidField {
            field: "core",
            reason: format!("invalid integer '{value}'"),
        })
}

fn parse_u8(value: &str) -> Result<u8, ConfigError> {
    Ok(parse_usize(value)?.min(usize::from(u8::MAX)) as u8)
}
