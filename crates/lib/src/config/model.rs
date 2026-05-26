use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use hashbrown::HashMap;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::parser::{IndentationConfig as ParserIndentationConfig, Parser};
use sqruff_lib_dialects::kind_to_dialect;

use super::loader::ConfigLoader;
use super::options::{ConfigInput, ConfigLoadOptions, ConfigOverrides};
use super::patch::ConfigPatch;
use super::raw::{RawConfig, Value, split_comma_separated_string};
use crate::api::SqruffError;
use crate::templaters::TemplaterKind;
use crate::utils::reflow::config::ReflowConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct FluffConfig {
    raw: RawConfig,
    core: CoreConfig,
    indentation: IndentationConfig,
    layout: LayoutConfig,
    rule_config: RuleConfigStore,
    templater_config: TemplaterConfigStore,

    dialect_kind: DialectKind,
    dialect: Dialect,
    templater_kind: TemplaterKind,
    sql_file_exts: Vec<String>,
    reflow: ReflowConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreConfig {
    no_color: bool,
    verbosity: i32,
    disable_noqa: bool,
    max_line_length: usize,
    rule_allowlist: Option<Vec<String>>,
    rule_denylist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndentationConfig {
    values: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutConfig {
    values: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleConfigStore {
    values: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplaterConfigStore {
    values: HashMap<String, Value>,
}

impl Default for FluffConfig {
    fn default() -> Self {
        Self::from_patch(ConfigPatch::default())
    }
}

impl FluffConfig {
    pub(crate) fn build_from_raw(configs: RawConfig) -> Self {
        let values = ConfigLoader::try_get_config_elems_from_file(
            None,
            include_str!("./default_config.cfg").into(),
        )
        .expect("built-in default config must be valid");

        let mut defaults = HashMap::new();
        ConfigLoader::incorporate_vals(&mut defaults, values);

        let mut raw = nested_combine(defaults, configs);
        normalize_core_lists(&mut raw);

        let core = CoreConfig::from_raw(&raw);
        let indentation = IndentationConfig::from_raw(&raw);
        let layout = LayoutConfig::from_raw(&raw);
        let rule_config = RuleConfigStore::from_raw(&raw);
        let templater_config = TemplaterConfigStore::from_raw(&raw);

        let dialect_kind = configured_dialect_kind_from_raw(&raw);
        let dialect_config = dialect_section_from_raw(&raw, dialect_kind);
        let dialect = kind_to_dialect(&dialect_kind, dialect_config)
            .expect("Dialect is disabled. Please enable the corresponding feature.");

        let templater_kind = raw["core"]["templater"]
            .as_string()
            .map(TemplaterKind::from_name)
            .transpose()
            .ok()
            .flatten()
            .unwrap_or(TemplaterKind::Raw);

        let sql_file_exts = raw["core"]["sql_file_exts"]
            .as_array()
            .unwrap_or_default()
            .iter()
            .filter_map(|it| it.as_string().map(ToOwned::to_owned))
            .collect();

        let reflow = ReflowConfig::from_config_sections(&core, &indentation, &layout);

        Self {
            raw,
            core,
            indentation,
            layout,
            rule_config,
            templater_config,
            dialect_kind,
            dialect,
            templater_kind,
            sql_file_exts,
            reflow,
        }
    }

    pub fn from_patch(patch: ConfigPatch) -> Self {
        Self::build_from_raw(patch.into())
    }

    pub fn with_patch(&self, patch: ConfigPatch) -> Self {
        let raw = nested_combine(self.raw.clone(), patch.into());
        Self::build_from_raw(raw)
    }

    pub fn builder() -> FluffConfigBuilder {
        FluffConfigBuilder {
            patch: ConfigPatch::default(),
        }
    }

    /// from_file creates a config object from a file path. The path is used both
    /// to read the file content and to resolve relative `_path`/`_dir` values.
    pub fn from_file(path: &Path) -> Result<FluffConfig, SqruffError> {
        ConfigLoader::new().load(ConfigLoadOptions {
            input: ConfigInput::File(path.to_path_buf()),
            ..Default::default()
        })
    }

    pub fn try_from_source(
        source: &str,
        optional_path_specification: Option<&Path>,
    ) -> Result<FluffConfig, SqruffError> {
        ConfigLoader::new().load(ConfigLoadOptions {
            input: ConfigInput::Source {
                text: source.to_string(),
                path: optional_path_specification.map(Path::to_path_buf),
            },
            ..Default::default()
        })
    }

    /// Loads a config object just based on the root directory.
    pub fn from_root(
        extra_config_path: Option<String>,
        ignore_local_config: bool,
        overrides: Option<ConfigOverrides>,
    ) -> Result<FluffConfig, SqruffError> {
        let input = extra_config_path
            .map(PathBuf::from)
            .map(ConfigInput::File)
            .unwrap_or_else(|| ConfigInput::ProjectRoot(".".into()));

        ConfigLoader::new().load(ConfigLoadOptions {
            input,
            ignore_local_config,
            overrides: overrides.unwrap_or_default(),
        })
    }

    pub fn core(&self) -> &CoreConfig {
        &self.core
    }

    pub fn indentation(&self) -> &IndentationConfig {
        &self.indentation
    }

    pub fn layout(&self) -> &LayoutConfig {
        &self.layout
    }

    pub fn dialect_kind(&self) -> DialectKind {
        self.dialect_kind
    }

    pub fn dialect(&self) -> &Dialect {
        &self.dialect
    }

    pub fn dialect_section(&self, dialect_kind: DialectKind) -> Option<&Value> {
        dialect_section_from_raw(&self.raw, dialect_kind)
    }

    pub fn templater_kind(&self) -> TemplaterKind {
        self.templater_kind
    }

    pub(crate) fn try_templater_kind(&self) -> Result<TemplaterKind, String> {
        self.raw["core"]["templater"]
            .as_string()
            .map(TemplaterKind::from_name)
            .transpose()
            .map(|templater| templater.unwrap_or(TemplaterKind::Raw))
    }

    pub fn sql_file_exts(&self) -> &[String] {
        self.sql_file_exts.as_ref()
    }

    pub fn no_color(&self) -> bool {
        self.core.no_color()
    }

    pub fn verbosity(&self) -> i32 {
        self.core.verbosity()
    }

    pub fn disable_noqa(&self) -> bool {
        self.core.disable_noqa()
    }

    pub fn max_line_length(&self) -> usize {
        self.core.max_line_length()
    }

    pub fn rule_allowlist(&self) -> Option<&[String]> {
        self.core.rule_allowlist()
    }

    pub fn rule_denylist(&self) -> &[String] {
        self.core.rule_denylist()
    }

    pub fn rule_config_map(&self, rule_config_ref: &str) -> HashMap<String, Value> {
        self.rule_config.config_map(rule_config_ref)
    }

    pub fn templater_section(&self, templater: TemplaterKind) -> Option<&HashMap<String, Value>> {
        self.templater_config.section(templater)
    }

    pub fn templater_value(&self, templater: TemplaterKind, key: &str) -> Option<&Value> {
        self.templater_config.value(templater, key)
    }

    pub fn templater_context(&self, templater: TemplaterKind) -> Option<&HashMap<String, Value>> {
        self.templater_value(templater, "context")
            .and_then(Value::as_map)
    }

    #[allow(dead_code)]
    pub(crate) fn templater_root_value(&self, key: &str) -> Option<&Value> {
        self.templater_config.root_value(key)
    }

    pub fn reflow(&self) -> &ReflowConfig {
        &self.reflow
    }

    pub(crate) fn verify_dialect_specified(&self) -> Option<SQLFluffUserError> {
        None
    }

    pub fn for_source<'a>(&'a self, source: &str) -> Result<Cow<'a, FluffConfig>, SqruffError> {
        if !source.contains("-- sqlfluff") && !source.contains("-- sqruff") {
            return Ok(Cow::Borrowed(self));
        }

        // Future implementation:
        // parse inline config into ConfigPatch,
        // apply to self,
        // return Cow::Owned(new_config).
        Ok(Cow::Borrowed(self))
    }
}

impl CoreConfig {
    fn from_raw(raw: &RawConfig) -> Self {
        let core = raw["core"].as_map().unwrap();

        Self {
            no_color: bool_value(core, "nocolor"),
            verbosity: int_value(core, "verbose"),
            disable_noqa: bool_value(core, "disable_noqa"),
            max_line_length: int_value(core, "max_line_length").max(0) as usize,
            rule_allowlist: string_list_value(core, "rule_allowlist"),
            rule_denylist: string_list_value(core, "rule_denylist").unwrap_or_default(),
        }
    }

    pub fn no_color(&self) -> bool {
        self.no_color
    }

    pub fn verbosity(&self) -> i32 {
        self.verbosity
    }

    pub fn disable_noqa(&self) -> bool {
        self.disable_noqa
    }

    pub fn max_line_length(&self) -> usize {
        self.max_line_length
    }

    pub fn rule_allowlist(&self) -> Option<&[String]> {
        self.rule_allowlist.as_deref()
    }

    pub fn rule_denylist(&self) -> &[String] {
        &self.rule_denylist
    }
}

impl IndentationConfig {
    fn from_raw(raw: &RawConfig) -> Self {
        Self {
            values: raw["indentation"].as_map().unwrap().clone(),
        }
    }

    pub(crate) fn value(&self, key: &str) -> &Value {
        self.values.get(key).unwrap_or(&Value::None)
    }

    pub fn indent_unit(&self) -> &str {
        self.value("indent_unit").as_string().unwrap_or("space")
    }

    pub fn tab_space_size(&self) -> usize {
        self.value("tab_space_size").as_int().unwrap_or(4) as usize
    }

    pub fn hanging_indents(&self) -> bool {
        self.value("hanging_indents").as_bool().unwrap_or_default()
    }

    pub fn allow_implicit_indents(&self) -> bool {
        self.value("allow_implicit_indents")
            .as_bool()
            .unwrap_or_default()
    }

    pub fn trailing_comments(&self) -> &str {
        self.value("trailing_comments")
            .as_string()
            .unwrap_or("before")
    }
}

impl LayoutConfig {
    fn from_raw(raw: &RawConfig) -> Self {
        Self {
            values: raw["layout"].as_map().unwrap().clone(),
        }
    }

    pub(crate) fn type_configs(&self) -> HashMap<String, Value> {
        self.values["type"].as_map().unwrap().clone()
    }
}

impl RuleConfigStore {
    fn from_raw(raw: &RawConfig) -> Self {
        Self {
            values: raw["rules"].as_map().unwrap().clone(),
        }
    }

    fn config_map(&self, rule_config_ref: &str) -> HashMap<String, Value> {
        if rule_config_ref.is_empty() || rule_config_ref == "rules" {
            return self.values.clone();
        }

        // Start with scalar values from the global [rules] section
        let mut merged: HashMap<String, Value> = self
            .values
            .iter()
            .filter(|(_, v)| !matches!(v, Value::Map(_)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Override/extend with rule-specific values
        if let Some(specific) = self.values.get(rule_config_ref).and_then(Value::as_map) {
            merged.extend(specific.clone());
        }

        merged
    }
}

impl TemplaterConfigStore {
    fn from_raw(raw: &RawConfig) -> Self {
        Self {
            values: raw["templater"].as_map().unwrap().clone(),
        }
    }

    #[allow(dead_code)]
    fn root_value(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    fn section(&self, templater: TemplaterKind) -> Option<&HashMap<String, Value>> {
        self.values.get(templater.as_str()).and_then(Value::as_map)
    }

    fn value(&self, templater: TemplaterKind, key: &str) -> Option<&Value> {
        self.section(templater)?.get(key)
    }
}

impl<'a> From<&'a FluffConfig> for Parser<'a> {
    fn from(config: &'a FluffConfig) -> Self {
        let dialect = config.dialect();
        let indentation_config = ParserIndentationConfig::from_bool_lookup(|key| {
            config.indentation.value(key).to_bool()
        });
        Self::new(dialect, indentation_config)
    }
}

fn configured_dialect_kind_from_raw(configs: &RawConfig) -> DialectKind {
    match configs
        .get("core")
        .and_then(|map| map.as_map().unwrap().get("dialect"))
    {
        None => DialectKind::default(),
        Some(Value::String(std)) => DialectKind::from_str(std).unwrap(),
        _value => DialectKind::default(),
    }
}

fn dialect_section_from_raw(configs: &RawConfig, dialect_kind: DialectKind) -> Option<&Value> {
    configs
        .get("dialect")
        .and_then(|v| v.as_map())
        .and_then(|m| m.get(dialect_kind.as_ref()))
}

fn normalize_core_lists(configs: &mut RawConfig) {
    for (in_key, out_key) in [
        ("ignore", "ignore"),
        ("warnings", "warnings"),
        ("rules", "rule_allowlist"),
        ("exclude_rules", "rule_denylist"),
    ] {
        match configs["core"].as_map().unwrap().get(in_key) {
            Some(value) if !value.is_none() => {
                let string = value.as_string().unwrap();
                let values = split_comma_separated_string(string);

                configs
                    .get_mut("core")
                    .unwrap()
                    .as_map_mut()
                    .unwrap()
                    .insert(out_key.into(), values);
            }
            _ => {}
        }
    }
}

fn nested_combine(mut a: RawConfig, b: RawConfig) -> RawConfig {
    for (key, value_b) in b {
        match (a.get(&key), value_b) {
            (Some(Value::Map(map_a)), Value::Map(map_b)) => {
                let combined = nested_combine(map_a.clone(), map_b);
                a.insert(key, Value::Map(combined));
            }
            (_, value) => {
                a.insert(key, value);
            }
        }
    }
    a
}

fn int_value(map: &HashMap<String, Value>, key: &str) -> i32 {
    map.get(key).and_then(Value::as_int).unwrap_or_default()
}

fn bool_value(map: &HashMap<String, Value>, key: &str) -> bool {
    map.get(key).and_then(Value::as_bool).unwrap_or_default()
}

fn string_list_value(map: &HashMap<String, Value>, key: &str) -> Option<Vec<String>> {
    map.get(key).and_then(Value::as_array).map(|values| {
        values
            .iter()
            .filter_map(|value| value.as_string().map(ToOwned::to_owned))
            .collect()
    })
}

/// Builder for [`FluffConfig`].
///
/// Obtain via [`FluffConfig::builder()`].
pub struct FluffConfigBuilder {
    patch: ConfigPatch,
}

impl FluffConfigBuilder {
    /// Apply a [`ConfigPatch`] to the builder.
    pub fn patch(mut self, patch: ConfigPatch) -> Self {
        self.patch = patch;
        self
    }

    /// Build the [`FluffConfig`], returning an error if the config is invalid
    /// (e.g. the requested templater is not supported in this build).
    pub fn build(self) -> Result<FluffConfig, SqruffError> {
        let config = FluffConfig::from_patch(self.patch);
        config.try_templater_kind().map_err(SqruffError::Config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::SqruffError;
    use sqruff_lib_core::dialects::init::DialectKind;

    #[test]
    fn test_dialect_config_section_parsing() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = snowflake

[sqruff:dialect:snowflake]
some_option = value
"#,
            None,
        )
        .unwrap();

        let dialect_section = config.dialect_section(DialectKind::Snowflake).unwrap();
        let snowflake_map = dialect_section.as_map().unwrap();
        assert_eq!(
            snowflake_map.get("some_option").unwrap().as_string(),
            Some("value")
        );
    }

    #[test]
    fn test_dialect_config_empty_section() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = bigquery

[sqruff:dialect:bigquery]
"#,
            None,
        )
        .unwrap();

        assert_eq!(config.dialect().name, DialectKind::Bigquery);
    }

    #[test]
    fn test_dialect_without_config_section() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = postgres
"#,
            None,
        )
        .unwrap();

        assert_eq!(config.dialect().name, DialectKind::Postgres);
    }

    #[test]
    fn config_loader_applies_typed_overrides() {
        let config = ConfigLoader::new()
            .load(ConfigLoadOptions {
                input: ConfigInput::Source {
                    text: "[sqruff]\ndialect = ansi\nrules = AL01\nexclude_rules = LT01\n".into(),
                    path: None,
                },
                overrides: ConfigOverrides {
                    dialect: Some(DialectKind::Postgres),
                    rules: Some(vec!["AL02".into(), "LT02".into()]),
                    exclude_rules: Some(vec!["CP01".into()]),
                },
                ..Default::default()
            })
            .unwrap();

        assert_eq!(config.dialect_kind(), DialectKind::Postgres);
        assert_eq!(
            config.rule_allowlist().unwrap(),
            &["AL02".to_string(), "LT02".to_string()]
        );
        assert_eq!(config.rule_denylist(), &["CP01".to_string()]);
    }

    #[test]
    fn test_templater_kind_defaults_to_raw() {
        let config = FluffConfig::try_from_source("", None).unwrap();
        assert_eq!(config.templater_kind(), TemplaterKind::Raw);
    }

    #[test]
    fn test_templater_kind_parses_placeholder() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
templater = placeholder
"#,
            None,
        )
        .unwrap();

        assert_eq!(config.templater_kind(), TemplaterKind::Placeholder);
    }

    #[test]
    fn test_templater_section_uses_typed_kind() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
templater = placeholder

[sqruff:templater:placeholder]
param_style = colon
"#,
            None,
        )
        .unwrap();

        let section = config
            .templater_section(TemplaterKind::Placeholder)
            .unwrap();
        assert_eq!(
            section.get("param_style").unwrap().as_string(),
            Some("colon")
        );
    }

    #[cfg(feature = "python")]
    #[test]
    fn test_templater_context_uses_typed_kind() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
templater = python

[sqruff:templater:python:context]
blah = foo
"#,
            None,
        )
        .unwrap();

        let context = config.templater_context(TemplaterKind::Python).unwrap();
        assert_eq!(context.get("blah").unwrap().as_string(), Some("foo"));
    }

    #[test]
    fn try_from_source_returns_config_error_for_invalid_path_value() {
        let err = FluffConfig::try_from_source(
            r#"
[sqruff]
templater = dbt

[sqruff:templater:dbt]
project_dir = 1
"#,
            None,
        )
        .unwrap_err();

        assert!(matches!(err, SqruffError::Config(_)));
        assert!(err.to_string().contains("invalid path value"));
    }
}
