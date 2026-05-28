use std::path::PathBuf;

use sqruff_lib_core::dialects::init::DialectKind;

use super::patch::ConfigPatch;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOverrides {
    pub dialect: Option<DialectKind>,
    pub rules: Option<Vec<String>>,
    pub exclude_rules: Option<Vec<String>>,
}

impl From<ConfigOverrides> for ConfigPatch {
    fn from(overrides: ConfigOverrides) -> Self {
        let mut patch = ConfigPatch::default();

        if let Some(dialect) = overrides.dialect {
            patch.set_dialect(Some(dialect));
        }
        if let Some(rules) = overrides.rules {
            patch.set_rules(Some(rules));
        }
        if let Some(exclude_rules) = overrides.exclude_rules {
            patch.set_exclude_rules(Some(exclude_rules));
        }

        patch
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLoadOptions {
    pub input: ConfigInput,
    pub ignore_local_config: bool,
    pub overrides: ConfigOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigInput {
    ProjectRoot(PathBuf),
    File(PathBuf),
    Source { text: String, path: Option<PathBuf> },
    Default,
}

impl Default for ConfigLoadOptions {
    fn default() -> Self {
        Self {
            input: ConfigInput::Default,
            ignore_local_config: false,
            overrides: ConfigOverrides::default(),
        }
    }
}
