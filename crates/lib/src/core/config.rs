use std::path::{Path, PathBuf};
use std::str::FromStr;

use configparser::ini::Ini;
use hashbrown::HashMap;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::{DialectKind, dialect_readout};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::parser::{IndentationConfig, Parser};
pub use sqruff_lib_core::value::Value;
use sqruff_lib_dialects::kind_to_dialect;

use crate::templaters::TemplaterKind;
use crate::utils::reflow::config::ReflowConfig;

/// split_comma_separated_string takes a string and splits it on commas and
/// trims and filters out empty strings.
pub fn split_comma_separated_string(raw_str: &str) -> Value {
    let values = raw_str
        .split(',')
        .filter_map(|x| {
            let trimmed = x.trim();
            (!trimmed.is_empty()).then(|| Value::String(trimmed.into()))
        })
        .collect();
    Value::Array(values)
}

fn split_string_or_array(value: &Value) -> Option<Value> {
    match value {
        Value::String(raw) => Some(split_comma_separated_string(raw)),
        Value::Array(values) => Some(Value::Array(
            values
                .iter()
                .map(|value| value.as_string().unwrap())
                .flat_map(|value| match split_comma_separated_string(value) {
                    Value::Array(values) => values,
                    _ => unreachable!(),
                })
                .collect(),
        )),
        _ => None,
    }
}

/// The class that actually gets passed around as a config object.
// TODO This is not a translation that is particularly accurate.
#[derive(Debug, PartialEq, Clone)]
pub struct FluffConfig {
    pub(crate) indentation: FluffConfigIndentation,
    pub raw: HashMap<String, Value>,
    extra_config_path: Option<String>,
    _configs: HashMap<String, HashMap<String, String>>,
    pub(crate) dialect: Dialect,
    sql_file_exts: Vec<String>,
    reflow: ReflowConfig,
}

impl Default for FluffConfig {
    fn default() -> Self {
        Self::new(<_>::default(), None, None)
    }
}

impl FluffConfig {
    fn configured_dialect_kind_from_raw(configs: &HashMap<String, Value>) -> DialectKind {
        match configs
            .get("core")
            .and_then(|map| map.as_map().unwrap().get("dialect"))
        {
            None => DialectKind::default(),
            Some(Value::String(std)) => DialectKind::from_str(std).unwrap(),
            _value => DialectKind::default(),
        }
    }

    fn dialect_section_from_raw(
        configs: &HashMap<String, Value>,
        dialect_kind: DialectKind,
    ) -> Option<&Value> {
        configs
            .get("dialect")
            .and_then(|v| v.as_map())
            .and_then(|m| m.get(dialect_kind.as_ref()))
    }

    pub fn override_dialect(&mut self, dialect: DialectKind) -> Result<(), String> {
        self.dialect =
            kind_to_dialect(&dialect, Self::dialect_section_from_raw(&self.raw, dialect))
                .ok_or(format!("Invalid dialect: {}", dialect.as_ref()))?;
        self.raw
            .entry("core".into())
            .or_insert_with(|| Value::Map(HashMap::new()))
            .as_map_mut()
            .unwrap()
            .insert("dialect".into(), Value::String(dialect.as_ref().into()));
        Ok(())
    }

    pub fn get(&self, key: &str, section: &str) -> &Value {
        &self.raw[section][key]
    }

    pub fn reflow(&self) -> &ReflowConfig {
        &self.reflow
    }

    fn templater_root_section(&self) -> Option<&HashMap<String, Value>> {
        self.raw.get("templater").and_then(Value::as_map)
    }

    pub fn templater_root_value(&self, key: &str) -> Option<&Value> {
        self.templater_root_section()?.get(key)
    }

    pub fn templater_section(&self, templater: TemplaterKind) -> Option<&HashMap<String, Value>> {
        self.templater_root_section()?
            .get(templater.as_str())
            .and_then(Value::as_map)
    }

    pub fn templater_value(&self, templater: TemplaterKind, key: &str) -> Option<&Value> {
        self.templater_section(templater)?.get(key)
    }

    pub fn templater_context(&self, templater: TemplaterKind) -> Option<&HashMap<String, Value>> {
        self.templater_value(templater, "context")
            .and_then(Value::as_map)
    }

    pub fn reload_reflow(&mut self) {
        self.reflow = ReflowConfig::from_fluff_config(self);
    }

    /// from_file creates a config object from a file path. The path is used both
    /// to read the file content and to resolve relative `_path`/`_dir` values.
    pub fn from_file(path: &Path) -> FluffConfig {
        Self::try_from_file(path).unwrap()
    }

    pub fn try_from_file(path: &Path) -> Result<FluffConfig, SQLFluffUserError> {
        let mut configs = HashMap::new();
        ConfigLoader::try_load_config_file(path, &mut configs)?;
        Ok(FluffConfig::new(configs, None, None))
    }

    /// from_source creates a config object from a string. This is used for testing and for
    /// loading a config from a string.
    ///
    /// The optional_path_specification is used to specify a path to use for relative paths in the
    /// config. This is useful for testing.
    pub fn from_source(source: &str, optional_path_specification: Option<&Path>) -> FluffConfig {
        Self::try_from_source(source, optional_path_specification).unwrap()
    }

    pub fn try_from_source(
        source: &str,
        optional_path_specification: Option<&Path>,
    ) -> Result<FluffConfig, SQLFluffUserError> {
        let configs = ConfigLoader::try_from_source(source, optional_path_specification)?;
        Ok(FluffConfig::new(configs, None, None))
    }

    pub fn get_section(&self, section: &str) -> &HashMap<String, Value> {
        self.raw[section].as_map().unwrap()
    }

    pub fn dialect_kind(&self) -> DialectKind {
        self.dialect.name()
    }

    pub fn templater_kind(&self) -> Result<TemplaterKind, String> {
        self.get("templater", "core")
            .as_string()
            .map(TemplaterKind::from_name)
            .transpose()
            .map(|templater| templater.unwrap_or(TemplaterKind::Raw))
    }

    pub fn dialect_section(&self, dialect_kind: DialectKind) -> Option<&Value> {
        Self::dialect_section_from_raw(&self.raw, dialect_kind)
    }

    // TODO This is not a translation that is particularly accurate.
    pub fn new(
        mut configs: HashMap<String, Value>,
        extra_config_path: Option<String>,
        indentation: Option<FluffConfigIndentation>,
    ) -> Self {
        fn nested_combine(
            mut a: HashMap<String, Value>,
            b: HashMap<String, Value>,
        ) -> HashMap<String, Value> {
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

        normalize_implicit_indents_map(&mut configs, "configuration")
            .expect("invalid implicit_indents configuration");

        let values = ConfigLoader::get_config_elems_from_file(
            None,
            include_str!("./default_config.cfg").into(),
        );

        let mut defaults = HashMap::new();
        ConfigLoader::incorporate_vals(&mut defaults, values);

        let mut configs = nested_combine(defaults, configs);

        let dialect_kind = Self::configured_dialect_kind_from_raw(&configs);

        // Extract dialect-specific configuration section (e.g., [sqruff:dialect:snowflake])
        let dialect_config = Self::dialect_section_from_raw(&configs, dialect_kind);

        let dialect = kind_to_dialect(&dialect_kind, dialect_config);
        for (in_key, out_key) in [
            // Deal with potential ignore & warning parameters
            ("ignore", "ignore"),
            ("warnings", "warnings"),
            ("rules", "rule_allowlist"),
            // Allowlists and denylistsignore_words
            ("exclude_rules", "rule_denylist"),
        ] {
            match configs["core"].as_map().unwrap().get(in_key) {
                Some(value) if !value.is_none() => {
                    let values = split_string_or_array(value).unwrap();

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

        let sql_file_exts = configs["core"]["sql_file_exts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|it| it.as_string().unwrap().to_owned())
            .collect();

        let mut this = Self {
            raw: configs,
            dialect: dialect
                .expect("Dialect is disabled. Please enable the corresponding feature."),
            extra_config_path,
            _configs: HashMap::new(),
            indentation: indentation.unwrap_or_default(),
            sql_file_exts,
            reflow: ReflowConfig::default(),
        };
        this.reflow = ReflowConfig::from_fluff_config(&this);
        this
    }

    pub fn with_sql_file_exts(mut self, exts: Vec<String>) -> Self {
        self.sql_file_exts = exts;
        self
    }

    /// Loads a config object just based on the root directory.
    // TODO This is not a translation that is particularly accurate.
    pub fn from_root(
        extra_config_path: Option<String>,
        ignore_local_config: bool,
        overrides: Option<HashMap<String, String>>,
    ) -> Result<FluffConfig, SQLFluffUserError> {
        let loader = ConfigLoader {};
        let mut config = loader.try_load_config_up_to_path(
            ".",
            extra_config_path.clone(),
            ignore_local_config,
        )?;

        if let Some(overrides) = overrides
            && let Some(dialect) = overrides.get("dialect")
        {
            let core = config
                .entry("core".into())
                .or_insert_with(|| Value::Map(HashMap::new()));

            core.as_map_mut()
                .unwrap()
                .insert("dialect".into(), Value::String(dialect.clone().into()));
        }

        Ok(FluffConfig::new(config, extra_config_path, None))
    }

    /// Construct a config from a subset of common options.
    ///
    /// `rules` identifies rules to include, while `exclude_rules` identifies
    /// rules to exclude. Both accept rule codes, names, groups, or aliases.
    /// Empty or omitted rule lists inherit the standard sqruff defaults.
    ///
    /// This is a convenience constructor for callers that do not need to
    /// assemble a full [`FluffConfig`] first.
    pub fn from_kwargs(
        dialect: Option<DialectKind>,
        rules: Option<Vec<String>>,
        exclude_rules: Option<Vec<String>>,
    ) -> Self {
        let mut core = HashMap::new();

        if let Some(dialect) = dialect {
            core.insert(
                "dialect".into(),
                Value::String(dialect.as_ref().to_owned().into()),
            );
        }
        if let Some(rules) = rules.filter(|rules| !rules.is_empty()) {
            core.insert("rules".into(), Value::String(rules.join(",").into()));
        }
        if let Some(exclude_rules) = exclude_rules.filter(|rules| !rules.is_empty()) {
            core.insert(
                "exclude_rules".into(),
                Value::String(exclude_rules.join(",").into()),
            );
        }

        Self::new(
            HashMap::from([("core".into(), Value::Map(core))]),
            None,
            None,
        )
    }

    /// Apply inline configuration before constructing a parser or rule pack.
    pub fn process_raw_file_for_config(&mut self, raw_str: &str) -> Result<(), SQLFluffUserError> {
        for line in raw_str.lines() {
            let line = line.trim();
            if line.starts_with("-- sqlfluff:") || line.starts_with("-- sqruff:") {
                self.process_inline_config(line)?;
            }
        }
        Ok(())
    }

    /// Apply a colon-separated inline configuration directive.
    pub fn process_inline_config(&mut self, config_line: &str) -> Result<(), SQLFluffUserError> {
        let directive = config_line
            .trim()
            .strip_prefix("-- sqlfluff:")
            .or_else(|| config_line.trim().strip_prefix("-- sqruff:"))
            .ok_or_else(|| {
                SQLFluffUserError::new("Invalid inline configuration directive".into())
            })?;
        let mut parts: Vec<String> = directive
            .split(':')
            .map(|part| part.trim().to_owned())
            .collect();
        if parts.len() < 2 {
            return Err(SQLFluffUserError::new(
                "Inline configuration requires a key and value".into(),
            ));
        }
        let raw_value = parts.pop().unwrap();
        if parts.len() == 1 {
            parts.insert(0, "core".into());
        }
        if parts == ["core", "dialect"] {
            DialectKind::from_str(&raw_value)
                .map_err(|err| SQLFluffUserError::new(err.to_string()))?;
        }
        let value = raw_value.parse::<Value>().map_err(|_| {
            SQLFluffUserError::new(format!("Invalid inline configuration value: {raw_value}"))
        })?;
        let mut raw = self.raw.clone();
        let mut elems = vec![(parts, value)];
        normalize_implicit_indents_elems(&mut elems, "inline configuration")?;
        ConfigLoader::incorporate_vals(&mut raw, elems);
        *self = Self::new(
            raw,
            self.extra_config_path.clone(),
            Some(self.indentation.clone()),
        );
        Ok(())
    }

    /// Check if the config specifies a dialect, raising an error if not.
    pub fn verify_dialect_specified(&self) -> Option<SQLFluffUserError> {
        if self._configs.get("core")?.get("dialect").is_some() {
            return None;
        }
        // Get list of available dialects for the error message. We must
        // import here rather than at file scope in order to avoid a circular
        // import.
        Some(SQLFluffUserError::new(format!(
            "No dialect was specified. You must configure a dialect or
specify one on the command line using --dialect after the
command. Available dialects: {}",
            dialect_readout().join(", ").as_str()
        )))
    }

    pub fn get_dialect(&self) -> &Dialect {
        &self.dialect
    }

    pub fn sql_file_exts(&self) -> &[String] {
        self.sql_file_exts.as_ref()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FluffConfigIndentation {
    pub template_blocks_indent: bool,
}

impl Default for FluffConfigIndentation {
    fn default() -> Self {
        Self {
            template_blocks_indent: true,
        }
    }
}

pub struct ConfigLoader;

impl ConfigLoader {
    fn user_home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }

    fn iter_config_locations_up_to_path(
        path: &Path,
        working_path: Option<&Path>,
        home_path: Option<&Path>,
        _ignore_local_config: bool,
    ) -> impl Iterator<Item = PathBuf> {
        let mut given_path = std::path::absolute(path).unwrap();
        let working_path = working_path
            .map(|path| std::path::absolute(path).unwrap())
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        if !given_path.is_dir() {
            given_path = given_path.parent().unwrap().into();
        }

        let resolve = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut paths = Vec::new();

        let home_path = home_path
            .map(|path| std::path::absolute(path).unwrap())
            .or_else(Self::user_home_dir)
            .map(|path| std::path::absolute(path).unwrap());
        if let Some(home_path) = home_path
            && let Ok(relative_path) = working_path.strip_prefix(&home_path)
        {
            let mut path_to_visit = home_path;
            paths.push(resolve(&path_to_visit));
            for component in relative_path.components() {
                path_to_visit.push(component.as_os_str());
                if path_to_visit != working_path {
                    paths.push(resolve(&path_to_visit));
                }
            }
        }

        if let Some(mut path_to_visit) = common_path::common_path(&given_path, &working_path) {
            loop {
                let path = resolve(&path_to_visit);
                if paths.last() != Some(&path) {
                    paths.push(path);
                }
                if path_to_visit == given_path {
                    break;
                }

                let Ok(relative_path) = given_path.strip_prefix(&path_to_visit) else {
                    break;
                };
                let Some(first_part) = relative_path.components().next() else {
                    break;
                };
                let next_path_to_visit = path_to_visit.join(first_part.as_os_str());
                if next_path_to_visit == path_to_visit {
                    break;
                }
                path_to_visit = next_path_to_visit;
            }
        } else {
            // This can happen on Windows when the paths are on different drives.
            paths.push(resolve(&working_path));
            paths.push(resolve(&given_path));
        }

        paths.into_iter()
    }

    pub fn load_config_up_to_path(
        &self,
        path: impl AsRef<Path>,
        extra_config_path: Option<String>,
        ignore_local_config: bool,
    ) -> HashMap<String, Value> {
        self.try_load_config_up_to_path(path, extra_config_path, ignore_local_config)
            .unwrap()
    }

    pub fn try_load_config_up_to_path(
        &self,
        path: impl AsRef<Path>,
        extra_config_path: Option<String>,
        ignore_local_config: bool,
    ) -> Result<HashMap<String, Value>, SQLFluffUserError> {
        let path = path.as_ref();

        let mut config_stack = if ignore_local_config {
            Vec::new()
        } else {
            let configs =
                Self::iter_config_locations_up_to_path(path, None, None, ignore_local_config);
            configs
                .map(|path| self.try_load_config_at_path(path))
                .collect::<Result<Vec<_>, _>>()?
        };

        if let Some(extra_config_path) = extra_config_path {
            let path = PathBuf::from(&extra_config_path);
            if !path.exists() {
                return Err(SQLFluffUserError::new(format!(
                    "Extra config path '{extra_config_path}' does not exist."
                )));
            }

            let path = std::path::absolute(&path).unwrap_or(path);
            let mut extra_config = HashMap::new();
            Self::try_load_config_file(path, &mut extra_config)?;
            config_stack.push(extra_config);
        }

        Ok(nested_combine(config_stack))
    }

    pub fn load_config_at_path(&self, path: impl AsRef<Path>) -> HashMap<String, Value> {
        self.try_load_config_at_path(path).unwrap()
    }

    pub fn try_load_config_at_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<HashMap<String, Value>, SQLFluffUserError> {
        let path = path.as_ref();

        let filename_options = [
            /* "setup.cfg", "tox.ini", "pep8.ini", */
            ".sqlfluff",
            ".sqruff",
            ".sqruff.ini",
            "pyproject.toml",
            "sqruff.toml",
        ];

        let mut configs = HashMap::new();

        if path.is_dir() {
            for fname in filename_options {
                let path = path.join(fname);
                if path.exists() {
                    ConfigLoader::try_load_config_file(path, &mut configs)?;
                }
            }
        } else if path.is_file() {
            ConfigLoader::try_load_config_file(path, &mut configs)?;
        };

        Ok(configs)
    }

    pub fn from_source(source: &str, path: Option<&Path>) -> HashMap<String, Value> {
        Self::try_from_source(source, path).unwrap()
    }

    pub fn try_from_source(
        source: &str,
        path: Option<&Path>,
    ) -> Result<HashMap<String, Value>, SQLFluffUserError> {
        let mut configs = HashMap::new();
        let elems = ConfigLoader::try_get_config_elems_from_file(path, Some(source))?;
        ConfigLoader::incorporate_vals(&mut configs, elems);
        Ok(configs)
    }

    pub fn load_config_file(path: impl AsRef<Path>, configs: &mut HashMap<String, Value>) {
        Self::try_load_config_file(path, configs).unwrap();
    }

    pub fn try_load_config_file(
        path: impl AsRef<Path>,
        configs: &mut HashMap<String, Value>,
    ) -> Result<(), SQLFluffUserError> {
        let elems = ConfigLoader::try_get_config_elems_from_file(path.as_ref().into(), None)?;
        ConfigLoader::incorporate_vals(configs, elems);
        Ok(())
    }

    fn get_config_elems_from_file(
        config_path: Option<&Path>,
        config_string: Option<&str>,
    ) -> Vec<(Vec<String>, Value)> {
        Self::try_get_config_elems_from_file(config_path, config_string).unwrap()
    }

    fn try_get_config_elems_from_file(
        config_path: Option<&Path>,
        config_string: Option<&str>,
    ) -> Result<Vec<(Vec<String>, Value)>, SQLFluffUserError> {
        let content = match (config_path, config_string) {
            (None, None) => {
                unimplemented!("One of fpath or config_string is required.")
            }
            (_, Some(text)) => text.to_owned(),
            (Some(path), None) => std::fs::read_to_string(path).map_err(|err| {
                config_error(config_path, format!("Unable to read config file: {err}"))
            })?,
        };

        let mut elems = if is_toml_config(config_path) {
            parse_toml_config_elems(&content, config_path)?
        } else {
            parse_ini_config_elems(&content, config_path)?
        };

        normalize_implicit_indents_elems(&mut elems, &config_reference(config_path))?;
        Ok(elems)
    }

    fn incorporate_vals(ctx: &mut HashMap<String, Value>, values: Vec<(Vec<String>, Value)>) {
        for (path, value) in values {
            let mut current_map = &mut *ctx;
            for key in path.iter().take(path.len() - 1) {
                match current_map
                    .entry(key.to_string())
                    .or_insert_with(|| Value::Map(HashMap::new()))
                    .as_map_mut()
                {
                    Some(slot) => current_map = slot,
                    None => panic!("Overriding config value with section! [{path:?}]"),
                }
            }

            let last_key = path.last().expect("Expected at least one element in path");
            current_map.insert(last_key.to_string(), value);
        }
    }
}

fn is_toml_config(config_path: Option<&Path>) -> bool {
    config_path.is_some_and(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "pyproject.toml" || name.ends_with(".toml"))
    })
}

fn config_error(config_path: Option<&Path>, message: impl std::fmt::Display) -> SQLFluffUserError {
    let location = config_reference(config_path);
    SQLFluffUserError::new(format!("Error loading config from {location}: {}", message))
}

fn config_reference(config_path: Option<&Path>) -> String {
    config_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "config source".to_owned())
}

fn format_toml_parse_error(
    content: &str,
    config_path: Option<&Path>,
    err: &toml::de::Error,
) -> SQLFluffUserError {
    let location = err.span().map(|span| {
        let mut offset = span.start.min(content.len());
        while !content.is_char_boundary(offset) {
            offset -= 1;
        }
        let prefix = &content[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .count()
            + 1;
        format!(" at line {line}, column {column}")
    });
    let hint = content
        .starts_with('\u{feff}')
        .then(|| toml_bom_hint(config_path));

    SQLFluffUserError::new(format!(
        "Failed to parse TOML config file {}: {}{}.{}",
        config_reference(config_path),
        err.message(),
        location.unwrap_or_default(),
        hint.unwrap_or_default(),
    ))
}

fn toml_bom_hint(config_path: Option<&Path>) -> &'static str {
    if config_path.is_some_and(|path| {
        path.file_name()
            .is_some_and(|name| name == "pyproject.toml")
    }) {
        " pyproject.toml contains a UTF-8 BOM; save it as UTF-8 without BOM."
    } else {
        " TOML config contains a UTF-8 BOM; save it as UTF-8 without BOM."
    }
}

const ALLOWABLE_IMPLICIT_INDENTS_VALUES: [&str; 3] = ["forbid", "allow", "require"];

fn validate_implicit_indents_value(
    value: &Value,
    logging_reference: &str,
) -> Result<(), SQLFluffUserError> {
    if value
        .as_string()
        .is_some_and(|value| ALLOWABLE_IMPLICIT_INDENTS_VALUES.contains(&value))
    {
        return Ok(());
    }

    Err(SQLFluffUserError::new(format!(
        "Config file {logging_reference:?} set an invalid value for `implicit_indents`: {value:?}. \
         Valid options are: {}.",
        ALLOWABLE_IMPLICIT_INDENTS_VALUES.join(", ")
    )))
}

fn translate_allow_implicit_indents(
    value: &Value,
    logging_reference: &str,
) -> Result<Value, SQLFluffUserError> {
    let Some(value) = value.as_bool() else {
        return Err(SQLFluffUserError::new(format!(
            "Config file {logging_reference:?} set an invalid value for the deprecated \
             `allow_implicit_indents` option: {value:?}. Expected true or false."
        )));
    };

    Ok(Value::String(if value { "allow" } else { "forbid" }.into()))
}

fn warn_implicit_indents_migration(logging_reference: &str, duplicate: bool) {
    if duplicate {
        log::warn!(
            "Config file {logging_reference} sets both deprecated `allow_implicit_indents` and \
             `implicit_indents`; the new value takes precedence."
        );
    } else {
        log::warn!(
            "Config file {logging_reference} uses deprecated `allow_implicit_indents`; use \
             `implicit_indents = forbid`, `allow`, or `require` instead."
        );
    }
}

fn normalize_implicit_indents_elems(
    elems: &mut Vec<(Vec<String>, Value)>,
    logging_reference: &str,
) -> Result<(), SQLFluffUserError> {
    let is_old = |path: &[String]| {
        path.len() == 2 && path[0] == "indentation" && path[1] == "allow_implicit_indents"
    };
    let is_new = |path: &[String]| {
        path.len() == 2 && path[0] == "indentation" && path[1] == "implicit_indents"
    };

    if let Some(old_idx) = elems.iter().position(|(path, _)| is_old(path)) {
        let new_exists = elems.iter().any(|(path, _)| is_new(path));
        warn_implicit_indents_migration(logging_reference, new_exists);
        if new_exists {
            elems.remove(old_idx);
        } else {
            let translated =
                translate_allow_implicit_indents(&elems[old_idx].1, logging_reference)?;
            elems[old_idx] = (
                vec!["indentation".into(), "implicit_indents".into()],
                translated,
            );
        }
    }

    if let Some((_, value)) = elems.iter().find(|(path, _)| is_new(path)) {
        validate_implicit_indents_value(value, logging_reference)?;
    }

    Ok(())
}

fn normalize_implicit_indents_map(
    configs: &mut HashMap<String, Value>,
    logging_reference: &str,
) -> Result<(), SQLFluffUserError> {
    let Some(indentation) = configs.get_mut("indentation").and_then(Value::as_map_mut) else {
        return Ok(());
    };

    if let Some(old_value) = indentation.remove("allow_implicit_indents") {
        let new_exists = indentation.contains_key("implicit_indents");
        warn_implicit_indents_migration(logging_reference, new_exists);
        if !new_exists {
            indentation.insert(
                "implicit_indents".into(),
                translate_allow_implicit_indents(&old_value, logging_reference)?,
            );
        }
    }

    if let Some(value) = indentation.get("implicit_indents") {
        validate_implicit_indents_value(value, logging_reference)?;
    }

    Ok(())
}

fn parse_ini_config_elems(
    content: &str,
    config_path: Option<&Path>,
) -> Result<Vec<(Vec<String>, Value)>, SQLFluffUserError> {
    let mut buff = Vec::new();
    let mut config = Ini::new();

    config
        .read(content.to_owned())
        .map_err(|err| config_error(config_path, err))?;

    for section in config.sections() {
        let key = if section == "sqlfluff" || section == "sqruff" {
            vec!["core".to_owned()]
        } else if let Some(key) = section
            .strip_prefix("sqlfluff:")
            .or_else(|| section.strip_prefix("sqruff:"))
        {
            key.split(':').map(ToOwned::to_owned).collect()
        } else {
            continue;
        };

        let config_map = config.get_map_ref();
        if let Some(section) = config_map.get(&section) {
            for (name, value) in section {
                let mut value: Value = value.as_deref().unwrap_or_default().parse().unwrap();
                let name_lowercase = name.to_lowercase();

                if matches!(
                    name_lowercase.as_str(),
                    "load_macros_from_path" | "exclude_macros_from_path" | "loader_search_path"
                ) {
                    value = resolve_comma_separated_config_paths(value, config_path);
                } else if name_lowercase.ends_with("_path") || name_lowercase.ends_with("_dir") {
                    value = resolve_relative_config_path(value, config_path);
                }

                let mut key = key.clone();
                key.extend(name.split('.').map(ToOwned::to_owned));
                buff.push((key, value));
            }
        }
    }

    Ok(buff)
}

fn parse_toml_config_elems(
    content: &str,
    config_path: Option<&Path>,
) -> Result<Vec<(Vec<String>, Value)>, SQLFluffUserError> {
    // Python's tomllib rejects a UTF-8 BOM, while the Rust toml parser accepts
    // it. Reject it explicitly so sqruff matches SQLFluff and can provide the
    // same targeted remediation hint.
    if content.starts_with('\u{feff}') {
        return Err(SQLFluffUserError::new(format!(
            "Failed to parse TOML config file {}: unexpected UTF-8 BOM at line 1, column 1.{}",
            config_reference(config_path),
            toml_bom_hint(config_path),
        )));
    }

    let root = content
        .parse::<toml::Table>()
        .map_err(|err| format_toml_parse_error(content, config_path, &err))?;

    let mut buff = Vec::new();

    for config_root in ["sqlfluff", "sqruff"] {
        if let Some(table) = root.get(config_root).and_then(toml::Value::as_table) {
            collect_toml_config_elems(table, Vec::new(), config_path, &mut buff);
        }
    }

    if let Some(tool) = root.get("tool").and_then(toml::Value::as_table) {
        for config_root in ["sqlfluff", "sqruff"] {
            if let Some(table) = tool.get(config_root).and_then(toml::Value::as_table) {
                collect_toml_config_elems(table, Vec::new(), config_path, &mut buff);
            }
        }
    }

    Ok(buff)
}

fn collect_toml_config_elems(
    table: &toml::Table,
    section_path: Vec<String>,
    config_path: Option<&Path>,
    buff: &mut Vec<(Vec<String>, Value)>,
) {
    for (name, value) in table {
        match value {
            toml::Value::Table(table) => {
                let mut section_path = section_path.clone();
                section_path.push(name.to_owned());
                collect_toml_config_elems(table, section_path, config_path, buff);
            }
            value => {
                let mut value = toml_value_to_config_value(value);
                if matches!(
                    name.as_str(),
                    "load_macros_from_path" | "exclude_macros_from_path" | "loader_search_path"
                ) {
                    value = resolve_comma_separated_config_paths(value, config_path);
                } else if name.ends_with("_path") || name.ends_with("_dir") {
                    value = resolve_relative_config_path(value, config_path);
                }

                let key = toml_config_key_path(&section_path, name);
                buff.push((key, value));
            }
        }
    }
}

fn toml_config_key_path(section_path: &[String], key: &str) -> Vec<String> {
    if section_path.is_empty()
        || (section_path.len() == 1
            && section_path
                .first()
                .is_some_and(|section| section == "core"))
    {
        vec!["core".to_owned(), key.to_owned()]
    } else if section_path
        .first()
        .is_some_and(|section| section == "rules")
        && section_path.len() > 1
    {
        let rest = &section_path[1..];
        vec!["rules".to_owned(), rest.join("."), key.to_owned()]
    } else {
        section_path
            .iter()
            .cloned()
            .chain(std::iter::once(key.to_owned()))
            .collect()
    }
}

fn toml_value_to_config_value(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(value) => Value::String(value.clone().into()),
        toml::Value::Integer(value) => {
            Value::Int((*value).try_into().expect("TOML integer out of i32 range"))
        }
        toml::Value::Float(value) => Value::Float(*value),
        toml::Value::Boolean(value) => Value::Bool(*value),
        toml::Value::Datetime(value) => Value::String(value.to_string().into()),
        // Config arrays contain strings, matching values loaded from INI files.
        toml::Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| {
                    let value = match value {
                        toml::Value::String(value) => value.clone(),
                        toml::Value::Boolean(true) => "True".to_owned(),
                        toml::Value::Boolean(false) => "False".to_owned(),
                        value => value.to_string(),
                    };
                    Value::String(value.into())
                })
                .collect::<Vec<_>>(),
        ),
        toml::Value::Table(values) => Value::Map(
            values
                .iter()
                .map(|(key, value)| (key.clone(), toml_value_to_config_value(value)))
                .collect(),
        ),
    }
}

fn resolve_relative_config_path(mut value: Value, config_path: Option<&Path>) -> Value {
    // Non-string values (e.g. `none`, which parses to `Value::None`) have no
    // path to resolve; leave them untouched.
    let Some(raw) = value.as_string() else {
        return value;
    };
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        let config_path = config_path.unwrap().parent().unwrap();
        let current_dir = std::env::current_dir().unwrap();
        let config_path = current_dir.join(config_path);
        let config_path = std::path::absolute(config_path).unwrap();
        let path = config_path.join(path);
        let path: String = path.to_string_lossy().into();
        value = Value::String(path.into());
    }
    value
}

fn resolve_config_path_pattern(value: Value, config_path: Option<&Path>) -> Vec<Value> {
    let resolved = resolve_relative_config_path(value, config_path);
    let Some(pattern) = resolved.as_string() else {
        return Vec::new();
    };
    let Ok(matches) = glob::glob(pattern) else {
        return Vec::new();
    };

    matches
        .filter_map(Result::ok)
        .map(|path| Value::String(path.to_string_lossy().into_owned().into()))
        .collect()
}

fn resolve_comma_separated_config_paths(value: Value, config_path: Option<&Path>) -> Value {
    let Some(Value::Array(paths)) = split_string_or_array(&value) else {
        return value;
    };

    let resolved_paths = paths
        .iter()
        .cloned()
        .flat_map(|path| resolve_config_path_pattern(path, config_path))
        .collect::<Vec<_>>();

    // Preserve the original patterns if none of them match. This mirrors
    // SQLFluff's fallback while keeping sqruff's array-valued config invariant.
    Value::Array(if resolved_paths.is_empty() {
        paths
    } else {
        resolved_paths
    })
}

fn nested_combine(config_stack: Vec<HashMap<String, Value>>) -> HashMap<String, Value> {
    let capacity = config_stack.len();
    let mut result = HashMap::with_capacity(capacity);

    for dict in config_stack {
        for (key, value) in dict {
            match (result.get_mut(&key), value) {
                (Some(Value::Map(existing)), Value::Map(incoming)) => {
                    *existing = nested_combine(vec![std::mem::take(existing), incoming]);
                }
                (_, value) => {
                    result.insert(key, value);
                }
            }
        }
    }

    result
}

impl<'a> From<&'a FluffConfig> for Parser<'a> {
    fn from(config: &'a FluffConfig) -> Self {
        let dialect = config.get_dialect();
        let indentation_section = &config.raw["indentation"];
        let indentation_config =
            IndentationConfig::from_bool_lookup(|key| indentation_section[key].to_bool());
        Self::new(dialect, indentation_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqruff_lib_core::dialects::init::DialectKind;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_config_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sqruff-config-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_from_kwargs_constructs_config_from_optional_values() {
        let default_config = FluffConfig::from_kwargs(None, None, None);
        assert_eq!(default_config.dialect_kind(), DialectKind::Ansi);

        let config = FluffConfig::from_kwargs(
            Some(DialectKind::Postgres),
            Some(vec!["LT01".into(), "LT02".into()]),
            Some(vec!["AM01".into()]),
        );

        assert_eq!(config.dialect_kind(), DialectKind::Postgres);
        assert_eq!(
            config.get("rule_allowlist", "core").as_array().unwrap(),
            &[Value::String("LT01".into()), Value::String("LT02".into())]
        );
        assert_eq!(
            config.get("rule_denylist", "core").as_array().unwrap(),
            &[Value::String("AM01".into())]
        );
    }

    #[test]
    fn default_sql_file_extensions_include_oracle_package_bodies() {
        let config = FluffConfig::default();

        assert!(config.sql_file_exts().iter().any(|ext| ext == ".pkb"));
    }

    #[test]
    fn test_config_locations_are_ordered_outer_to_inner() {
        let root = temp_config_dir("path-order");
        let nested = root.join("config").join("nested");
        fs::create_dir_all(&nested).unwrap();

        let paths = ConfigLoader::iter_config_locations_up_to_path(
            &nested.join("query.sql"),
            Some(&root),
            Some(&root),
            false,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec![
                root.canonicalize().unwrap(),
                root.join("config").canonicalize().unwrap(),
                nested.canonicalize().unwrap(),
            ]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_parent_configs_are_loaded_from_home_to_working_directory() {
        let root = temp_config_dir("parent-search");
        let home = root.join("home");
        let project = home.join("project");
        let working = project.join("nested");
        fs::create_dir_all(&working).unwrap();

        fs::write(
            home.join(".sqruff"),
            "[sqruff]\ndialect = mysql\nexclude_rules = AM01\n",
        )
        .unwrap();
        fs::write(project.join(".sqruff"), "[sqruff]\nmax_line_length = 91\n").unwrap();
        fs::write(working.join(".sqruff"), "[sqruff]\ndialect = postgres\n").unwrap();

        let locations = ConfigLoader::iter_config_locations_up_to_path(
            &working.join("query.sql"),
            Some(&working),
            Some(&home),
            false,
        )
        .collect::<Vec<_>>();
        assert_eq!(
            locations,
            vec![
                home.canonicalize().unwrap(),
                project.canonicalize().unwrap(),
                working.canonicalize().unwrap(),
            ]
        );

        let configs = locations
            .into_iter()
            .map(|path| ConfigLoader.try_load_config_at_path(path).unwrap())
            .collect();
        let config = FluffConfig::new(nested_combine(configs), None, None);

        assert_eq!(config.get_dialect().name, DialectKind::Postgres);
        assert_eq!(config.raw["core"]["max_line_length"].as_int(), Some(91));
        assert_eq!(
            config.raw["core"]["rule_denylist"].as_array().unwrap(),
            &[Value::String("AM01".into())]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_extra_config_is_loaded_last_from_exact_file() {
        let root = temp_config_dir("extra-config");
        let extra_config = root.join("extra").join("this_can_have_any_name.cfg");
        fs::create_dir_all(extra_config.parent().unwrap()).unwrap();
        fs::write(
            root.join(".sqruff"),
            "[sqruff]\ndialect = mysql\n[sqruff:bar]\nfoo = project\n",
        )
        .unwrap();
        fs::write(&extra_config, "[sqruff:bar]\nfoo = extra\n").unwrap();

        let config = ConfigLoader
            .try_load_config_up_to_path(
                root.join("query.sql"),
                Some(extra_config.to_string_lossy().into_owned()),
                false,
            )
            .unwrap();

        assert_eq!(config["core"]["dialect"].as_string(), Some("mysql"));
        assert_eq!(config["bar"]["foo"].as_string(), Some("extra"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_missing_extra_config_returns_user_error() {
        let root = temp_config_dir("missing-extra-config");
        let missing = root.join("does-not-exist.cfg");

        let error = ConfigLoader
            .try_load_config_up_to_path(&root, Some(missing.to_string_lossy().into_owned()), true)
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("Extra config path '{}' does not exist.", missing.display())
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_dialect_config_section_parsing() {
        // Test that [sqruff:dialect:snowflake] section is correctly parsed
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = snowflake

[sqruff:dialect:snowflake]
some_option = value
"#,
            None,
        );

        // Verify that the dialect config section is accessible
        let dialect_section = config.raw.get("dialect");
        assert!(dialect_section.is_some());

        let snowflake_config = dialect_section.unwrap().as_map().unwrap().get("snowflake");
        assert!(snowflake_config.is_some());

        let snowflake_map = snowflake_config.unwrap().as_map().unwrap();
        assert_eq!(
            snowflake_map.get("some_option").unwrap().as_string(),
            Some("value")
        );
    }

    #[test]
    fn test_dialect_config_empty_section() {
        // Test that empty [sqruff:dialect:bigquery] section works
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = bigquery

[sqruff:dialect:bigquery]
"#,
            None,
        );

        // The config should still be valid
        assert_eq!(config.get_dialect().name, DialectKind::Bigquery);
    }

    #[test]
    fn test_dialect_without_config_section() {
        // Test that a dialect works without a config section
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = postgres
"#,
            None,
        );

        // The config should still be valid
        assert_eq!(config.get_dialect().name, DialectKind::Postgres);
    }

    #[test]
    fn test_templater_kind_defaults_to_raw() {
        let config = FluffConfig::from_source("", None);
        assert_eq!(config.templater_kind().unwrap(), TemplaterKind::Raw);
    }

    #[test]
    fn test_templater_kind_parses_placeholder() {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
templater = placeholder
"#,
            None,
        );

        assert_eq!(config.templater_kind().unwrap(), TemplaterKind::Placeholder);
    }

    #[test]
    fn test_library_path_none_does_not_resolve() {
        // A `library_path` of `none` parses to `Value::None` and must not be run
        // through relative-path resolution (which would otherwise panic without a
        // config path). It should simply be stored as an empty value.
        let config = FluffConfig::from_source(
            r#"
[sqruff]
library_path = none
"#,
            None,
        );

        assert!(config.get("library_path", "core").is_none());
    }

    #[test]
    fn test_config_path_patterns_expand_globs_and_exact_files() {
        let root = temp_config_dir("path-globs");
        let macros = root.join("macros");
        let nested = macros.join("nested");
        fs::create_dir_all(&nested).unwrap();
        for path in [
            macros.join("macro1.sql"),
            macros.join("macro2.sql"),
            macros.join("other.txt"),
            nested.join("macro3.sql"),
        ] {
            fs::write(path, "").unwrap();
        }
        let config_path = root.join(".sqruff");

        let mut sql_matches =
            resolve_config_path_pattern(Value::String("macros/*.sql".into()), Some(&config_path));
        sql_matches.sort_by(|left, right| left.as_string().cmp(&right.as_string()));
        assert_eq!(
            sql_matches,
            vec![
                Value::String(
                    macros
                        .join("macro1.sql")
                        .to_string_lossy()
                        .into_owned()
                        .into()
                ),
                Value::String(
                    macros
                        .join("macro2.sql")
                        .to_string_lossy()
                        .into_owned()
                        .into()
                ),
            ]
        );

        assert_eq!(
            resolve_config_path_pattern(
                Value::String("macros/nested/macro3.sql".into()),
                Some(&config_path),
            ),
            vec![Value::String(
                nested
                    .join("macro3.sql")
                    .to_string_lossy()
                    .into_owned()
                    .into()
            )]
        );
        assert!(
            resolve_config_path_pattern(Value::String("missing/*.sql".into()), Some(&config_path),)
                .is_empty()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_comma_separated_config_paths_expand_globs() {
        let root = temp_config_dir("comma-path-globs");
        let macros = root.join("macros");
        let nested = macros.join("nested");
        fs::create_dir_all(&nested).unwrap();
        for path in [
            macros.join("macro1.sql"),
            macros.join("macro2.sql"),
            nested.join("macro3.sql"),
        ] {
            fs::write(path, "").unwrap();
        }
        let config_path = root.join(".sqruff");
        let config = FluffConfig::from_source(
            r#"
[sqruff:templater:jinja]
load_macros_from_path = macros/*.sql, macros/nested/*.sql
"#,
            Some(&config_path),
        );

        let paths = config.raw["templater"]["jinja"]["load_macros_from_path"]
            .as_array()
            .unwrap();
        assert_eq!(paths.len(), 3);
        assert!(
            paths
                .iter()
                .any(|path| path.as_string().unwrap().ends_with("macro1.sql"))
        );
        assert!(
            paths
                .iter()
                .any(|path| path.as_string().unwrap().ends_with("macro2.sql"))
        );
        assert!(
            paths
                .iter()
                .any(|path| path.as_string().unwrap().ends_with("macro3.sql"))
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_comma_separated_config_paths_preserve_unmatched_patterns() {
        let root = temp_config_dir("unmatched-path-globs");
        let config_path = root.join(".sqruff");
        let config = FluffConfig::from_source(
            r#"
[sqruff:templater:jinja]
load_macros_from_path = empty/*.sql, missing/*.sql
"#,
            Some(&config_path),
        );

        assert_eq!(
            config.raw["templater"]["jinja"]["load_macros_from_path"]
                .as_array()
                .unwrap(),
            &[
                Value::String("empty/*.sql".into()),
                Value::String("missing/*.sql".into()),
            ]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_jinja_loader_search_path_resolves_each_ini_path() {
        let root = temp_config_dir("loader-search-path-ini");
        fs::create_dir_all(root.join("search_a")).unwrap();
        fs::create_dir_all(root.join("search_b/subdir")).unwrap();
        let config_path = root.join(".sqruff");
        let config = FluffConfig::from_source(
            r#"
[sqruff:templater:jinja]
loader_search_path = search_a, search_b/subdir
"#,
            Some(&config_path),
        );

        let paths = config
            .raw
            .get("templater")
            .unwrap()
            .as_map()
            .unwrap()
            .get("jinja")
            .unwrap()
            .as_map()
            .unwrap()
            .get("loader_search_path")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_string().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                config_path
                    .parent()
                    .unwrap()
                    .join("search_a")
                    .to_string_lossy()
                    .to_string(),
                config_path
                    .parent()
                    .unwrap()
                    .join("search_b/subdir")
                    .to_string_lossy()
                    .to_string(),
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_jinja_macro_paths_resolve_each_ini_path() {
        let root = temp_config_dir("macro-paths-ini");
        fs::create_dir_all(root.join("macros/excluded")).unwrap();
        fs::create_dir_all(root.join("shared")).unwrap();
        fs::write(root.join("shared/macros.sql"), "").unwrap();
        fs::write(root.join("shared/ignored.sql"), "").unwrap();
        let config_path = root.join(".sqruff");
        let config = FluffConfig::from_source(
            r#"
[sqruff:templater:jinja]
load_macros_from_path = macros, shared/macros.sql
exclude_macros_from_path = macros/excluded, shared/ignored.sql
"#,
            Some(&config_path),
        );

        for (key, relative_paths) in [
            ("load_macros_from_path", ["macros", "shared/macros.sql"]),
            (
                "exclude_macros_from_path",
                ["macros/excluded", "shared/ignored.sql"],
            ),
        ] {
            let paths = config
                .raw
                .get("templater")
                .unwrap()
                .as_map()
                .unwrap()
                .get("jinja")
                .unwrap()
                .as_map()
                .unwrap()
                .get(key)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_string().unwrap().to_string())
                .collect::<Vec<_>>();
            let expected = relative_paths
                .into_iter()
                .map(|path| {
                    config_path
                        .parent()
                        .unwrap()
                        .join(path)
                        .to_string_lossy()
                        .to_string()
                })
                .collect::<Vec<_>>();
            assert_eq!(paths, expected);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_jinja_macro_paths_resolve_toml_arrays() {
        let root = temp_config_dir("macro-paths-toml");
        fs::create_dir_all(root.join("macros/excluded")).unwrap();
        fs::create_dir_all(root.join("shared")).unwrap();
        fs::write(root.join("shared/macros.sql"), "").unwrap();
        fs::write(root.join("shared/ignored.sql"), "").unwrap();
        let config_path = root.join("pyproject.toml");
        let config = FluffConfig::from_source(
            r#"
[tool.sqruff.templater.jinja]
load_macros_from_path = ["macros", "shared/macros.sql"]
exclude_macros_from_path = ["macros/excluded", "shared/ignored.sql"]
"#,
            Some(&config_path),
        );

        for key in ["load_macros_from_path", "exclude_macros_from_path"] {
            let paths = config
                .raw
                .get("templater")
                .unwrap()
                .as_map()
                .unwrap()
                .get("jinja")
                .unwrap()
                .as_map()
                .unwrap()
                .get(key)
                .unwrap()
                .as_array()
                .unwrap();
            assert_eq!(paths.len(), 2);
            assert!(paths.iter().all(|value| {
                Path::new(value.as_string().unwrap()).starts_with(config_path.parent().unwrap())
            }));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_jinja_loader_search_path_resolves_toml_array() {
        let root = temp_config_dir("loader-search-path-toml");
        fs::create_dir_all(root.join("search_a")).unwrap();
        fs::create_dir_all(root.join("search_b/subdir")).unwrap();
        let config_path = root.join("pyproject.toml");
        let config = FluffConfig::from_source(
            r#"
[tool.sqruff.templater.jinja]
loader_search_path = ["search_a", "search_b/subdir"]
"#,
            Some(&config_path),
        );

        let paths = config
            .raw
            .get("templater")
            .unwrap()
            .as_map()
            .unwrap()
            .get("jinja")
            .unwrap()
            .as_map()
            .unwrap()
            .get("loader_search_path")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(
            paths[0].as_string(),
            Some(
                config_path
                    .parent()
                    .unwrap()
                    .join("search_a")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(
            paths[1].as_string(),
            Some(
                config_path
                    .parent()
                    .unwrap()
                    .join("search_b/subdir")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_templater_section_uses_typed_kind() {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
templater = placeholder

[sqruff:templater:placeholder]
param_style = colon
"#,
            None,
        );

        let section = config
            .templater_section(TemplaterKind::Placeholder)
            .unwrap();
        assert_eq!(
            section.get("param_style").unwrap().as_string(),
            Some("colon")
        );
    }

    #[test]
    fn test_sqruff_toml_parses_sqlfluff_root_config() {
        let config = FluffConfig::from_source(
            r#"
[sqlfluff]
dialect = "postgres"
templater = "placeholder"
max_line_length = 60

[sqlfluff.indentation]
indented_joins = false

[sqlfluff.layout.type.comma]
line_position = "trailing"
"#,
            Some(Path::new("sqruff.toml")),
        );

        assert_eq!(config.get_dialect().name, DialectKind::Postgres);
        assert_eq!(config.templater_kind().unwrap(), TemplaterKind::Placeholder);
        assert_eq!(config.raw["core"]["max_line_length"].as_int(), Some(60));
        assert_eq!(
            config.raw["indentation"]["indented_joins"].as_bool(),
            Some(false)
        );
        assert_eq!(
            config.raw["layout"]["type"]["comma"]["line_position"].as_string(),
            Some("trailing")
        );
    }

    #[test]
    fn test_ini_dotted_keys_create_nested_structures_with_coerced_values() {
        let configs = ConfigLoader::from_source(
            r#"
[sqruff:templater:jinja:context]
namespace.projectname = test
namespace.count = 42
namespace.ratio = 3.14
namespace.enabled = true
namespace.disabled = false
namespace.nullable = none
other.nested.key = value
simple_key = simple_value
"#,
            None,
        );

        let context = &configs["templater"]["jinja"]["context"];
        assert_eq!(
            context["namespace"]["projectname"].as_string(),
            Some("test")
        );
        assert_eq!(context["namespace"]["count"].as_int(), Some(42));
        assert_eq!(context["namespace"]["ratio"], Value::Float(3.14));
        assert_eq!(context["namespace"]["enabled"].as_bool(), Some(true));
        assert_eq!(context["namespace"]["disabled"].as_bool(), Some(false));
        assert!(context["namespace"]["nullable"].is_none());
        assert_eq!(context["other"]["nested"]["key"].as_string(), Some("value"));
        assert_eq!(context["simple_key"].as_string(), Some("simple_value"));
    }

    #[test]
    fn test_ini_dotted_keys_apply_to_all_sections() {
        let configs = ConfigLoader::from_source(
            r#"
[sqruff:rules]
some.nested.config = value
"#,
            None,
        );

        assert_eq!(
            configs["rules"]["some"]["nested"]["config"].as_string(),
            Some("value")
        );
    }

    #[test]
    fn test_pyproject_toml_parses_tool_sqlfluff_config() {
        let config = FluffConfig::from_source(
            r#"
[project]
name = "example"

[tool.sqlfluff.core]
dialect = "postgres"
templater = "placeholder"
max_line_length = 42
exclude_rules = ["CP01", "LT05"]

[tool.sqlfluff.templater.placeholder]
param_style = "colon"

[tool.sqlfluff.rules.capitalisation.keywords]
capitalisation_policy = "upper"
"#,
            Some(Path::new("pyproject.toml")),
        );

        assert_eq!(config.get_dialect().name, DialectKind::Postgres);
        assert_eq!(config.templater_kind().unwrap(), TemplaterKind::Placeholder);
        assert_eq!(config.raw["core"]["max_line_length"].as_int(), Some(42));
        assert_eq!(
            config.raw["core"]["rule_denylist"].as_array().unwrap(),
            vec![Value::String("CP01".into()), Value::String("LT05".into())]
        );
        assert_eq!(
            config
                .templater_section(TemplaterKind::Placeholder)
                .unwrap()
                .get("param_style")
                .unwrap()
                .as_string(),
            Some("colon")
        );
        assert_eq!(
            config.raw["rules"]["capitalisation.keywords"]["capitalisation_policy"].as_string(),
            Some("upper")
        );
    }

    #[test]
    fn test_pyproject_toml_coerces_array_items_to_strings() {
        let config = FluffConfig::from_source(
            r#"
[tool.sqlfluff.core]
string_values = ["one", "two"]
integer_values = [1, 2]
float_values = [1.5, 2.0]
boolean_values = [true, false]
"#,
            Some(Path::new("pyproject.toml")),
        );

        assert_eq!(
            config.raw["core"]["string_values"].as_array().unwrap(),
            vec![Value::String("one".into()), Value::String("two".into())]
        );
        assert_eq!(
            config.raw["core"]["integer_values"].as_array().unwrap(),
            vec![Value::String("1".into()), Value::String("2".into())]
        );
        assert_eq!(
            config.raw["core"]["float_values"].as_array().unwrap(),
            vec![Value::String("1.5".into()), Value::String("2.0".into())]
        );
        assert_eq!(
            config.raw["core"]["boolean_values"].as_array().unwrap(),
            vec![Value::String("True".into()), Value::String("False".into())]
        );
    }

    #[test]
    fn test_load_config_at_path_discovers_toml_configs() {
        let dir = temp_config_dir("toml-discovery");
        fs::write(
            dir.join("pyproject.toml"),
            r#"
[tool.sqlfluff.core]
max_line_length = 41
"#,
        )
        .unwrap();
        fs::write(
            dir.join("sqruff.toml"),
            r#"
[sqruff]
max_line_length = 39
"#,
        )
        .unwrap();

        let config = FluffConfig::new(ConfigLoader {}.load_config_at_path(&dir), None, None);
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(config.raw["core"]["max_line_length"].as_int(), Some(39));
    }

    #[test]
    fn test_load_config_at_path_discovers_sqruff_ini() {
        let dir = temp_config_dir("sqruff-ini");
        fs::write(
            dir.join(".sqruff.ini"),
            r#"
[sqruff]
max_line_length = 44
"#,
        )
        .unwrap();

        let config = FluffConfig::new(ConfigLoader {}.load_config_at_path(&dir), None, None);
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(config.raw["core"]["max_line_length"].as_int(), Some(44));
    }

    #[test]
    fn test_try_from_source_invalid_toml_returns_error() {
        let err = FluffConfig::try_from_source(
            "[sqlfluff]\ndialect = \"ansi\"\nrules = [1,,2]\n",
            Some(Path::new("sqruff.toml")),
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("Failed to parse TOML config file sqruff.toml"));
        assert!(message.contains("line 3"));
        assert!(message.contains("column 12"));
        assert!(!message.contains("UTF-8 BOM"));
    }

    #[test]
    fn test_try_from_source_invalid_toml_with_utf8_bom_includes_hint() {
        let err = FluffConfig::try_from_source(
            "\u{feff}[tool.sqlfluff.core]\ndialect = \"ansi\"\n",
            Some(Path::new("pyproject.toml")),
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("Failed to parse TOML config file pyproject.toml"));
        assert!(message.contains("line 1, column 1"));
        assert!(message.contains("UTF-8 BOM"));
        assert!(message.contains("UTF-8 without BOM"));
    }

    #[test]
    fn test_implicit_indents_values_are_validated() {
        for value in ["invalid", "true", "REQUIRE", "", "123"] {
            let source = format!("[sqruff:indentation]\nimplicit_indents = {value}\n");
            let err = FluffConfig::try_from_source(&source, None).unwrap_err();
            assert!(
                err.to_string()
                    .contains("set an invalid value for `implicit_indents`")
            );
            assert!(
                err.to_string()
                    .contains("Valid options are: forbid, allow, require")
            );
        }

        for value in ALLOWABLE_IMPLICIT_INDENTS_VALUES {
            let source = format!("[sqruff:indentation]\nimplicit_indents = {value}\n");
            let config = FluffConfig::try_from_source(&source, None).unwrap();
            assert_eq!(
                config.raw["indentation"]["implicit_indents"].as_string(),
                Some(value)
            );
        }
    }

    #[test]
    fn test_allow_implicit_indents_is_migrated() {
        for (old_value, expected) in [("true", "allow"), ("false", "forbid")] {
            let source = format!("[sqruff:indentation]\nallow_implicit_indents = {old_value}\n");
            let config = FluffConfig::try_from_source(&source, None).unwrap();
            let indentation = config.raw["indentation"].as_map().unwrap();
            assert_eq!(indentation["implicit_indents"].as_string(), Some(expected));
            assert!(!indentation.contains_key("allow_implicit_indents"));
        }

        let mut config = FluffConfig::default();
        config
            .process_inline_config("-- sqlfluff:indentation:allow_implicit_indents:true")
            .unwrap();
        assert_eq!(
            config.raw["indentation"]["implicit_indents"].as_string(),
            Some("allow")
        );
    }

    #[cfg(feature = "python")]
    #[test]
    fn test_templater_context_uses_typed_kind() {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
templater = python

[sqruff:templater:python:context]
blah = foo
"#,
            None,
        );

        let context = config.templater_context(TemplaterKind::Python).unwrap();
        assert_eq!(context.get("blah").unwrap().as_string(), Some("foo"));
    }
}
