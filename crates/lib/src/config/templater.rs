use std::path::{Path, PathBuf};

use hashbrown::HashMap;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};

use super::de as config_de;
use super::error::ConfigError;
use super::setting::{Merge, NullableSetting, Setting};
use crate::templaters::{PlaceholderStyle, TemplaterKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemplaterConfigSection {
    Root,
    Placeholder,
    Jinja,
    Dbt,
    Python,
    JinjaContext,
    DbtContext,
    PythonContext,
}

impl TemplaterConfigSection {
    pub(crate) fn parse(path: &[&str]) -> Option<Self> {
        match path {
            [] => Some(Self::Root),
            ["placeholder"] => Some(Self::Placeholder),
            ["jinja"] => Some(Self::Jinja),
            ["dbt"] => Some(Self::Dbt),
            ["python"] => Some(Self::Python),
            ["jinja", "context"] => Some(Self::JinjaContext),
            ["dbt", "context"] => Some(Self::DbtContext),
            ["python", "context"] => Some(Self::PythonContext),
            _ => None,
        }
    }
}

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
        section: TemplaterConfigSection,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        match section {
            TemplaterConfigSection::Root => {
                let section: TemplaterConfigRootPatch =
                    config_de::deserialize_section(section_name, values)?;
                self.unwrap_wrapped_queries
                    .merge(section.unwrap_wrapped_queries);
            }
            TemplaterConfigSection::Placeholder => {
                self.placeholder
                    .merge(config_de::deserialize_section(section_name, values)?);
            }
            TemplaterConfigSection::Jinja => {
                let mut section: JinjaTemplaterConfigPatch =
                    config_de::deserialize_section(section_name, values)?;
                section.resolve_paths(config_path, section_name)?;
                self.jinja.merge(section);
            }
            TemplaterConfigSection::Dbt => {
                let mut section: DbtTemplaterConfigPatch =
                    config_de::deserialize_section(section_name, values)?;
                section.resolve_paths(config_path, section_name)?;
                self.dbt.merge(section);
            }
            TemplaterConfigSection::Python => {
                self.python
                    .merge(config_de::deserialize_section(section_name, values)?);
            }
            TemplaterConfigSection::JinjaContext => {
                self.jinja
                    .context
                    .extend(string_context(section_name, values)?);
            }
            TemplaterConfigSection::PythonContext => {
                self.python
                    .context
                    .extend(string_context(section_name, values)?);
            }
            TemplaterConfigSection::DbtContext => {
                let context = string_context(section_name, values)?;
                self.dbt.context =
                    Setting::Set(Some(serde_json::to_string(&context).map_err(|err| {
                        ConfigError::InvalidSection {
                            section: section_name.to_string(),
                            reason: err.to_string(),
                        }
                    })?));
            }
        }
        Ok(())
    }

    pub(crate) fn merge_toml_section(
        &mut self,
        config_path: Option<&Path>,
        section: TemplaterConfigSection,
        section_name: &str,
        table: &toml::value::Table,
    ) -> Result<(), ConfigError> {
        match section {
            TemplaterConfigSection::Root => {
                let section: TemplaterConfigRootPatch =
                    config_de::deserialize_toml_table(section_name, table)?;
                self.unwrap_wrapped_queries
                    .merge(section.unwrap_wrapped_queries);
            }
            TemplaterConfigSection::Placeholder => {
                self.placeholder
                    .merge(config_de::deserialize_toml_table(section_name, table)?);
            }
            TemplaterConfigSection::Jinja => {
                let mut section: JinjaTemplaterConfigPatch =
                    config_de::deserialize_toml_table(section_name, table)?;
                section.resolve_paths(config_path, section_name)?;
                self.jinja.merge(section);
            }
            TemplaterConfigSection::Dbt => {
                let mut section: DbtTemplaterConfigPatch =
                    config_de::deserialize_toml_table(section_name, table)?;
                section.resolve_paths(config_path, section_name)?;
                self.dbt.merge(section);
            }
            TemplaterConfigSection::Python => {
                self.python
                    .merge(config_de::deserialize_toml_table(section_name, table)?);
            }
            TemplaterConfigSection::JinjaContext => {
                let context: HashMap<String, String> =
                    config_de::deserialize_toml_table(section_name, table)?;
                self.jinja.context.extend(context);
            }
            TemplaterConfigSection::PythonContext => {
                let context: HashMap<String, String> =
                    config_de::deserialize_toml_table(section_name, table)?;
                self.python.context.extend(context);
            }
            TemplaterConfigSection::DbtContext => {
                let context: HashMap<String, String> =
                    config_de::deserialize_toml_table(section_name, table)?;
                self.dbt.context =
                    Setting::Set(Some(serde_json::to_string(&context).map_err(|err| {
                        ConfigError::InvalidSection {
                            section: section_name.to_string(),
                            reason: err.to_string(),
                        }
                    })?));
            }
        }
        Ok(())
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
}

impl Merge for PythonTemplaterConfigPatch {
    fn merge(&mut self, other: Self) {
        self.context.extend(other.context);
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
    pub(crate) fn from_patch(
        kind: TemplaterKind,
        patch: &TemplaterConfigPatch,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            unwrap_wrapped_queries: patch
                .unwrap_wrapped_queries
                .clone()
                .into_option()
                .unwrap_or(false),
            placeholder: PlaceholderTemplaterConfig::from_patch(&patch.placeholder)?,
            jinja: JinjaTemplaterConfig::from_patch(&patch.jinja),
            dbt: DbtTemplaterConfig::from_patch(&patch.dbt),
            python: PythonTemplaterConfig::from_patch(&patch.python),
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
    fn from_patch(patch: &PlaceholderTemplaterConfigPatch) -> Result<Self, ConfigError> {
        Ok(Self {
            param_regex: patch.param_regex.clone().into_option().flatten(),
            param_style: patch.param_style.clone().into_option().flatten(),
            values: patch.values.clone(),
        })
    }
}

impl JinjaTemplaterConfig {
    fn from_patch(patch: &JinjaTemplaterConfigPatch) -> Self {
        Self {
            templater_paths: patch
                .templater_paths
                .clone()
                .into_option()
                .unwrap_or_default(),
            loader_search_path: patch.loader_search_path.clone().into_option().flatten(),
            apply_dbt_builtins: patch
                .apply_dbt_builtins
                .clone()
                .into_option()
                .unwrap_or(false),
            ignore_templating: patch.ignore_templating.clone().into_option().flatten(),
            library_paths: patch
                .library_paths
                .clone()
                .into_option()
                .unwrap_or_default(),
            context: patch.context.clone(),
        }
    }
}

impl DbtTemplaterConfig {
    fn from_patch(patch: &DbtTemplaterConfigPatch) -> Self {
        Self {
            profiles_dir: patch.profiles_dir.clone().into_option().flatten(),
            project_dir: patch.project_dir.clone().into_option().flatten(),
            profile: patch.profile.clone().into_option().flatten(),
            target: patch.target.clone().into_option().flatten(),
            target_path: patch.target_path.clone().into_option().flatten(),
            context: patch.context.clone().into_option().flatten(),
        }
    }
}

impl PythonTemplaterConfig {
    fn from_patch(patch: &PythonTemplaterConfigPatch) -> Self {
        Self {
            context: patch.context.clone(),
        }
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
