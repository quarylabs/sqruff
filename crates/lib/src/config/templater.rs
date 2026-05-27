use std::path::{Path, PathBuf};

use hashbrown::HashMap;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};

use super::de as config_de;
use super::error::ConfigError;
use super::raw::{RawConfig, Value};
use super::setting::{Merge, NullableSetting, Setting};
use crate::templaters::{PlaceholderStyle, TemplaterKind};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct TemplaterConfigPatch {
    pub unwrap_wrapped_queries: Setting<bool>,
    pub placeholder: PlaceholderTemplaterConfigPatch,
    pub jinja: JinjaTemplaterConfigPatch,
    pub dbt: DbtTemplaterConfigPatch,
    pub python: PythonTemplaterConfigPatch,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct PlaceholderTemplaterConfigPatch {
    pub param_regex: NullableSetting<String>,
    #[serde(default, deserialize_with = "config_de::nullable_setting_from_str")]
    pub param_style: NullableSetting<PlaceholderStyle>,
    #[serde(flatten)]
    pub values: HashMap<String, PlaceholderParamValue>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct JinjaTemplaterConfigPatch {
    pub templater_paths: Setting<Vec<PathBuf>>,
    pub loader_search_path: NullableSetting<PathBuf>,
    pub apply_dbt_builtins: Setting<bool>,
    pub ignore_templating: NullableSetting<bool>,
    pub library_paths: Setting<Vec<PathBuf>>,
    #[serde(default)]
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct DbtTemplaterConfigPatch {
    pub profiles_dir: NullableSetting<PathBuf>,
    pub project_dir: NullableSetting<PathBuf>,
    pub profile: NullableSetting<String>,
    pub target: NullableSetting<String>,
    pub target_path: NullableSetting<PathBuf>,
    pub context: NullableSetting<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct PythonTemplaterConfigPatch {
    #[serde(default)]
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceholderParamValue {
    String(String),
    Int(i32),
    Bool(bool),
}

impl PlaceholderParamValue {
    pub fn as_replacement(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Int(value) => value.to_string(),
            Self::Bool(true) => "true".to_string(),
            Self::Bool(false) => "false".to_string(),
        }
    }
}

impl<'de> Deserialize<'de> for PlaceholderParamValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(PlaceholderParamValueVisitor)
    }
}

struct PlaceholderParamValueVisitor;

impl<'de> Visitor<'de> for PlaceholderParamValueVisitor {
    type Value = PlaceholderParamValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a string, integer, or bool placeholder value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(PlaceholderParamValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value = i32::try_from(value)
            .map_err(|_| E::custom(format!("integer placeholder value out of range: {value}")))?;
        Ok(PlaceholderParamValue::Int(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value = i32::try_from(value)
            .map_err(|_| E::custom(format!("integer placeholder value out of range: {value}")))?;
        Ok(PlaceholderParamValue::Int(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(PlaceholderParamValue::String(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(PlaceholderParamValue::String(value))
    }
}

impl TemplaterConfigPatch {
    pub(crate) fn merge_section(
        &mut self,
        config_path: Option<&Path>,
        path: &[String],
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        match path {
            [] => {
                let section: TemplaterConfigRootPatch =
                    config_de::deserialize_section(section_name, values)?;
                self.unwrap_wrapped_queries
                    .merge(section.unwrap_wrapped_queries);
            }
            [name] if name == "placeholder" => {
                self.placeholder
                    .merge(config_de::deserialize_section(section_name, values)?);
            }
            [name] if name == "jinja" => {
                let mut section: JinjaTemplaterConfigPatch =
                    config_de::deserialize_section(section_name, values)?;
                section.resolve_paths(config_path, section_name)?;
                self.jinja.merge(section);
            }
            [name] if name == "dbt" => {
                let mut section: DbtTemplaterConfigPatch =
                    config_de::deserialize_section(section_name, values)?;
                section.resolve_paths(config_path, section_name)?;
                self.dbt.merge(section);
            }
            [name] if name == "python" => {
                self.python
                    .merge(config_de::deserialize_section(section_name, values)?);
            }
            [name, child] if name == "jinja" && child == "context" => {
                self.jinja
                    .context
                    .extend(string_context(section_name, values)?);
            }
            [name, child] if name == "python" && child == "context" => {
                self.python
                    .context
                    .extend(string_context(section_name, values)?);
            }
            [name, child] if name == "dbt" && child == "context" => {
                let context = string_context(section_name, values)?;
                self.dbt.context =
                    Setting::Set(Some(serde_json::to_string(&context).map_err(|err| {
                        ConfigError::InvalidSection {
                            section: section_name.to_string(),
                            reason: err.to_string(),
                        }
                    })?));
            }
            _ => {
                return Err(ConfigError::UnknownSection(section_name.to_string()));
            }
        }
        Ok(())
    }

    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        let templater = raw
            .entry("templater".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let templater_map = templater.as_map_mut().expect("templater must be a map");

        if let Setting::Set(value) = self.unwrap_wrapped_queries {
            templater_map.insert("unwrap_wrapped_queries".into(), Value::Bool(value));
        }
        self.placeholder.merge_into_raw(templater_map);
        self.jinja.merge_into_raw(templater_map);
        self.dbt.merge_into_raw(templater_map);
        self.python.merge_into_raw(templater_map);
    }
}

impl Merge for TemplaterConfigPatch {
    fn merge(&mut self, other: Self) {
        self.unwrap_wrapped_queries
            .merge(other.unwrap_wrapped_queries);
        self.placeholder.merge(other.placeholder);
        self.jinja.merge(other.jinja);
        self.dbt.merge(other.dbt);
        self.python.merge(other.python);
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct TemplaterConfigRootPatch {
    unwrap_wrapped_queries: Setting<bool>,
}

impl Merge for PlaceholderTemplaterConfigPatch {
    fn merge(&mut self, other: Self) {
        self.param_regex.merge(other.param_regex);
        self.param_style.merge(other.param_style);
        self.values.extend(other.values);
    }
}

impl PlaceholderTemplaterConfigPatch {
    fn merge_into_raw(self, templater_map: &mut HashMap<String, Value>) {
        if !self.has_values() {
            return;
        }

        let map = templater_map
            .entry("placeholder".into())
            .or_insert_with(|| Value::Map(HashMap::new()))
            .as_map_mut()
            .expect("placeholder templater config must be a map");

        merge_nullable_string(map, "param_regex", self.param_regex);
        match self.param_style {
            Setting::Unset => {}
            Setting::Set(Some(value)) => {
                map.insert("param_style".into(), Value::String(value.as_str().into()));
            }
            Setting::Set(None) => {
                map.insert("param_style".into(), Value::None);
            }
        }
        for (key, value) in self.values {
            map.insert(key, value.into_value());
        }
    }

    fn has_values(&self) -> bool {
        self.param_regex.is_set() || self.param_style.is_set() || !self.values.is_empty()
    }
}

impl PlaceholderParamValue {
    fn into_value(self) -> Value {
        match self {
            Self::String(value) => Value::String(value.into()),
            Self::Int(value) => Value::Int(value),
            Self::Bool(value) => Value::Bool(value),
        }
    }
}

impl Merge for JinjaTemplaterConfigPatch {
    fn merge(&mut self, other: Self) {
        self.templater_paths.merge(other.templater_paths);
        self.loader_search_path.merge(other.loader_search_path);
        self.apply_dbt_builtins.merge(other.apply_dbt_builtins);
        self.ignore_templating.merge(other.ignore_templating);
        self.library_paths.merge(other.library_paths);
        self.context.extend(other.context);
    }
}

impl JinjaTemplaterConfigPatch {
    fn resolve_paths(
        &mut self,
        config_path: Option<&Path>,
        section_name: &str,
    ) -> Result<(), ConfigError> {
        resolve_path_list_setting(
            config_path,
            section_name,
            "templater_paths",
            &mut self.templater_paths,
        )?;
        resolve_path_setting(
            config_path,
            section_name,
            "loader_search_path",
            &mut self.loader_search_path,
        )?;
        resolve_path_list_setting(
            config_path,
            section_name,
            "library_paths",
            &mut self.library_paths,
        )?;
        Ok(())
    }

    fn merge_into_raw(self, templater_map: &mut HashMap<String, Value>) {
        if !self.has_values() {
            return;
        }

        let map = templater_map
            .entry("jinja".into())
            .or_insert_with(|| Value::Map(HashMap::new()))
            .as_map_mut()
            .expect("jinja templater config must be a map");

        merge_path_list(map, "templater_paths", self.templater_paths);
        merge_nullable_path(map, "loader_search_path", self.loader_search_path);
        if let Setting::Set(value) = self.apply_dbt_builtins {
            map.insert("apply_dbt_builtins".into(), Value::Bool(value));
        }
        match self.ignore_templating {
            Setting::Unset => {}
            Setting::Set(Some(value)) => {
                map.insert("ignore_templating".into(), Value::Bool(value));
            }
            Setting::Set(None) => {
                map.insert("ignore_templating".into(), Value::None);
            }
        }
        merge_path_list(map, "library_paths", self.library_paths);
        if !self.context.is_empty() {
            map.insert("context".into(), string_map_value(self.context));
        }
    }

    fn has_values(&self) -> bool {
        self.templater_paths.is_set()
            || self.loader_search_path.is_set()
            || self.apply_dbt_builtins.is_set()
            || self.ignore_templating.is_set()
            || self.library_paths.is_set()
            || !self.context.is_empty()
    }
}

impl Merge for DbtTemplaterConfigPatch {
    fn merge(&mut self, other: Self) {
        self.profiles_dir.merge(other.profiles_dir);
        self.project_dir.merge(other.project_dir);
        self.profile.merge(other.profile);
        self.target.merge(other.target);
        self.target_path.merge(other.target_path);
        self.context.merge(other.context);
    }
}

impl DbtTemplaterConfigPatch {
    fn resolve_paths(
        &mut self,
        config_path: Option<&Path>,
        section_name: &str,
    ) -> Result<(), ConfigError> {
        resolve_path_setting(
            config_path,
            section_name,
            "profiles_dir",
            &mut self.profiles_dir,
        )?;
        resolve_path_setting(
            config_path,
            section_name,
            "project_dir",
            &mut self.project_dir,
        )?;
        resolve_path_setting(
            config_path,
            section_name,
            "target_path",
            &mut self.target_path,
        )?;
        Ok(())
    }

    fn merge_into_raw(self, templater_map: &mut HashMap<String, Value>) {
        if !self.has_values() {
            return;
        }

        let map = templater_map
            .entry("dbt".into())
            .or_insert_with(|| Value::Map(HashMap::new()))
            .as_map_mut()
            .expect("dbt templater config must be a map");

        merge_nullable_path(map, "profiles_dir", self.profiles_dir);
        merge_nullable_path(map, "project_dir", self.project_dir);
        merge_nullable_string(map, "profile", self.profile);
        merge_nullable_string(map, "target", self.target);
        merge_nullable_path(map, "target_path", self.target_path);
        merge_nullable_string(map, "context", self.context);
    }

    fn has_values(&self) -> bool {
        self.profiles_dir.is_set()
            || self.project_dir.is_set()
            || self.profile.is_set()
            || self.target.is_set()
            || self.target_path.is_set()
            || self.context.is_set()
    }
}

impl Merge for PythonTemplaterConfigPatch {
    fn merge(&mut self, other: Self) {
        self.context.extend(other.context);
    }
}

impl PythonTemplaterConfigPatch {
    fn merge_into_raw(self, templater_map: &mut HashMap<String, Value>) {
        if self.context.is_empty() {
            return;
        }

        let map = templater_map
            .entry("python".into())
            .or_insert_with(|| Value::Map(HashMap::new()))
            .as_map_mut()
            .expect("python templater config must be a map");
        map.insert("context".into(), string_map_value(self.context));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplaterConfig {
    pub unwrap_wrapped_queries: bool,
    pub placeholder: PlaceholderTemplaterConfig,
    pub jinja: JinjaTemplaterConfig,
    pub dbt: DbtTemplaterConfig,
    pub python: PythonTemplaterConfig,
    kind: TemplaterKind,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaceholderTemplaterConfig {
    pub param_regex: Option<String>,
    pub param_style: Option<PlaceholderStyle>,
    pub values: HashMap<String, PlaceholderParamValue>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JinjaTemplaterConfig {
    pub templater_paths: Vec<PathBuf>,
    pub loader_search_path: Option<PathBuf>,
    pub apply_dbt_builtins: bool,
    pub ignore_templating: Option<bool>,
    pub library_paths: Vec<PathBuf>,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DbtTemplaterConfig {
    pub profiles_dir: Option<PathBuf>,
    pub project_dir: Option<PathBuf>,
    pub profile: Option<String>,
    pub target: Option<String>,
    pub target_path: Option<PathBuf>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PythonTemplaterConfig {
    pub context: HashMap<String, String>,
}

impl TemplaterConfig {
    pub(crate) fn from_raw(raw: &RawConfig) -> Result<Self, ConfigError> {
        let kind = raw["core"]["templater"]
            .as_string()
            .map(TemplaterKind::from_name)
            .transpose()
            .map_err(ConfigError::UnsupportedTemplater)?
            .unwrap_or(TemplaterKind::Raw);

        let values = raw["templater"].as_map().cloned().unwrap_or_default();

        Ok(Self {
            unwrap_wrapped_queries: bool_value(&values, "unwrap_wrapped_queries", false)?,
            placeholder: PlaceholderTemplaterConfig::from_value(values.get("placeholder"))?,
            jinja: JinjaTemplaterConfig::from_value(values.get("jinja"))?,
            dbt: DbtTemplaterConfig::from_value(values.get("dbt"))?,
            python: PythonTemplaterConfig::from_value(values.get("python"))?,
            kind,
        })
    }

    pub fn kind(&self) -> TemplaterKind {
        self.kind
    }

    pub fn context(&self, templater: TemplaterKind) -> Option<&HashMap<String, String>> {
        match templater {
            TemplaterKind::Placeholder | TemplaterKind::Raw => None,
            #[cfg(feature = "python")]
            TemplaterKind::Jinja => Some(&self.jinja.context),
            #[cfg(feature = "python")]
            TemplaterKind::Dbt => None,
            #[cfg(feature = "python")]
            TemplaterKind::Python => Some(&self.python.context),
        }
    }
}

impl PlaceholderTemplaterConfig {
    fn from_value(value: Option<&Value>) -> Result<Self, ConfigError> {
        let map = optional_map(value, "placeholder")?;
        let mut cfg = Self::default();

        for (key, value) in map {
            match key.as_str() {
                "param_regex" => cfg.param_regex = nullable_string_value(value, "param_regex")?,
                "param_style" => {
                    cfg.param_style = nullable_string_value(value, "param_style")?
                        .map(|value| PlaceholderStyle::from_name(&value))
                        .transpose()
                        .map_err(|reason| ConfigError::InvalidField {
                            field: "param_style",
                            reason,
                        })?;
                }
                _ => {
                    cfg.values
                        .insert(key.clone(), placeholder_param_value(key, value)?);
                }
            }
        }

        Ok(cfg)
    }
}

impl JinjaTemplaterConfig {
    fn from_value(value: Option<&Value>) -> Result<Self, ConfigError> {
        let map = optional_map(value, "jinja")?;
        Ok(Self {
            templater_paths: path_list_value(map, "templater_paths")?,
            loader_search_path: nullable_path_value(map, "loader_search_path")?,
            apply_dbt_builtins: bool_value(map, "apply_dbt_builtins", false)?,
            ignore_templating: nullable_bool_value(map, "ignore_templating")?,
            library_paths: path_list_value(map, "library_paths")?,
            context: context_value(map, "context")?,
        })
    }
}

impl DbtTemplaterConfig {
    fn from_value(value: Option<&Value>) -> Result<Self, ConfigError> {
        let map = optional_map(value, "dbt")?;
        Ok(Self {
            profiles_dir: nullable_path_value(map, "profiles_dir")?,
            project_dir: nullable_path_value(map, "project_dir")?,
            profile: nullable_string_value_from_map(map, "profile")?,
            target: nullable_string_value_from_map(map, "target")?,
            target_path: nullable_path_value(map, "target_path")?,
            context: nullable_string_value_from_map(map, "context")?,
        })
    }
}

impl PythonTemplaterConfig {
    fn from_value(value: Option<&Value>) -> Result<Self, ConfigError> {
        let map = optional_map(value, "python")?;
        Ok(Self {
            context: context_value(map, "context")?,
        })
    }
}

fn optional_map<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a HashMap<String, Value>, ConfigError> {
    match value {
        Some(value) => value.as_map().ok_or_else(|| ConfigError::InvalidField {
            field,
            reason: "expected map".to_string(),
        }),
        None => Ok(empty_map()),
    }
}

fn empty_map() -> &'static HashMap<String, Value> {
    static EMPTY: std::sync::LazyLock<HashMap<String, Value>> =
        std::sync::LazyLock::new(HashMap::new);
    &EMPTY
}

fn bool_value(
    map: &HashMap<String, Value>,
    key: &'static str,
    default: bool,
) -> Result<bool, ConfigError> {
    match map.get(key) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::None) | None => Ok(default),
        Some(_) => Err(ConfigError::InvalidField {
            field: key,
            reason: "expected bool".to_string(),
        }),
    }
}

fn nullable_bool_value(
    map: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Option<bool>, ConfigError> {
    match map.get(key) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(Value::None) | None => Ok(None),
        Some(_) => Err(ConfigError::InvalidField {
            field: key,
            reason: "expected bool or None".to_string(),
        }),
    }
}

fn nullable_string_value_from_map(
    map: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Option<String>, ConfigError> {
    match map.get(key) {
        Some(value) => nullable_string_value(value, key),
        None => Ok(None),
    }
}

fn nullable_string_value(
    value: &Value,
    field: &'static str,
) -> Result<Option<String>, ConfigError> {
    match value {
        Value::String(value) => Ok(Some(value.to_string())),
        Value::None => Ok(None),
        _ => Err(ConfigError::InvalidField {
            field,
            reason: "expected string or None".to_string(),
        }),
    }
}

fn nullable_path_value(
    map: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Option<PathBuf>, ConfigError> {
    nullable_string_value_from_map(map, key)?
        .map(|value| valid_path(key, &value).map(PathBuf::from))
        .transpose()
}

fn path_list_value(
    map: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Vec<PathBuf>, ConfigError> {
    match map.get(key) {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => valid_path(key, value).map(PathBuf::from),
                _ => Err(ConfigError::InvalidField {
                    field: key,
                    reason: "expected string path".to_string(),
                }),
            })
            .collect(),
        Some(Value::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| valid_path(key, value).map(PathBuf::from))
            .collect(),
        Some(Value::None) | None => Ok(Vec::new()),
        Some(_) => Err(ConfigError::InvalidField {
            field: key,
            reason: "expected string path list".to_string(),
        }),
    }
}

fn context_value(
    map: &HashMap<String, Value>,
    key: &'static str,
) -> Result<HashMap<String, String>, ConfigError> {
    let Some(value) = map.get(key) else {
        return Ok(HashMap::new());
    };
    let map = value.as_map().ok_or_else(|| ConfigError::InvalidField {
        field: key,
        reason: "expected map".to_string(),
    })?;
    map.iter()
        .map(|(key, value)| {
            let value = value.as_string().ok_or_else(|| ConfigError::InvalidField {
                field: "context",
                reason: format!("context value '{key}' must be a string"),
            })?;
            Ok((key.clone(), value.to_string()))
        })
        .collect()
}

fn placeholder_param_value(key: &str, value: &Value) -> Result<PlaceholderParamValue, ConfigError> {
    match value {
        Value::String(value) => Ok(PlaceholderParamValue::String(value.to_string())),
        Value::Int(value) => Ok(PlaceholderParamValue::Int(*value)),
        Value::Bool(value) => Ok(PlaceholderParamValue::Bool(*value)),
        _ => Err(ConfigError::InvalidField {
            field: "placeholder",
            reason: format!("invalid value for placeholder parameter '{key}'"),
        }),
    }
}

fn string_context(
    section_name: &str,
    values: &std::collections::HashMap<String, Option<String>>,
) -> Result<HashMap<String, String>, ConfigError> {
    values
        .iter()
        .map(|(key, value)| {
            let Some(value) = value else {
                return Err(ConfigError::InvalidSection {
                    section: section_name.to_string(),
                    reason: format!("context value '{key}' must be a string"),
                });
            };
            Ok((key.clone(), value.clone()))
        })
        .collect()
}

fn resolve_path_setting(
    config_path: Option<&Path>,
    section_name: &str,
    key: &'static str,
    setting: &mut NullableSetting<PathBuf>,
) -> Result<(), ConfigError> {
    let Setting::Set(Some(path)) = setting else {
        return Ok(());
    };
    validate_path(section_name, key, path)?;
    *path = resolve_path(config_path, path);
    Ok(())
}

fn resolve_path_list_setting(
    config_path: Option<&Path>,
    section_name: &str,
    key: &'static str,
    setting: &mut Setting<Vec<PathBuf>>,
) -> Result<(), ConfigError> {
    let Setting::Set(paths) = setting else {
        return Ok(());
    };
    for path in paths {
        validate_path(section_name, key, path)?;
        *path = resolve_path(config_path, path);
    }
    Ok(())
}

fn validate_path(section_name: &str, key: &'static str, path: &Path) -> Result<(), ConfigError> {
    let raw = path.to_string_lossy();
    if raw.trim().is_empty()
        || raw.trim().eq_ignore_ascii_case("none")
        || raw.trim().parse::<i32>().is_ok()
        || raw.trim().eq_ignore_ascii_case("true")
        || raw.trim().eq_ignore_ascii_case("false")
    {
        return Err(ConfigError::InvalidSection {
            section: section_name.to_string(),
            reason: format!("invalid path value for config key '{key}'"),
        });
    }
    Ok(())
}

fn valid_path<'a>(key: &'static str, value: &'a str) -> Result<&'a str, ConfigError> {
    if value.trim().is_empty()
        || value.trim().eq_ignore_ascii_case("none")
        || value.trim().parse::<i32>().is_ok()
        || value.trim().eq_ignore_ascii_case("true")
        || value.trim().eq_ignore_ascii_case("false")
    {
        return Err(ConfigError::InvalidField {
            field: key,
            reason: "invalid path value".to_string(),
        });
    }
    Ok(value)
}

fn resolve_path(config_path: Option<&Path>, path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    if let Some(config_path) = config_path.and_then(Path::parent)
        && let Ok(current_dir) = std::env::current_dir()
        && let Ok(config_path) = std::path::absolute(current_dir.join(config_path))
    {
        return config_path.join(path);
    }

    path.to_path_buf()
}

fn merge_nullable_string(
    map: &mut HashMap<String, Value>,
    key: &'static str,
    value: NullableSetting<String>,
) {
    match value {
        Setting::Unset => {}
        Setting::Set(Some(value)) => {
            map.insert(key.into(), Value::String(value.into()));
        }
        Setting::Set(None) => {
            map.insert(key.into(), Value::None);
        }
    }
}

fn merge_nullable_path(
    map: &mut HashMap<String, Value>,
    key: &'static str,
    value: NullableSetting<PathBuf>,
) {
    match value {
        Setting::Unset => {}
        Setting::Set(Some(value)) => {
            map.insert(key.into(), Value::String(value.to_string_lossy().into()));
        }
        Setting::Set(None) => {
            map.insert(key.into(), Value::None);
        }
    }
}

fn merge_path_list(
    map: &mut HashMap<String, Value>,
    key: &'static str,
    value: Setting<Vec<PathBuf>>,
) {
    if let Setting::Set(values) = value {
        map.insert(
            key.into(),
            Value::Array(
                values
                    .into_iter()
                    .map(|value| Value::String(value.to_string_lossy().into()))
                    .collect(),
            ),
        );
    }
}

fn string_map_value(values: HashMap<String, String>) -> Value {
    Value::Map(
        values
            .into_iter()
            .map(|(key, value)| (key, Value::String(value.into())))
            .collect(),
    )
}
