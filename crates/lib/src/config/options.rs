use hashbrown::HashMap;

pub type ConfigOverrides = HashMap<String, String>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigLoadOptions {
    pub extra_config_path: Option<String>,
    pub ignore_local_config: bool,
    pub overrides: Option<ConfigOverrides>,
}
