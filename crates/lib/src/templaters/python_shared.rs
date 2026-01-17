use crate::core::config::FluffConfig;
use ahash::AHashMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::{Bound, Python};
use serde::{Deserialize, Serialize};
use sqruff_lib_core::errors::SQLFluffUserError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonFluffConfig {
    templater_unwrap_wrapped_queries: bool,

    jinja_templater_paths: Vec<String>,
    jinja_loader_search_path: Vec<String>,
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
        let templater = value.templater.clone();
        Self {
            templater_unwrap_wrapped_queries: templater.unwrap_wrapped_queries,
            jinja_templater_paths: templater.jinja.templater_paths,
            jinja_loader_search_path: templater.jinja.loader_search_path,
            jinja_apply_dbt_builtins: templater.jinja.apply_dbt_builtins,
            jinja_ignore_templating: templater.jinja.ignore_templating,
            jinja_library_paths: templater.jinja.library_paths,
            dbt_profile: None,
            dbt_profiles_dir: templater.dbt.profiles_dir,
            dbt_target: None,
            dbt_target_path: None,
            dbt_context: None,
            dbt_project_dir: templater.dbt.project_dir,
        }
    }
}

impl<'py> FluffConfig {
    pub fn to_python_context(
        &self,
        py: Python<'py>,
        templater_name: &str,
    ) -> Result<Bound<'py, PyDict>, SQLFluffUserError> {
        let empty = AHashMap::default();
        let context = match templater_name {
            "jinja" => &self.templater.jinja.context,
            "dbt" => &self.templater.dbt.context,
            "python" => &self.templater.python.context,
            _ => &empty,
        };
        let hashmap = context
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<AHashMap<String, String>>();
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
        let config = FluffConfig::from_source("", None).unwrap();

        let python_fluff_config = PythonFluffConfig::from(&config);

        assert_eq!(python_fluff_config.templater_unwrap_wrapped_queries, true);
        assert_eq!(
            python_fluff_config.jinja_templater_paths,
            Vec::<String>::new()
        );
        assert_eq!(
            python_fluff_config.jinja_loader_search_path,
            Vec::<String>::new()
        );
        assert_eq!(python_fluff_config.jinja_apply_dbt_builtins, true);
        assert_eq!(python_fluff_config.jinja_ignore_templating, None);
    }

    // TODO Add more tests
}
