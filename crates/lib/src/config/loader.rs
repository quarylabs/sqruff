use std::path::{Path, PathBuf};
use std::str::FromStr;

use configparser::ini::Ini;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;

use super::de;
use super::error::ConfigError;
use super::layout::LayoutTypeConfigPatch;
use super::model::FluffConfig;
use super::options::{ConfigInput, ConfigLoadOptions, ConfigOverrides};
use super::patch::ConfigPatch;
use super::raw::Value;
use super::setting::{Merge, Setting};
use crate::api::SqruffError;

pub struct ConfigLoader;

enum SectionPath {
    Core,
    Indentation,
    LayoutType(String),
    Templater(Vec<String>),
    RulesGlobal,
    Rule(String),
    Dialect(String),
}

impl SectionPath {
    fn parse(section_name: &str) -> Result<Self, ConfigError> {
        if section_name == "sqlfluff" || section_name == "sqruff" {
            return Ok(Self::Core);
        }

        let Some(path) = section_name
            .strip_prefix("sqlfluff:")
            .or_else(|| section_name.strip_prefix("sqruff:"))
        else {
            return Err(ConfigError::UnknownSection(section_name.to_string()));
        };

        let parts = path.split(':').collect::<Vec<_>>();
        match parts.as_slice() {
            ["indentation"] => Ok(Self::Indentation),
            ["layout", "type", kind] => {
                kind.parse::<SyntaxKind>()
                    .map_err(|_| ConfigError::InvalidSection {
                        section: section_name.to_string(),
                        reason: format!("invalid layout syntax kind '{kind}'"),
                    })?;
                Ok(Self::LayoutType((*kind).to_string()))
            }
            ["templater"] => Ok(Self::Templater(Vec::new())),
            ["templater", rest @ ..] if !rest.is_empty() => Ok(Self::Templater(
                rest.iter().map(|part| (*part).to_string()).collect(),
            )),
            ["rules"] => Ok(Self::RulesGlobal),
            ["rules", rule_section] => Ok(Self::Rule((*rule_section).to_string())),
            ["dialect", dialect] => {
                let dialect = DialectKind::from_str(dialect)
                    .map_err(|_| ConfigError::UnknownDialect((*dialect).to_string()))?;
                Ok(Self::Dialect(dialect.as_ref().to_string()))
            }
            _ => Err(ConfigError::UnknownSection(section_name.to_string())),
        }
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn load(&self, options: ConfigLoadOptions) -> Result<FluffConfig, SqruffError> {
        let mut patch = match options.input {
            ConfigInput::ProjectRoot(path) => {
                self.try_load_config_up_to_path(path, None, options.ignore_local_config)?
            }
            ConfigInput::File(path) => ConfigLoader::try_load_config_file(path)?,
            ConfigInput::Source { text, path } => Self::try_from_source(&text, path.as_deref())?,
            ConfigInput::Default => ConfigPatch::default(),
        };

        apply_overrides(&mut patch, options.overrides);

        FluffConfig::try_from_patch(patch)
    }

    #[allow(unused_variables)]
    fn iter_config_locations_up_to_path(
        path: &Path,
        working_path: Option<&Path>,
        ignore_local_config: bool,
    ) -> impl Iterator<Item = PathBuf> {
        let mut given_path = std::path::absolute(path).unwrap();
        let working_path = std::env::current_dir().unwrap();

        if !given_path.is_dir() {
            given_path = given_path.parent().unwrap().into();
        }

        let common_path = common_path::common_path(&given_path, working_path).unwrap();
        let mut path_to_visit = common_path;

        let head = Some(given_path.canonicalize().unwrap()).into_iter();
        let tail = std::iter::from_fn(move || {
            if path_to_visit != given_path {
                let path = path_to_visit.canonicalize().unwrap();

                let next_path_to_visit = {
                    let path_to_visit_as_path = path_to_visit.as_path();
                    let given_path_as_path = given_path.as_path();

                    match given_path_as_path.strip_prefix(path_to_visit_as_path) {
                        Ok(relative_path) => {
                            if let Some(first_part) = relative_path.components().next() {
                                path_to_visit.join(first_part.as_os_str())
                            } else {
                                path_to_visit.clone()
                            }
                        }
                        Err(_) => path_to_visit.clone(),
                    }
                };

                if next_path_to_visit == path_to_visit {
                    return None;
                }

                path_to_visit = next_path_to_visit;

                Some(path)
            } else {
                None
            }
        });

        head.chain(tail)
    }

    pub fn try_load_config_up_to_path(
        &self,
        path: impl AsRef<Path>,
        extra_config_path: Option<String>,
        ignore_local_config: bool,
    ) -> Result<ConfigPatch, SqruffError> {
        let path = path.as_ref();

        let config_stack = if ignore_local_config {
            match extra_config_path {
                Some(path) => vec![self.try_load_config_at_path(path)?],
                None => Vec::new(),
            }
        } else {
            let configs = Self::iter_config_locations_up_to_path(path, None, ignore_local_config);
            configs
                .map(|path| self.try_load_config_at_path(path))
                .collect::<Result<Vec<_>, _>>()?
        };

        Ok(merge_patches(config_stack))
    }

    pub fn try_load_config_at_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ConfigPatch, SqruffError> {
        let path = path.as_ref();

        let filename_options = [".sqlfluff", ".sqruff"];
        let mut patch = ConfigPatch::default();

        if path.is_dir() {
            for fname in filename_options {
                let path = path.join(fname);
                if path.exists() {
                    patch.merge(ConfigLoader::try_load_config_file(path)?);
                }
            }
        } else if path.is_file() {
            patch.merge(ConfigLoader::try_load_config_file(path)?);
        };

        Ok(patch)
    }

    pub fn try_from_source(source: &str, path: Option<&Path>) -> Result<ConfigPatch, SqruffError> {
        Self::patch_from_ini(path, Some(source))
    }

    pub fn try_load_config_file(path: impl AsRef<Path>) -> Result<ConfigPatch, SqruffError> {
        Self::patch_from_ini(path.as_ref().into(), None)
    }

    pub(crate) fn patch_from_ini(
        config_path: Option<&Path>,
        config_string: Option<&str>,
    ) -> Result<ConfigPatch, SqruffError> {
        let mut config = Ini::new();

        let content = match (config_path, config_string) {
            (None, None) | (Some(_), Some(_)) => {
                return Err(SqruffError::Config(
                    "one of config path or config string is required".to_string(),
                ));
            }
            (None, Some(text)) => text.to_owned(),
            (Some(path), None) => {
                std::fs::read_to_string(path).map_err(|source| SqruffError::Io {
                    path: path.to_path_buf(),
                    source,
                })?
            }
        };

        config
            .read(join_ini_continuations(&content))
            .map_err(SqruffError::Config)?;
        let config_map = config.get_map_ref();
        let mut patch = ConfigPatch::default();

        let mut sections = Vec::new();
        for section_name in config.sections() {
            let section_path = SectionPath::parse(&section_name).map_err(config_error)?;
            let Some(values) = config_map.get(&section_name) else {
                continue;
            };
            let values = normalize_section_values(config_path, &section_name, values)?;
            sections.push((section_name, section_path, values));
        }

        for (section_name, section_path, values) in sections {
            match section_path {
                SectionPath::Core => {
                    patch.core.merge(
                        de::deserialize_section(&section_name, &values).map_err(config_error)?,
                    );
                }
                SectionPath::Indentation => {
                    patch.indentation.merge(
                        de::deserialize_section(&section_name, &values).map_err(config_error)?,
                    );
                }
                SectionPath::LayoutType(kind) => {
                    let section: LayoutTypeConfigPatch =
                        de::deserialize_section(&section_name, &values).map_err(config_error)?;
                    patch.layout.types.entry(kind).or_default().merge(section);
                }
                SectionPath::Templater(path) => {
                    patch
                        .templater
                        .merge_section(&path, &section_name, &values)
                        .map_err(config_error)?;
                }
                SectionPath::RulesGlobal => {
                    patch
                        .rules
                        .merge_global(&section_name, &values)
                        .map_err(config_error)?;
                }
                SectionPath::Rule(rule_section) => {
                    patch
                        .rules
                        .merge_rule_section(rule_section, &section_name, &values)
                        .map_err(config_error)?;
                }
                SectionPath::Dialect(dialect) => {
                    patch.dialects.dialects.insert(
                        dialect,
                        Value::Map(
                            de::deserialize_value_map(&section_name, &values)
                                .map_err(config_error)?,
                        ),
                    );
                }
            }
        }

        Ok(patch)
    }
}

fn normalize_section_values(
    config_path: Option<&Path>,
    section_name: &str,
    values: &std::collections::HashMap<String, Option<String>>,
) -> Result<std::collections::HashMap<String, Option<String>>, SqruffError> {
    let mut normalized = std::collections::HashMap::new();

    for (name, value) in values {
        let name_lowercase = name.to_lowercase();

        if name_lowercase == "load_macros_from_path" {
            return Err(SqruffError::Unsupported(
                "load_macros_from_path config is not implemented",
            ));
        }

        let mut value = value.clone();
        if name_lowercase.ends_with("_path") || name_lowercase.ends_with("_dir") {
            let Some(path_value) = value.as_deref() else {
                return Err(SqruffError::Config(format!(
                    "invalid path value for config key '{name}' in section '{section_name}'"
                )));
            };
            if path_value.trim().is_empty() || path_value.trim().eq_ignore_ascii_case("none") {
                return Err(SqruffError::Config(format!(
                    "invalid path value for config key '{name}' in section '{section_name}'"
                )));
            }
            if path_value.trim().parse::<i32>().is_ok()
                || path_value.trim().eq_ignore_ascii_case("true")
                || path_value.trim().eq_ignore_ascii_case("false")
            {
                return Err(SqruffError::Config(format!(
                    "invalid path value for config key '{name}' in section '{section_name}'"
                )));
            }
            let path = PathBuf::from(path_value);
            if !path.is_absolute()
                && let Some(config_path) = config_path.and_then(Path::parent)
                && let Ok(current_dir) = std::env::current_dir()
                && let Ok(config_path) = std::path::absolute(current_dir.join(config_path))
            {
                let path = config_path.join(path);
                value = Some(path.to_string_lossy().into());
            }
        }

        normalized.insert(name.clone(), value);
    }

    Ok(normalized)
}

fn join_ini_continuations(content: &str) -> String {
    let mut lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if (line.starts_with(' ') || line.starts_with('\t'))
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with(';')
        {
            if let Some(previous) = lines.last_mut() {
                previous.push_str(trimmed);
            } else {
                lines.push(trimmed.to_string());
            }
        } else {
            lines.push(line.to_string());
        }
    }

    lines.join("\n")
}

fn merge_patches(config_stack: Vec<ConfigPatch>) -> ConfigPatch {
    config_stack
        .into_iter()
        .fold(ConfigPatch::default(), |mut acc, patch| {
            acc.merge(patch);
            acc
        })
}

fn apply_overrides(config: &mut ConfigPatch, overrides: ConfigOverrides) {
    if let Some(dialect) = overrides.dialect {
        config.core.dialect = Setting::Set(Some(dialect));
    }

    if let Some(rules) = overrides.rules {
        config.core.rules = Setting::Set(Some(rules));
    }

    if let Some(exclude_rules) = overrides.exclude_rules {
        config.core.exclude_rules = Setting::Set(Some(exclude_rules));
    }
}

fn config_error(error: ConfigError) -> SqruffError {
    SqruffError::Config(error.to_string())
}
