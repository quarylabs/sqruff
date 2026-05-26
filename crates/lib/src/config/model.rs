use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use hashbrown::HashMap;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::{IndentationConfig as ParserIndentationConfig, Parser};
use sqruff_lib_dialects::kind_to_dialect;

use super::error::ConfigError;
use super::layout::LayoutConfig;
use super::loader::ConfigLoader;
use super::options::{ConfigInput, ConfigLoadOptions, ConfigOverrides};
use super::patch::ConfigPatch;
use super::raw::{RawConfig, Value, merge_configs, split_comma_separated_string};
use super::rules::RuleConfigs;
use super::templater::TemplaterConfig;
use crate::api::SqruffError;
use crate::templaters::TemplaterKind;
use crate::utils::reflow::config::ReflowConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct FluffConfig {
    /// Kept for internal operations that still require the raw map
    /// (e.g. `with_patch`, `for_source`).
    raw: RawConfig,

    core: CoreConfig,
    indentation: IndentationConfig,
    layout: LayoutConfig,
    templater: TemplaterConfig,
    rules: RuleConfigs,
    dialects: DialectConfigStore,

    dialect_kind: DialectKind,
    dialect: Dialect,
    reflow: ReflowConfig,
}

// ── DialectConfigStore ───────────────────────────────────────────────────────

/// Resolved per-dialect configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct DialectConfigStore {
    values: HashMap<String, Value>,
}

impl DialectConfigStore {
    fn from_raw(raw: &RawConfig) -> Self {
        Self {
            values: raw
                .get("dialect")
                .and_then(Value::as_map)
                .cloned()
                .unwrap_or_default(),
        }
    }

    pub fn section(&self, dialect_kind: DialectKind) -> Option<&Value> {
        self.values.get(dialect_kind.as_ref())
    }
}

// ── CoreConfig ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CoreConfig {
    pub verbose: u8,
    pub no_color: bool,
    pub dialect: Option<DialectKind>,
    pub templater: TemplaterKind,
    pub rule_allowlist: Option<Vec<RuleSelector>>,
    pub rule_denylist: Vec<RuleSelector>,
    pub output_line_length: usize,
    pub runaway_limit: usize,
    pub ignore: Vec<ErrorCategory>,
    pub warnings: Vec<WarningSelector>,
    pub warn_unused_ignores: bool,
    pub ignore_templated_areas: bool,
    pub encoding: EncodingMode,
    pub disable_noqa: bool,
    pub sql_file_exts: Vec<String>,
    pub fix_even_unparsable: bool,
    pub large_file_skip_char_limit: usize,
    pub large_file_skip_byte_limit: usize,
    pub max_line_length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingMode {
    Autodetect,
    Utf8,
    Utf8Sig,
    Other,
}

impl EncodingMode {
    fn from_name(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "autodetect" => Self::Autodetect,
            "utf-8" => Self::Utf8,
            "utf-8-sig" => Self::Utf8Sig,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSelector(String);

impl RuleSelector {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RuleSelector {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningSelector(String);

impl WarningSelector {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WarningSelector {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Lexing,
    Linting,
    Parsing,
    Templating,
}

impl ErrorCategory {
    fn from_name(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "lexing" => Ok(Self::Lexing),
            "linting" => Ok(Self::Linting),
            "parsing" => Ok(Self::Parsing),
            "templating" => Ok(Self::Templating),
            _ => Err(format!("unknown error category '{value}'")),
        }
    }
}

// ── IndentationConfig ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct IndentationConfig {
    values: HashMap<String, Value>,
}

// ── Backward-compatible type aliases ─────────────────────────────────────────

/// Deprecated alias — prefer [`RuleConfigs`].
#[deprecated(note = "Use `RuleConfigs` instead")]
pub type RuleConfigStore = RuleConfigs;

/// Deprecated alias — prefer [`TemplaterConfig`].
#[deprecated(note = "Use `TemplaterConfig` instead")]
pub type TemplaterConfigStore = TemplaterConfig;

impl Default for FluffConfig {
    fn default() -> Self {
        Self::from_patch(ConfigPatch::default())
    }
}

impl FluffConfig {
    pub(crate) fn try_build_from_raw(configs: RawConfig) -> Result<Self, SqruffError> {
        let defaults: RawConfig =
            ConfigLoader::try_from_source(include_str!("./default_config.cfg"), None)
                .expect("built-in default config must be valid")
                .into();

        let mut raw = merge_configs(defaults, configs);
        normalize_core_lists(&mut raw);

        let core = CoreConfig::from_raw(&raw);
        let indentation = IndentationConfig::from_raw(&raw);
        let layout = LayoutConfig::from_raw(&raw).map_err(config_error)?;
        let rules = RuleConfigs::from_raw(&raw);
        let templater = TemplaterConfig::from_raw(&raw);
        let dialects = DialectConfigStore::from_raw(&raw);

        let dialect_kind = core.dialect.unwrap_or_default();
        let dialect_config = dialects.section(dialect_kind);
        let dialect = kind_to_dialect(&dialect_kind, dialect_config)
            .expect("Dialect is disabled. Please enable the corresponding feature.");

        let reflow =
            ReflowConfig::from_config_parts(&layout, &indentation, &core).map_err(config_error)?;

        Ok(Self {
            raw,
            core,
            indentation,
            layout,
            rules,
            templater,
            dialects,
            dialect_kind,
            dialect,
            reflow,
        })
    }

    pub(crate) fn build_from_raw(configs: RawConfig) -> Self {
        Self::try_build_from_raw(configs).expect("config must be valid")
    }

    pub fn from_patch(patch: ConfigPatch) -> Self {
        Self::build_from_raw(patch.into())
    }

    pub fn try_from_patch(patch: ConfigPatch) -> Result<Self, SqruffError> {
        Self::try_build_from_raw(patch.into())
    }

    pub fn with_patch(&self, patch: ConfigPatch) -> Self {
        let raw = merge_configs(self.raw.clone(), patch.into());
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
        self.dialects.section(dialect_kind)
    }

    pub fn templater_kind(&self) -> TemplaterKind {
        self.templater.kind()
    }

    pub(crate) fn try_templater_kind(&self) -> Result<TemplaterKind, String> {
        Ok(self.templater.kind())
    }

    pub fn sql_file_exts(&self) -> &[String] {
        self.core.sql_file_exts()
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

    pub fn ignore_templated_areas(&self) -> bool {
        self.core.ignore_templated_areas
    }

    pub fn output_line_length(&self) -> usize {
        self.core.output_line_length
    }

    pub fn runaway_limit(&self) -> usize {
        self.core.runaway_limit
    }

    pub fn max_line_length(&self) -> usize {
        self.core.max_line_length()
    }

    pub fn fix_even_unparsable(&self) -> bool {
        self.core.fix_even_unparsable
    }

    pub fn rule_allowlist(&self) -> Option<&[RuleSelector]> {
        self.core.rule_allowlist()
    }

    pub fn rule_denylist(&self) -> &[RuleSelector] {
        self.core.rule_denylist()
    }

    pub fn rule_config_map(&self, rule_config_ref: &str) -> HashMap<String, Value> {
        self.rules.config_map(rule_config_ref)
    }

    pub fn templater_section(&self, templater: TemplaterKind) -> Option<&HashMap<String, Value>> {
        self.templater.section(templater)
    }

    pub fn templater_value(&self, templater: TemplaterKind, key: &str) -> Option<&Value> {
        self.templater.value(templater, key)
    }

    pub fn templater_context(&self, templater: TemplaterKind) -> Option<&HashMap<String, Value>> {
        self.templater_value(templater, "context")
            .and_then(Value::as_map)
    }

    #[cfg(feature = "python")]
    pub(crate) fn templater_root_value(&self, key: &str) -> Option<&Value> {
        self.templater.root_value(key)
    }

    pub fn reflow(&self) -> &ReflowConfig {
        &self.reflow
    }

    pub fn for_source<'a>(&'a self, source: &str) -> Result<Cow<'a, FluffConfig>, SqruffError> {
        if !source.contains("-- sqlfluff") && !source.contains("-- sqruff") {
            return Ok(Cow::Borrowed(self));
        }

        let mut patch = ConfigPatch::default();
        let mut found = false;

        for line in source.lines() {
            let Some(directive) = extract_inline_config_directive(line) else {
                continue;
            };

            let Some((key, raw_value)) = directive.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let raw_value = raw_value.trim();

            // Keys without a section prefix go under `core` (e.g. `dialect`).
            // Keys with `:` are treated as a nested path (e.g. `rules:LT01:comma_style`).
            let path: Vec<&str> = if key.contains(':') {
                key.split(':').collect()
            } else {
                vec!["core", key]
            };

            let value: Value = raw_value
                .parse()
                .unwrap_or_else(|_| Value::String(raw_value.into()));
            patch.set_value(&path, value);
            found = true;
        }

        if found {
            Ok(Cow::Owned(self.with_patch(patch)))
        } else {
            Ok(Cow::Borrowed(self))
        }
    }
}

/// Scan a single SQL line for an inline sqruff/sqlfluff config directive of the
/// form `-- sqruff:set key=value` (or `-- sqlfluff:set …`).  Returns the
/// `key=value` substring (with surrounding whitespace stripped), or `None`
/// when the line contains no directive.
fn extract_inline_config_directive(line: &str) -> Option<&str> {
    for prefix in &["-- sqlfluff:set", "-- sqruff:set"] {
        if let Some(pos) = line.find(prefix) {
            let rest = line[pos + prefix.len()..].trim();
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

impl CoreConfig {
    fn from_raw(raw: &RawConfig) -> Self {
        let core = raw["core"].as_map().unwrap();

        Self {
            verbose: u8_value(core, "verbose"),
            no_color: bool_value(core, "nocolor"),
            dialect: dialect_value(core, "dialect"),
            templater: templater_value(core, "templater"),
            rule_allowlist: rule_selector_list_value(core, "rule_allowlist"),
            rule_denylist: rule_selector_list_value(core, "rule_denylist").unwrap_or_default(),
            output_line_length: usize_value(core, "output_line_length"),
            runaway_limit: usize_value(core, "runaway_limit"),
            ignore: error_category_list_value(core, "ignore").unwrap_or_default(),
            warnings: warning_selector_list_value(core, "warnings").unwrap_or_default(),
            warn_unused_ignores: bool_value(core, "warn_unused_ignores"),
            ignore_templated_areas: bool_value(core, "ignore_templated_areas"),
            encoding: encoding_value(core, "encoding"),
            disable_noqa: bool_value(core, "disable_noqa"),
            sql_file_exts: string_vec_value(core, "sql_file_exts"),
            fix_even_unparsable: bool_value(core, "fix_even_unparsable"),
            large_file_skip_char_limit: usize_value(core, "large_file_skip_char_limit"),
            large_file_skip_byte_limit: usize_value(core, "large_file_skip_byte_limit"),
            max_line_length: usize_value(core, "max_line_length"),
        }
    }

    pub fn no_color(&self) -> bool {
        self.no_color
    }

    pub fn verbosity(&self) -> i32 {
        i32::from(self.verbose)
    }

    pub fn disable_noqa(&self) -> bool {
        self.disable_noqa
    }

    pub fn max_line_length(&self) -> usize {
        self.max_line_length
    }

    pub fn rule_allowlist(&self) -> Option<&[RuleSelector]> {
        self.rule_allowlist.as_deref()
    }

    pub fn rule_denylist(&self) -> &[RuleSelector] {
        &self.rule_denylist
    }

    pub fn sql_file_exts(&self) -> &[String] {
        &self.sql_file_exts
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

impl<'a> From<&'a FluffConfig> for Parser<'a> {
    fn from(config: &'a FluffConfig) -> Self {
        let dialect = config.dialect();
        let indentation_config = ParserIndentationConfig::from_bool_lookup(|key| {
            config.indentation.value(key).to_bool()
        });
        Self::new(dialect, indentation_config)
    }
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
            Some(_) => {
                configs
                    .get_mut("core")
                    .unwrap()
                    .as_map_mut()
                    .unwrap()
                    .insert(out_key.into(), Value::None);
            }
            _ => {}
        }
    }
}

fn config_error(err: ConfigError) -> SqruffError {
    SqruffError::Config(err.to_string())
}

fn int_value(map: &HashMap<String, Value>, key: &str) -> i32 {
    map.get(key).and_then(Value::as_int).unwrap_or_default()
}

fn u8_value(map: &HashMap<String, Value>, key: &str) -> u8 {
    int_value(map, key).clamp(0, i32::from(u8::MAX)) as u8
}

fn usize_value(map: &HashMap<String, Value>, key: &str) -> usize {
    int_value(map, key).max(0) as usize
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

fn string_vec_value(map: &HashMap<String, Value>, key: &str) -> Vec<String> {
    string_list_value(map, key).unwrap_or_default()
}

fn dialect_value(map: &HashMap<String, Value>, key: &str) -> Option<DialectKind> {
    map.get(key)
        .and_then(Value::as_string)
        .map(DialectKind::from_str)
        .transpose()
        .unwrap()
}

fn templater_value(map: &HashMap<String, Value>, key: &str) -> TemplaterKind {
    map.get(key)
        .and_then(Value::as_string)
        .map(TemplaterKind::from_name)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or(TemplaterKind::Raw)
}

fn rule_selector_list_value(map: &HashMap<String, Value>, key: &str) -> Option<Vec<RuleSelector>> {
    string_list_value(map, key).map(|values| values.into_iter().map(RuleSelector::from).collect())
}

fn warning_selector_list_value(
    map: &HashMap<String, Value>,
    key: &str,
) -> Option<Vec<WarningSelector>> {
    string_list_value(map, key)
        .map(|values| values.into_iter().map(WarningSelector::from).collect())
}

fn error_category_list_value(
    map: &HashMap<String, Value>,
    key: &str,
) -> Option<Vec<ErrorCategory>> {
    string_list_value(map, key).map(|values| {
        values
            .into_iter()
            .map(|value| ErrorCategory::from_name(&value).unwrap())
            .collect()
    })
}

fn encoding_value(map: &HashMap<String, Value>, key: &str) -> EncodingMode {
    map.get(key)
        .and_then(Value::as_string)
        .map(EncodingMode::from_name)
        .unwrap_or(EncodingMode::Autodetect)
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
        let config = FluffConfig::try_from_patch(self.patch)?;
        config.try_templater_kind().map_err(SqruffError::Config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::SqruffError;
    use crate::templaters::TemplaterKind;
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
            config
                .rule_allowlist()
                .unwrap()
                .iter()
                .map(RuleSelector::as_str)
                .collect::<Vec<_>>(),
            vec!["AL02", "LT02"]
        );
        assert_eq!(
            config
                .rule_denylist()
                .iter()
                .map(RuleSelector::as_str)
                .collect::<Vec<_>>(),
            vec!["CP01"]
        );
    }

    #[test]
    fn core_config_resolves_typed_values() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
verbose = 2
nocolor = True
dialect = postgres
templater = placeholder
rules = AL01,LT02
exclude_rules = CP01
output_line_length = 120
runaway_limit = 20
ignore = lexing,parsing
warnings = LT01,TMP
warn_unused_ignores = True
ignore_templated_areas = False
encoding = utf-8-sig
disable_noqa = True
sql_file_exts = .sql,.ddl
fix_even_unparsable = True
large_file_skip_char_limit = 100
large_file_skip_byte_limit = 200
max_line_length = 90
"#,
            None,
        )
        .unwrap();

        let core = config.core();
        assert_eq!(core.verbose, 2);
        assert!(core.no_color);
        assert_eq!(core.dialect, Some(DialectKind::Postgres));
        assert_eq!(core.templater, TemplaterKind::Placeholder);
        assert_eq!(
            core.rule_allowlist
                .as_deref()
                .unwrap()
                .iter()
                .map(RuleSelector::as_str)
                .collect::<Vec<_>>(),
            vec!["AL01", "LT02"]
        );
        assert_eq!(
            core.rule_denylist
                .iter()
                .map(RuleSelector::as_str)
                .collect::<Vec<_>>(),
            vec!["CP01"]
        );
        assert_eq!(core.output_line_length, 120);
        assert_eq!(core.runaway_limit, 20);
        assert_eq!(
            core.ignore,
            vec![ErrorCategory::Lexing, ErrorCategory::Parsing]
        );
        assert_eq!(
            core.warnings
                .iter()
                .map(WarningSelector::as_str)
                .collect::<Vec<_>>(),
            vec!["LT01", "TMP"]
        );
        assert!(core.warn_unused_ignores);
        assert!(!core.ignore_templated_areas);
        assert_eq!(core.encoding, EncodingMode::Utf8Sig);
        assert!(core.disable_noqa);
        assert_eq!(core.sql_file_exts, vec![".sql", ".ddl"]);
        assert!(core.fix_even_unparsable);
        assert_eq!(core.large_file_skip_char_limit, 100);
        assert_eq!(core.large_file_skip_byte_limit, 200);
        assert_eq!(core.max_line_length, 90);
    }

    #[test]
    fn typed_patch_absent_values_do_not_override_existing_config() {
        let base = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = postgres
rules = AL01
"#,
            None,
        )
        .unwrap();

        let patch: ConfigPatch = serde_json::from_value(serde_json::json!({
            "core": {}
        }))
        .unwrap();
        let config = base.with_patch(patch);

        assert_eq!(config.dialect_kind(), DialectKind::Postgres);
        assert_eq!(
            config
                .rule_allowlist()
                .unwrap()
                .iter()
                .map(RuleSelector::as_str)
                .collect::<Vec<_>>(),
            vec!["AL01"]
        );
    }

    #[test]
    fn typed_patch_null_values_clear_existing_config() {
        let base = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = postgres
rules = AL01
exclude_rules = LT01
"#,
            None,
        )
        .unwrap();

        let patch: ConfigPatch = serde_json::from_value(serde_json::json!({
            "core": {
                "dialect": null,
                "rules": null,
                "exclude_rules": null
            }
        }))
        .unwrap();
        let config = base.with_patch(patch);

        assert_eq!(config.dialect_kind(), DialectKind::default());
        assert_eq!(config.rule_allowlist(), None);
        assert!(config.rule_denylist().is_empty());
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

    #[test]
    fn try_from_source_rejects_unknown_section() {
        let err = FluffConfig::try_from_source(
            r#"
[sqruff:unknown]
value = true
"#,
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown config section"));
    }

    #[test]
    fn try_from_source_rejects_unknown_core_key() {
        let err = FluffConfig::try_from_source(
            r#"
[sqruff]
not_a_real_key = true
"#,
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn try_from_source_rejects_invalid_bool() {
        let err = FluffConfig::try_from_source(
            r#"
[sqruff]
nocolor = maybe
"#,
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("invalid bool"));
    }

    #[test]
    fn try_from_source_rejects_invalid_layout_syntax_kind() {
        let err = FluffConfig::try_from_source(
            r#"
[sqruff:layout:type:not_a_segment]
spacing_before = touch
"#,
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("invalid layout syntax kind"));
    }

    #[test]
    fn try_from_source_rejects_invalid_rule_option() {
        let err = FluffConfig::try_from_source(
            r#"
[sqruff:rules:layout.long_lines]
not_a_rule_option = true
"#,
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("invalid rule option"));
    }
}
