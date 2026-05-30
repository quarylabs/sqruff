use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::{IndentationConfig as ParserIndentationConfig, Parser};
use sqruff_lib_dialects::{DialectConfigs, kind_to_dialect};

use super::error::ConfigError;
use super::layout::LayoutConfig;
use super::loader::ConfigLoader;
use super::options::{ConfigInput, ConfigLoadOptions, ConfigOverrides};
use super::patch::ConfigPatch;
use super::rules::RuleConfigs;
use super::setting::Merge;
use super::templater::TemplaterConfig;
use crate::api::SqruffError;
use crate::templaters::TemplaterKind;
use crate::utils::reflow::config::ReflowConfig;

#[derive(Debug, Clone)]
pub struct FluffConfig {
    patch: ConfigPatch,

    core: CoreConfig,
    indentation: IndentationConfig,
    layout: LayoutConfig,
    templater: TemplaterConfig,
    rules: RuleConfigs,
    dialects: DialectConfigs,

    dialect_kind: DialectKind,
    dialect: Dialect,
    reflow: ReflowConfig,
}

pub type DialectConfigStore = DialectConfigs;

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

impl FromStr for EncodingMode {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_name(value))
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

impl FromStr for ErrorCategory {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_name(value)
    }
}

// ── IndentationConfig ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct IndentationConfig {
    pub indent_unit: String,
    pub tab_space_size: usize,
    pub indented_joins: bool,
    pub indented_ctes: bool,
    pub indented_using_on: bool,
    pub indented_on_contents: bool,
    pub indented_then: bool,
    pub indented_then_contents: bool,
    pub indented_joins_on: bool,
    pub hanging_indents: bool,
    pub allow_implicit_indents: bool,
    pub template_blocks_indent: bool,
    pub skip_indentation_in: Vec<String>,
    pub trailing_comments: String,
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
    pub(crate) fn try_build_from_patch(configs: ConfigPatch) -> Result<Self, SqruffError> {
        let mut patch = ConfigLoader::try_from_source(include_str!("./default_config.cfg"), None)
            .expect("built-in default config must be valid");
        patch.merge(configs);

        let core = CoreConfig::from_patch(&patch).map_err(config_error)?;
        let indentation = IndentationConfig::from_patch(&patch);
        let layout = LayoutConfig::from_patch(&patch.layout).map_err(config_error)?;
        let rules = RuleConfigs::from_patch(&patch.rules).map_err(config_error)?;
        let templater =
            TemplaterConfig::from_patch(core.templater, &patch.templater).map_err(config_error)?;
        let dialects = patch.dialects.resolve();

        let dialect_kind = core.dialect.unwrap_or_default();
        let dialect = kind_to_dialect(&dialect_kind, &dialects)
            .expect("Dialect is disabled. Please enable the corresponding feature.");

        let reflow =
            ReflowConfig::from_config_parts(&layout, &indentation, &core).map_err(config_error)?;

        Ok(Self {
            patch,
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

    pub(crate) fn build_from_patch(patch: ConfigPatch) -> Self {
        Self::try_build_from_patch(patch).expect("config must be valid")
    }

    pub fn from_patch(patch: ConfigPatch) -> Self {
        Self::build_from_patch(patch)
    }

    pub fn try_from_patch(patch: ConfigPatch) -> Result<Self, SqruffError> {
        Self::try_build_from_patch(patch)
    }

    pub fn with_patch(&self, patch: ConfigPatch) -> Self {
        let mut merged = self.patch.clone();
        merged.merge(patch);
        Self::build_from_patch(merged)
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

    pub fn dialect_configs(&self) -> &DialectConfigs {
        &self.dialects
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

    pub fn rules(&self) -> &RuleConfigs {
        &self.rules
    }

    pub fn templater(&self) -> &TemplaterConfig {
        &self.templater
    }

    #[cfg(feature = "python")]
    pub fn templater_context(
        &self,
        templater: TemplaterKind,
    ) -> Option<&hashbrown::HashMap<String, String>> {
        self.templater.context(templater)
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

            patch.set_inline(&path, raw_value).map_err(config_error)?;
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
    fn from_patch(patch: &ConfigPatch) -> Result<Self, ConfigError> {
        let core = &patch.core;
        Ok(Self {
            verbose: core.verbose.clone().into_option().unwrap_or_default(),
            no_color: core.nocolor.clone().into_option().unwrap_or_default(),
            dialect: core
                .dialect
                .clone()
                .into_option()
                .flatten()
                .map(|value| {
                    DialectKind::from_str(&value)
                        .map_err(|_| ConfigError::UnknownDialect(value.to_string()))
                })
                .transpose()?,
            templater: core
                .templater
                .clone()
                .into_option()
                .map(|value| TemplaterKind::from_name(&value))
                .transpose()
                .map_err(ConfigError::UnsupportedTemplater)?
                .unwrap_or(TemplaterKind::Raw),
            rule_allowlist: core
                .rules
                .clone()
                .into_option()
                .flatten()
                .map(|values| values.into_iter().map(RuleSelector::from).collect()),
            rule_denylist: core
                .exclude_rules
                .clone()
                .into_option()
                .flatten()
                .unwrap_or_default()
                .into_iter()
                .map(RuleSelector::from)
                .collect(),
            output_line_length: core
                .output_line_length
                .clone()
                .into_option()
                .unwrap_or_default(),
            runaway_limit: core.runaway_limit.clone().into_option().unwrap_or_default(),
            ignore: core
                .ignore
                .clone()
                .into_option()
                .flatten()
                .unwrap_or_default()
                .into_iter()
                .map(|value| {
                    value.parse().map_err(|err| ConfigError::InvalidField {
                        field: "ignore",
                        reason: err,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            warnings: core
                .warnings
                .clone()
                .into_option()
                .flatten()
                .unwrap_or_default()
                .into_iter()
                .map(WarningSelector::from)
                .collect(),
            warn_unused_ignores: core
                .warn_unused_ignores
                .clone()
                .into_option()
                .unwrap_or_default(),
            ignore_templated_areas: core
                .ignore_templated_areas
                .clone()
                .into_option()
                .unwrap_or_default(),
            encoding: core
                .encoding
                .clone()
                .into_option()
                .as_deref()
                .map(EncodingMode::from_name)
                .unwrap_or(EncodingMode::Autodetect),
            disable_noqa: core.disable_noqa.clone().into_option().unwrap_or_default(),
            sql_file_exts: core.sql_file_exts.clone().into_option().unwrap_or_default(),
            fix_even_unparsable: core
                .fix_even_unparsable
                .clone()
                .into_option()
                .unwrap_or_default(),
            large_file_skip_char_limit: core
                .large_file_skip_char_limit
                .clone()
                .into_option()
                .unwrap_or_default(),
            large_file_skip_byte_limit: core
                .large_file_skip_byte_limit
                .clone()
                .into_option()
                .unwrap_or_default(),
            max_line_length: core
                .max_line_length
                .clone()
                .into_option()
                .unwrap_or_default(),
        })
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
    fn from_patch(patch: &ConfigPatch) -> Self {
        let indentation = &patch.indentation;
        Self {
            indent_unit: indentation
                .indent_unit
                .clone()
                .into_option()
                .unwrap_or_else(|| "space".to_string()),
            tab_space_size: indentation
                .tab_space_size
                .clone()
                .into_option()
                .unwrap_or(4),
            indented_joins: indentation
                .indented_joins
                .clone()
                .into_option()
                .unwrap_or(false),
            indented_ctes: indentation
                .indented_ctes
                .clone()
                .into_option()
                .unwrap_or(false),
            indented_using_on: indentation
                .indented_using_on
                .clone()
                .into_option()
                .unwrap_or(false),
            indented_on_contents: indentation
                .indented_on_contents
                .clone()
                .into_option()
                .unwrap_or(false),
            indented_then: indentation
                .indented_then
                .clone()
                .into_option()
                .unwrap_or(false),
            indented_then_contents: indentation
                .indented_then_contents
                .clone()
                .into_option()
                .unwrap_or(false),
            indented_joins_on: indentation
                .indented_joins_on
                .clone()
                .into_option()
                .unwrap_or(false),
            hanging_indents: indentation
                .hanging_indents
                .clone()
                .into_option()
                .unwrap_or(false),
            allow_implicit_indents: indentation
                .allow_implicit_indents
                .clone()
                .into_option()
                .unwrap_or(false),
            template_blocks_indent: indentation
                .template_blocks_indent
                .clone()
                .into_option()
                .unwrap_or(false),
            skip_indentation_in: indentation
                .skip_indentation_in
                .clone()
                .into_option()
                .unwrap_or_default(),
            trailing_comments: indentation
                .trailing_comments
                .clone()
                .into_option()
                .unwrap_or_else(|| "before".to_string()),
        }
    }

    pub(crate) fn bool_value(&self, key: &str) -> bool {
        match key {
            "indented_joins" => self.indented_joins,
            "indented_ctes" => self.indented_ctes,
            "indented_using_on" => self.indented_using_on,
            "indented_on_contents" => self.indented_on_contents,
            "indented_then" => self.indented_then,
            "indented_then_contents" => self.indented_then_contents,
            "indented_joins_on" => self.indented_joins_on,
            _ => false,
        }
    }

    pub fn indent_unit(&self) -> &str {
        &self.indent_unit
    }

    pub fn tab_space_size(&self) -> usize {
        self.tab_space_size
    }

    pub fn hanging_indents(&self) -> bool {
        self.hanging_indents
    }

    pub fn allow_implicit_indents(&self) -> bool {
        self.allow_implicit_indents
    }

    pub fn trailing_comments(&self) -> &str {
        &self.trailing_comments
    }
}

impl<'a> From<&'a FluffConfig> for Parser<'a> {
    fn from(config: &'a FluffConfig) -> Self {
        let dialect = config.dialect();
        let indentation_config =
            ParserIndentationConfig::from_bool_lookup(|key| config.indentation.bool_value(key));
        Self::new(dialect, indentation_config)
    }
}

fn config_error(err: ConfigError) -> SqruffError {
    SqruffError::Config(err.to_string())
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
dialect = postgres

[sqruff:dialect:postgres]
pgvector = true
"#,
            None,
        )
        .unwrap();

        assert!(config.dialect_configs().postgres.pgvector);
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
    fn test_templater_config_uses_typed_fields() {
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

        assert_eq!(
            config.templater().placeholder.param_style,
            Some(crate::templaters::PlaceholderStyle::Colon)
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
        assert_eq!(context.get("blah").unwrap(), "foo");
    }

    #[test]
    fn try_from_source_returns_config_error_for_invalid_path_value() {
        let err = FluffConfig::try_from_source(
            r#"
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
    fn try_from_source_rejects_invalid_ignore_category() {
        let err = FluffConfig::try_from_source(
            r#"
[sqruff]
ignore = parsing,not_a_category
"#,
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown error category"));
    }

    #[test]
    fn unknown_encoding_resolves_to_other_for_compatibility() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
encoding = made-up-encoding
"#,
            None,
        )
        .unwrap();

        assert_eq!(config.core().encoding, EncodingMode::Other);
    }

    #[test]
    fn core_numeric_values_are_clamped_like_legacy_config() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
verbose = 999
max_line_length = -1
output_line_length = -1
runaway_limit = -1
large_file_skip_char_limit = -1
large_file_skip_byte_limit = -1
"#,
            None,
        )
        .unwrap();

        assert_eq!(config.core().verbose, u8::MAX);
        assert_eq!(config.core().max_line_length, 0);
        assert_eq!(config.core().output_line_length, 0);
        assert_eq!(config.core().runaway_limit, 0);
        assert_eq!(config.core().large_file_skip_char_limit, 0);
        assert_eq!(config.core().large_file_skip_byte_limit, 0);
    }

    #[test]
    fn core_none_values_clear_nullable_config() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = postgres
rules = None
exclude_rules = None
"#,
            None,
        )
        .unwrap();

        assert_eq!(config.dialect_kind(), DialectKind::Postgres);
        assert_eq!(config.rule_allowlist(), None);
        assert!(config.rule_denylist().is_empty());
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
    fn layout_type_alias_is_parsed_once_into_typed_key() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff:layout:type:aggregate_order_by]
line_position = leading
"#,
            None,
        )
        .unwrap();

        assert!(
            config.layout().types.contains_key(
                &sqruff_lib_core::dialects::syntax::SyntaxKind::AggregateOrderByClause
            )
        );
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

    #[test]
    fn sqruff_and_sqlfluff_section_prefixes_parse_identically() {
        let sqruff = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = postgres
rules = AL01

[sqruff:indentation]
tab_space_size = 2

[sqruff:rules:aliasing.table]
aliasing = implicit
"#,
            None,
        )
        .unwrap();

        let sqlfluff = FluffConfig::try_from_source(
            r#"
[sqlfluff]
dialect = postgres
rules = AL01

[sqlfluff:indentation]
tab_space_size = 2

[sqlfluff:rules:aliasing.table]
aliasing = implicit
"#,
            None,
        )
        .unwrap();

        assert_eq!(sqruff.dialect_kind(), sqlfluff.dialect_kind());
        assert_eq!(
            sqruff
                .rule_allowlist()
                .unwrap()
                .iter()
                .map(RuleSelector::as_str)
                .collect::<Vec<_>>(),
            sqlfluff
                .rule_allowlist()
                .unwrap()
                .iter()
                .map(RuleSelector::as_str)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            sqruff.indentation().tab_space_size(),
            sqlfluff.indentation().tab_space_size()
        );
        assert_eq!(
            sqruff.rules().aliasing.table.aliasing,
            sqlfluff.rules().aliasing.table.aliasing
        );
    }

    #[test]
    fn toml_config_normalizes_to_typed_patch_sections() {
        let patch = ConfigLoader::patch_from_source(
            r#"
[tool.sqruff.core]
dialect = "postgres"
rules = "AL01,LT02"

[tool.sqruff.indentation]
tab_space_size = 2

[tool.sqruff.layout.type.comma]
line_position = "leading"

[tool.sqruff.rules.aliasing.table]
aliasing = "implicit"

[tool.sqruff.dialect.postgres]
pgvector = true
"#,
            None,
            crate::config::ConfigFormat::Toml,
        )
        .unwrap();

        let config = ConfigLoader::new().load_patch(patch).unwrap();

        assert_eq!(config.dialect_kind(), DialectKind::Postgres);
        assert_eq!(
            config
                .rule_allowlist()
                .unwrap()
                .iter()
                .map(RuleSelector::as_str)
                .collect::<Vec<_>>(),
            vec!["AL01", "LT02"]
        );
        assert_eq!(config.indentation().tab_space_size(), 2);
        assert!(
            config
                .layout()
                .types
                .contains_key(&sqruff_lib_core::dialects::syntax::SyntaxKind::Comma)
        );
        assert_eq!(
            config.rules().aliasing.table.aliasing,
            crate::config::AliasingStyle::Implicit
        );
        assert!(config.dialect_configs().postgres.pgvector);
    }

    #[test]
    fn toml_none_string_clears_nullable_csv_config() {
        let patch = ConfigLoader::patch_from_source(
            r#"
[tool.sqlfluff.core]
rules = "None"
exclude_rules = "None"
"#,
            None,
            crate::config::ConfigFormat::Toml,
        )
        .unwrap();

        let config = ConfigLoader::new().load_patch(patch).unwrap();

        assert_eq!(config.rule_allowlist(), None);
        assert!(config.rule_denylist().is_empty());
    }
}
