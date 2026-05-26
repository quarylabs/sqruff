use hashbrown::HashMap;

use super::Value;
use super::raw::{RawConfig, insert_config_path};

#[derive(Debug, Default, Clone)]
pub struct ConfigPatch {
    raw: HashMap<String, Value>,
}

impl ConfigPatch {
    /// Construct a patch from an existing map of top-level config sections.
    pub fn from_sections(sections: HashMap<String, Value>) -> Self {
        Self { raw: sections }
    }

    /// Set a string value at the given nested path, creating intermediate maps as needed.
    pub fn set_string(&mut self, path: &[&str], value: &str) {
        self.set_value(path, Value::String(value.into()));
    }

    /// Set an arbitrary value at the given nested path, creating intermediate maps as needed.
    pub fn set_value(&mut self, path: &[&str], value: Value) {
        let path: Vec<String> = path.iter().map(|s| s.to_string()).collect();
        insert_config_path(&mut self.raw, &path, value);
    }

    /// Return the value at the given nested path, if it exists.
    pub fn value(&self, path: &[&str]) -> Option<&Value> {
        let (first, rest) = path.split_first()?;
        let mut current = self.raw.get(*first)?;
        for key in rest {
            current = current.as_map()?.get(*key)?;
        }
        Some(current)
    }
}

impl From<ConfigPatch> for RawConfig {
    fn from(patch: ConfigPatch) -> Self {
        patch.raw
    }
}
