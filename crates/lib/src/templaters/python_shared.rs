use crate::config::FluffConfig;
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
        let templater = value.templater();
        Self {
            templater_unwrap_wrapped_queries: templater.unwrap_wrapped_queries,
            jinja_templater_paths: templater
                .jinja
                .templater_paths
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect(),
            jinja_loader_search_path: templater
                .jinja
                .loader_search_path
                .as_ref()
                .map(|value| value.to_string_lossy().into_owned()),
            jinja_apply_dbt_builtins: templater.jinja.apply_dbt_builtins,
            jinja_ignore_templating: templater.jinja.ignore_templating,
            jinja_library_paths: templater
                .jinja
                .library_paths
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect(),
            dbt_profile: templater.dbt.profile.clone(),
            dbt_profiles_dir: templater
                .dbt
                .profiles_dir
                .as_ref()
                .map(|value| value.to_string_lossy().into_owned()),
            dbt_target: templater.dbt.target.clone(),
            dbt_target_path: templater
                .dbt
                .target_path
                .as_ref()
                .map(|value| value.to_string_lossy().into_owned()),
            dbt_context: templater.dbt.context.clone(),
            dbt_project_dir: templater
                .dbt
                .project_dir
                .as_ref()
                .map(|value| value.to_string_lossy().into_owned()),
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
            .map(|(k, v)| Ok((k.to_string(), v.to_string())))
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
        let config = FluffConfig::try_from_source("", None).unwrap();

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
