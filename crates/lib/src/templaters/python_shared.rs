use crate::core::config::FluffConfig;
use crate::templaters::TemplaterKind;
use hashbrown::HashMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::{Bound, Python};
use serde::{Deserialize, Serialize};
use sqruff_lib_core::errors::SQLFluffUserError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonFluffConfig {
    templater_unwrap_wrapped_queries: bool,

    jinja_templater_paths: Vec<String>,
    jinja_loader_search_path: Option<String>,
    jinja_apply_dbt_builtins: bool,
    jinja_ignore_templating: Option<bool>,
    jinja_library_paths: Vec<String>,

    dbt_profile: Option<String>,
    dbt_profiles_dir: Option<String>,
    dbt_target: Option<String>,
    dbt_target_path: Option<String>,
    dbt_context: Option<String>,
    dbt_project_dir: Option<String>,
}

impl PythonFluffConfig {
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl From<&FluffConfig> for PythonFluffConfig {
    fn from(value: &FluffConfig) -> Self {
        Self {
            templater_unwrap_wrapped_queries: value
                .templater_root_value("unwrap_wrapped_queries")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            jinja_templater_paths: value
                .templater_value(TemplaterKind::Jinja, "templater_paths")
                .map(|value| {
                    value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_string().unwrap().to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            jinja_loader_search_path: value
                .templater_value(TemplaterKind::Jinja, "loader_search_path")
                .and_then(|value| value.as_string())
                .map(ToString::to_string),
            jinja_apply_dbt_builtins: value
                .templater_value(TemplaterKind::Jinja, "apply_dbt_builtins")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            jinja_ignore_templating: value
                .templater_value(TemplaterKind::Jinja, "ignore_templating")
                .and_then(|value| value.as_bool()),
            jinja_library_paths: value
                .templater_value(TemplaterKind::Jinja, "library_paths")
                .map(|value| {
                    value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_string().unwrap().to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            dbt_profile: None,
            dbt_profiles_dir: value
                .templater_value(TemplaterKind::Dbt, "profiles_dir")
                .and_then(|value| value.as_string())
                .map(ToString::to_string),
            dbt_target: None,
            dbt_target_path: None,
            dbt_context: None,
            dbt_project_dir: value
                .templater_value(TemplaterKind::Dbt, "project_dir")
                .and_then(|value| value.as_string())
                .map(ToString::to_string),
        }
    }
}

impl From<FluffConfig> for PythonFluffConfig {
    fn from(value: FluffConfig) -> Self {
        Self::from(&value)
    }
}

impl<'py> FluffConfig {
    pub fn to_python_context(
        &self,
        py: Python<'py>,
        templater: TemplaterKind,
    ) -> Result<Bound<'py, PyDict>, SQLFluffUserError> {
        let empty = HashMap::default();
        let hashmap = self
            .templater_context(templater)
            .unwrap_or(&empty)
            .iter()
            .map(|(k, v)| {
                let value = v.as_string().ok_or(SQLFluffUserError::new(
                    "Python templater context values must be strings".to_string(),
                ))?;
                Ok((k.to_string(), value.to_string()))
            })
            .collect::<Result<HashMap<String, String>, SQLFluffUserError>>()?;
        // pass object with Rust tuple of positional arguments
        let py_dict = PyDict::new(py);
        for (k, v) in hashmap {
            py_dict
                .set_item(k, v)
                .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {e:?}")))?;
        }
        Ok(py_dict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fluff_base_config() {
        let config = FluffConfig::from_source("", None);

        let python_fluff_config = PythonFluffConfig::from(config);

        assert!(python_fluff_config.templater_unwrap_wrapped_queries);
        assert_eq!(
            python_fluff_config.jinja_templater_paths,
            Vec::<String>::new()
        );
        assert_eq!(python_fluff_config.jinja_loader_search_path, None);
        assert!(python_fluff_config.jinja_apply_dbt_builtins);
        assert_eq!(python_fluff_config.jinja_ignore_templating, None);
    }

    // TODO Add more tests
}
