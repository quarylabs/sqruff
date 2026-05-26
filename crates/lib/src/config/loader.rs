use std::path::{Path, PathBuf};

use configparser::ini::Ini;
use hashbrown::HashMap;

use super::model::FluffConfig;
use super::options::{ConfigInput, ConfigLoadOptions, ConfigOverrides};
use super::raw::{RawConfig, Value, insert_config_path, nested_combine};
use crate::api::SqruffError;
pub struct ConfigLoader;

impl ConfigLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn load(&self, options: ConfigLoadOptions) -> Result<FluffConfig, SqruffError> {
        let mut configs = match options.input {
            ConfigInput::ProjectRoot(path) => {
                self.try_load_config_up_to_path(path, None, options.ignore_local_config)?
            }
            ConfigInput::File(path) => {
                let mut configs = HashMap::new();
                Self::try_load_config_file(path, &mut configs)?;
                configs
            }
            ConfigInput::Source { text, path } => Self::try_from_source(&text, path.as_deref())?,
            ConfigInput::Default => HashMap::new(),
        };

        apply_overrides(&mut configs, options.overrides)?;

        Ok(FluffConfig::build_from_raw(configs))
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
                    // Convert `path_to_visit` & `given_path` to `Path`
                    let path_to_visit_as_path = path_to_visit.as_path();
                    let given_path_as_path = given_path.as_path();

                    // Attempt to create a relative path from `given_path` to `path_to_visit`
                    match given_path_as_path.strip_prefix(path_to_visit_as_path) {
                        Ok(relative_path) => {
                            // Get the first component of the relative path
                            if let Some(first_part) = relative_path.components().next() {
                                // Combine `path_to_visit` with the first part of the relative path
                                path_to_visit.join(first_part.as_os_str())
                            } else {
                                // If there are no components in the relative path, return
                                // `path_to_visit`
                                path_to_visit.clone()
                            }
                        }
                        Err(_) => {
                            // If `given_path` is not relative to `path_to_visit`, handle the error
                            // (e.g., return `path_to_visit`)
                            // This part depends on how you want to handle the error.
                            path_to_visit.clone()
                        }
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
    ) -> Result<HashMap<String, Value>, SqruffError> {
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

        Ok(nested_combine(config_stack))
    }

    pub fn try_load_config_at_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<HashMap<String, Value>, SqruffError> {
        let path = path.as_ref();

        let filename_options = [
            /* "setup.cfg", "tox.ini", "pep8.ini", */
            ".sqlfluff",
            ".sqruff", /* "pyproject.toml" */
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

    pub fn try_from_source(
        source: &str,
        path: Option<&Path>,
    ) -> Result<HashMap<String, Value>, SqruffError> {
        let mut configs = HashMap::new();
        let elems = ConfigLoader::try_get_config_elems_from_file(path, Some(source))?;
        ConfigLoader::incorporate_vals(&mut configs, elems);
        Ok(configs)
    }

    pub fn try_load_config_file(
        path: impl AsRef<Path>,
        configs: &mut HashMap<String, Value>,
    ) -> Result<(), SqruffError> {
        let elems = ConfigLoader::try_get_config_elems_from_file(path.as_ref().into(), None)?;
        ConfigLoader::incorporate_vals(configs, elems);
        Ok(())
    }

    pub(crate) fn try_get_config_elems_from_file(
        config_path: Option<&Path>,
        config_string: Option<&str>,
    ) -> Result<Vec<(Vec<String>, Value)>, SqruffError> {
        let mut buff = Vec::new();
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

        config.read(content).map_err(SqruffError::Config)?;

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

                    if name_lowercase == "load_macros_from_path" {
                        return Err(SqruffError::Unsupported(
                            "load_macros_from_path config is not implemented",
                        ));
                    } else if name_lowercase.ends_with("_path") || name_lowercase.ends_with("_dir")
                    {
                        // if absolute_path, just keep
                        // if relative path, make it absolute
                        let Some(path_value) = value.as_string() else {
                            return Err(SqruffError::Config(format!(
                                "invalid path value for config key '{name}'"
                            )));
                        };
                        let path = PathBuf::from(path_value);
                        if !path.is_absolute()
                            && let Some(config_path) = config_path.and_then(Path::parent)
                        {
                            // make config path absolute
                            if let Ok(current_dir) = std::env::current_dir()
                                && let Ok(config_path) =
                                    std::path::absolute(current_dir.join(config_path))
                            {
                                let path = config_path.join(path);
                                let path: String = path.to_string_lossy().into();
                                value = Value::String(path.into());
                            }
                        }
                    }

                    let mut key = key.clone();
                    key.push(name.clone());
                    buff.push((key, value));
                }
            }
        }

        Ok(buff)
    }

    pub(crate) fn incorporate_vals(
        ctx: &mut HashMap<String, Value>,
        values: Vec<(Vec<String>, Value)>,
    ) {
        for (path, value) in values {
            insert_config_path(ctx, &path, value);
        }
    }
}

fn apply_overrides(config: &mut RawConfig, overrides: ConfigOverrides) -> Result<(), SqruffError> {
    if overrides.dialect.is_none() && overrides.rules.is_none() && overrides.exclude_rules.is_none()
    {
        return Ok(());
    }

    let core = config
        .entry("core".into())
        .or_insert_with(|| Value::Map(HashMap::new()));

    let Some(core) = core.as_map_mut() else {
        return Err(SqruffError::Config(
            "core config section must be a table".to_string(),
        ));
    };

    if let Some(dialect) = overrides.dialect {
        core.insert("dialect".into(), Value::String(dialect.as_ref().into()));
    }

    if let Some(rules) = overrides.rules {
        core.insert("rules".into(), Value::String(rules.join(",").into()));
    }

    if let Some(exclude_rules) = overrides.exclude_rules {
        core.insert(
            "exclude_rules".into(),
            Value::String(exclude_rules.join(",").into()),
        );
    }

    Ok(())
}
