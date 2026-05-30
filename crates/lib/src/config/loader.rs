use std::path::{Path, PathBuf};
use std::str::FromStr;

use configparser::ini::Ini;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;

use super::de;
use super::error::ConfigError;
use super::layout::{LayoutTypeConfigPatch, parse_layout_syntax_kind};
use super::model::FluffConfig;
use super::options::{ConfigInput, ConfigLoadOptions, ConfigOverrides};
use super::patch::ConfigPatch;
use super::rules::RuleConfigSection;
use super::setting::Merge;
use super::templater::TemplaterConfigSection;
use crate::api::SqruffError;

pub struct ConfigLoader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Ini,
    Toml,
}

impl ConfigFormat {
    pub fn from_path(path: &Path) -> Result<Self, SqruffError> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("toml") => Ok(Self::Toml),
            _ => Ok(Self::Ini),
        }
    }
}

enum SectionPath {
    Core,
    Indentation,
    LayoutType(SyntaxKind),
    Templater(TemplaterConfigSection),
    RulesGlobal,
    Rule(RuleConfigSection),
    Dialect(DialectKind),
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
                let kind =
                    parse_layout_syntax_kind(kind).ok_or_else(|| ConfigError::InvalidSection {
                        section: section_name.to_string(),
                        reason: format!("invalid layout syntax kind '{kind}'"),
                    })?;
                Ok(Self::LayoutType(kind))
            }
            ["templater"] => Ok(Self::Templater(TemplaterConfigSection::Root)),
            ["templater", rest @ ..] if !rest.is_empty() => TemplaterConfigSection::parse(rest)
                .map(Self::Templater)
                .ok_or_else(|| ConfigError::UnknownSection(section_name.to_string())),
            ["rules"] => Ok(Self::RulesGlobal),
            ["rules", rule_section] => RuleConfigSection::from_str(rule_section)
                .map(Self::Rule)
                .map_err(|_| ConfigError::UnknownSection(section_name.to_string())),
            ["dialect", dialect] => {
                let dialect = DialectKind::from_str(dialect)
                    .map_err(|_| ConfigError::UnknownDialect((*dialect).to_string()))?;
                Ok(Self::Dialect(dialect))
            }
            _ => Err(ConfigError::UnknownSection(section_name.to_string())),
        }
    }

    fn parse_toml(path: &[&str]) -> Result<Self, ConfigError> {
        match path {
            [] | ["core"] => Ok(Self::Core),
            ["indentation"] => Ok(Self::Indentation),
            ["layout", "type", kind] => {
                let kind =
                    parse_layout_syntax_kind(kind).ok_or_else(|| ConfigError::InvalidSection {
                        section: toml_section_name(path),
                        reason: format!("invalid layout syntax kind '{kind}'"),
                    })?;
                Ok(Self::LayoutType(kind))
            }
            ["templater"] => Ok(Self::Templater(TemplaterConfigSection::Root)),
            ["templater", rest @ ..] if !rest.is_empty() => TemplaterConfigSection::parse(rest)
                .map(Self::Templater)
                .ok_or_else(|| ConfigError::UnknownSection(toml_section_name(path))),
            ["rules"] => Ok(Self::RulesGlobal),
            ["rules", rest @ ..] if !rest.is_empty() => {
                let rule_section = rest.join(".");
                RuleConfigSection::from_str(&rule_section)
                    .map(Self::Rule)
                    .map_err(|_| ConfigError::UnknownSection(toml_section_name(path)))
            }
            ["dialect", dialect] => {
                let dialect = DialectKind::from_str(dialect)
                    .map_err(|_| ConfigError::UnknownDialect((*dialect).to_string()))?;
                Ok(Self::Dialect(dialect))
            }
            _ => Err(ConfigError::UnknownSection(toml_section_name(path))),
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

    pub fn load_patch(&self, patch: ConfigPatch) -> Result<FluffConfig, SqruffError> {
        FluffConfig::try_from_patch(patch)
    }

    fn iter_config_locations_up_to_path(
        path: &Path,
        working_path: Option<&Path>,
        ignore_local_config: bool,
    ) -> impl Iterator<Item = PathBuf> {
        let mut given_path = std::path::absolute(path).unwrap();
        let working_path = working_path
            .map(|path| std::path::absolute(path).unwrap())
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        if !given_path.is_dir() {
            given_path = given_path.parent().unwrap().into();
        }

        let common_path = common_path::common_path(&given_path, working_path).unwrap();
        let mut path_to_visit = common_path;
        let mut locations = Vec::new();

        loop {
            locations.push(path_to_visit.clone());
            if path_to_visit == given_path || ignore_local_config {
                break;
            }

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
                break;
            }
            path_to_visit = next_path_to_visit;
        }

        locations.into_iter()
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

        let filename_options = [
            "setup.cfg",
            "tox.ini",
            "pep8.ini",
            ".sqlfluff",
            ".sqruff",
            "pyproject.toml",
        ];
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
        let format = path
            .map(ConfigFormat::from_path)
            .transpose()?
            .unwrap_or(ConfigFormat::Ini);
        Self::patch_from_source(source, path, format)
    }

    pub fn try_load_config_file(path: impl AsRef<Path>) -> Result<ConfigPatch, SqruffError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|source| SqruffError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let format = ConfigFormat::from_path(path)?;
        Self::patch_from_source(&content, Some(path), format)
    }

    pub(crate) fn patch_from_source(
        source: &str,
        path: Option<&Path>,
        format: ConfigFormat,
    ) -> Result<ConfigPatch, SqruffError> {
        match format {
            ConfigFormat::Ini => Self::patch_from_ini(path, Some(source)),
            ConfigFormat::Toml => Self::patch_from_toml(path, source),
        }
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
            let values = normalize_section_values(values)?;
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
                        .merge_section(config_path, path, &section_name, &values)
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
                    patch
                        .dialects
                        .merge_section(dialect, &section_name, &values)
                        .map_err(config_error)?;
                }
            }
        }

        Ok(patch)
    }

    pub(crate) fn patch_from_toml(
        config_path: Option<&Path>,
        source: &str,
    ) -> Result<ConfigPatch, SqruffError> {
        let value: toml::Value =
            toml::from_str(source).map_err(|err| SqruffError::Config(err.to_string()))?;
        let Some(root) = find_toml_root(&value) else {
            return Ok(ConfigPatch::default());
        };

        let mut patch = ConfigPatch::default();
        merge_toml_table(config_path, &mut patch, &[], root).map_err(config_error)?;
        Ok(patch)
    }
}

fn find_toml_root(value: &toml::Value) -> Option<&toml::value::Table> {
    let tool = value.get("tool")?;
    tool.get("sqruff")
        .or_else(|| tool.get("sqlfluff"))?
        .as_table()
}

fn merge_toml_table(
    config_path: Option<&Path>,
    patch: &mut ConfigPatch,
    path: &[&str],
    table: &toml::value::Table,
) -> Result<(), ConfigError> {
    let leaf_table = table
        .iter()
        .filter(|(_, value)| !value.is_table())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<toml::value::Table>();

    if !leaf_table.is_empty() {
        merge_toml_leaf(config_path, patch, path, &leaf_table)?;
    }

    for (key, value) in table {
        let Some(table) = value.as_table() else {
            continue;
        };
        let mut next_path = path.to_vec();
        next_path.push(key);
        merge_toml_table(config_path, patch, &next_path, table)?;
    }

    Ok(())
}

fn merge_toml_leaf(
    config_path: Option<&Path>,
    patch: &mut ConfigPatch,
    path: &[&str],
    table: &toml::value::Table,
) -> Result<(), ConfigError> {
    let section_name = toml_section_name(path);

    match SectionPath::parse_toml(path)? {
        SectionPath::Core => {
            patch
                .core
                .merge(de::deserialize_toml_table(&section_name, table)?);
        }
        SectionPath::Indentation => {
            patch
                .indentation
                .merge(de::deserialize_toml_table(&section_name, table)?);
        }
        SectionPath::LayoutType(kind) => {
            let section: LayoutTypeConfigPatch = de::deserialize_toml_table(&section_name, table)?;
            patch.layout.types.entry(kind).or_default().merge(section);
        }
        SectionPath::Templater(section) => {
            patch
                .templater
                .merge_toml_section(config_path, section, &section_name, table)?;
        }
        SectionPath::RulesGlobal => {
            patch.rules.merge_global_toml(&section_name, table)?;
        }
        SectionPath::Rule(rule_section) => {
            patch
                .rules
                .merge_rule_section_toml(rule_section, &section_name, table)?;
        }
        SectionPath::Dialect(dialect) => match dialect {
            DialectKind::Postgres => patch
                .dialects
                .postgres
                .merge(de::deserialize_toml_table(&section_name, table)?),
            _ if table.is_empty() => {}
            _ => return Err(ConfigError::UnknownSection(section_name)),
        },
    }

    Ok(())
}

fn toml_section_name(path: &[&str]) -> String {
    if path.is_empty() {
        "tool.sqruff".to_string()
    } else {
        format!("tool.sqruff.{}", path.join("."))
    }
}

fn normalize_section_values(
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
        normalized.insert(name.clone(), value.clone());
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
    config.merge(overrides.into());
}

fn config_error(error: ConfigError) -> SqruffError {
    SqruffError::Config(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_locations_are_parent_before_child() {
        let root = std::path::absolute("target/config-location-test").unwrap();
        let nested = root.join("a").join("b").join("query.sql");

        let locations = ConfigLoader::iter_config_locations_up_to_path(&nested, Some(&root), false)
            .collect::<Vec<_>>();

        assert_eq!(
            locations,
            vec![root.clone(), root.join("a"), root.join("a").join("b")]
        );
    }
}
