use std::str::FromStr;

use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::DialectConfigs;

use super::ConfigError;
use super::de;
use super::layout::LayoutConfigPatch;
use super::rules::{RuleConfigSection, RuleConfigsPatch};
use super::setting::{Merge, NullableSetting, Setting};
use super::templater::TemplaterConfigPatch;

// ── CoreConfigPatch ──────────────────────────────────────────────────────────

/// Typed patch for the `[sqruff]` / core section.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoreConfigPatch {
    pub dialect: NullableSetting<String>,
    #[serde(default, deserialize_with = "de::nonnegative_usize_setting")]
    pub max_line_length: Setting<usize>,
    pub nocolor: Setting<bool>,
    #[serde(default, deserialize_with = "de::saturated_u8_setting")]
    pub verbose: Setting<u8>,
    #[serde(default, deserialize_with = "de::nonnegative_usize_setting")]
    pub output_line_length: Setting<usize>,
    #[serde(default, deserialize_with = "de::nonnegative_usize_setting")]
    pub runaway_limit: Setting<usize>,
    pub disable_noqa: Setting<bool>,
    pub warn_unused_ignores: Setting<bool>,
    pub ignore_templated_areas: Setting<bool>,
    pub fix_even_unparsable: Setting<bool>,
    #[serde(default, deserialize_with = "de::nonnegative_usize_setting")]
    pub large_file_skip_char_limit: Setting<usize>,
    #[serde(default, deserialize_with = "de::nonnegative_usize_setting")]
    pub large_file_skip_byte_limit: Setting<usize>,
    pub encoding: Setting<String>,
    #[serde(default, deserialize_with = "de::nullable_setting_csv")]
    pub ignore: NullableSetting<Vec<String>>,
    #[serde(default, deserialize_with = "de::nullable_setting_csv")]
    pub warnings: NullableSetting<Vec<String>>,
    #[serde(default, deserialize_with = "de::nullable_setting_csv")]
    pub rules: NullableSetting<Vec<String>>,
    #[serde(default, deserialize_with = "de::nullable_setting_csv")]
    pub exclude_rules: NullableSetting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_optional_string")]
    pub templater: Setting<String>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub sql_file_exts: Setting<Vec<String>>,
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
        dialect: DialectKind,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        match dialect {
            DialectKind::Postgres => self
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
            ["rules", section, key] => {
                let section = RuleConfigSection::from_str(section)
                    .map_err(|_| ConfigError::UnknownSection(format!("rules:{section}")))?;
                self.rules.merge_rule_section(
                    section,
                    &format!("inline rules section '{}'", section.as_str()),
                    &one_value_map(key, raw_value),
                )
            }
            _ => Err(ConfigError::UnknownSection(path.join(":"))),
        }
    }

    fn set_core_inline(&mut self, key: &str, raw_value: &str) -> Result<(), ConfigError> {
        match key {
            "dialect" => {
                self.core.dialect = Setting::Set(de::optional_string(raw_value));
            }
            "rules" => self.core.rules = Setting::Set(de::optional_csv(raw_value)),
            "exclude_rules" => self.core.exclude_rules = Setting::Set(de::optional_csv(raw_value)),
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
            "ignore" => self.core.ignore = Setting::Set(de::optional_csv(raw_value)),
            "warnings" => self.core.warnings = Setting::Set(de::optional_csv(raw_value)),
            "sql_file_exts" => self.core.sql_file_exts = Setting::Set(de::csv(raw_value)),
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
