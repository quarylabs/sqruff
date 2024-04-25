use std::ops::Index;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use ahash::AHashMap;
use configparser::ini::Ini;
use itertools::Itertools;

use super::dialects::base::Dialect;
use crate::core::dialects::init::{dialect_readout, dialect_selector, get_default_dialect};
use crate::core::errors::SQLFluffUserError;

#[derive(Clone, Debug)]
pub struct RemovedConfig<'a> {
    #[allow(dead_code)]
    old_path: Vec<&'static str>,

    #[allow(dead_code)]
    warning: &'a str,

    #[allow(dead_code)]
    new_path: Option<Vec<&'a str>>,

    #[allow(dead_code)]
    translation_func: Option<fn(&'a str) -> &'a str>,
}

pub fn removed_configs() -> [RemovedConfig<'static>; 12] {
    [
        RemovedConfig {
            old_path: vec!["rules", "max_line_length"],
            warning: "The max_line_length config has moved from sqlfluff:rules to the root sqlfluff level.",
            new_path: Some(vec!["max_line_length"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L003", "hanging_indents"],
            warning: "Hanging indents are no longer supported in SQLFluff from version 2.0.0 onwards. See https://docs.sqlfluff.com/en/stable/layout.html#hanging-indents",
            new_path: None,
            translation_func: None,
        },
        RemovedConfig {
            old_path: vec!["rules", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L002", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L003", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L004", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L016", "tab_space_size"],
            warning: "The tab_space_size config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "tab_space_size"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "indent_unit"],
            warning: "The indent_unit config has moved from sqlfluff:rules to sqlfluff:indentation.",
            new_path: Some(vec!["indentation", "indent_unit"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L007", "operator_new_lines"],
            warning: "Use the line_position config in the appropriate sqlfluff:layout section (e.g. sqlfluff:layout:type:binary_operator).",
            new_path: Some(vec!["layout", "type", "binary_operator", "line_position"]),
            translation_func: Some(|x| if x == "before" { "trailing" } else { "leading" }),
        },
        RemovedConfig {
            old_path: vec!["rules", "comma_style"],
            warning: "Use the line_position config in the appropriate sqlfluff:layout section (e.g. sqlfluff:layout:type:comma).",
            new_path: Some(vec!["layout", "type", "comma", "line_position"]),
            translation_func: Some(|x| x),
        },
        // L019 used to have a more specific version of the same /config itself.
        RemovedConfig {
            old_path: vec!["rules", "L019", "comma_style"],
            warning: "Use the line_position config in the appropriate sqlfluff:layout section (e.g. sqlfluff:layout:type:comma).",
            new_path: Some(vec!["layout", "type", "comma", "line_position"]),
            translation_func: Some(|x| x),
        },
        RemovedConfig {
            old_path: vec!["rules", "L003", "lint_templated_tokens"],
            warning: "No longer used.",
            new_path: None,
            translation_func: None,
        },
    ]
}

/// split_comma_separated_string takes a string and splits it on commas and
/// trims and filters out empty strings.
pub fn split_comma_separated_string(raw_str: &str) -> Value {
    let values = raw_str
        .split(',')
        .flat_map(|x| {
            let trimmed = x.trim();
            (!trimmed.is_empty()).then(|| Value::String(trimmed.into()))
        })
        .collect();

    Value::Array(values)
}

/// The class that actually gets passed around as a config object.
// TODO This is not a translation that is particularly accurate.
#[derive(Debug, PartialEq, Clone)]
pub struct FluffConfig {
    pub(crate) indentation: FluffConfigIndentation,
    pub(crate) raw: AHashMap<String, Value>,
    extra_config_path: Option<String>,
    _configs: AHashMap<String, AHashMap<String, String>>,
    pub(crate) dialect: Dialect,
    sql_file_exts: Vec<String>,
}

impl Default for FluffConfig {
    fn default() -> Self {
        Self::new(<_>::default(), None, None)
    }
}

impl FluffConfig {
    pub fn get(&self, key: &str, section: &str) -> &Value {
        &self.raw[section][key]
    }

    pub fn get_section(&self, section: &str) -> &AHashMap<String, Value> {
        self.raw[section].as_map().unwrap()
    }

    // TODO This is not a translation that is particularly accurate.
    pub fn new(
        configs: AHashMap<String, Value>,
        extra_config_path: Option<String>,
        indentation: Option<FluffConfigIndentation>,
    ) -> Self {
        fn nested_combine(
            mut a: AHashMap<String, Value>,
            b: AHashMap<String, Value>,
        ) -> AHashMap<String, Value> {
            for (key, value_b) in b {
                match (a.get(&key), value_b) {
                    (Some(Value::Map(map_a)), Value::Map(map_b)) => {
                        let combined = nested_combine(map_a.clone(), map_b);
                        a.insert(key, Value::Map(combined));
                    }
                    (_, value) => {
                        a.insert(key, value);
                    }
                }
            }
            a
        }

        let _dialect = match configs.get("dialect") {
            None => get_default_dialect(),
            Some(Value::String(std)) => {
                dialect_selector(std).unwrap_or_else(|| panic!("Unknown dialect {std}"));
                std.as_ref()
            }
            value => unimplemented!("dialect key {value:?}"),
        }
        .to_string();

        let values = ConfigLoader
            .get_config_elems_from_file(None, include_str!("./default_config.cfg").into());

        let mut defaults = AHashMap::new();
        ConfigLoader.incorporate_vals(&mut defaults, values);

        let mut configs = nested_combine(defaults, configs);

        for (in_key, out_key) in [
            // Deal with potential ignore & warning parameters
            ("ignore", "ignore"),
            ("warnings", "warnings"),
            ("rules", "rule_allowlist"),
            // Allowlists and denylistsignore_words
            ("exclude_rules", "rule_denylist"),
        ] {
            match configs["core"].as_map().unwrap().get(in_key) {
                Some(value) if !value.is_none() => {
                    let string = value.as_string().unwrap();
                    let values = split_comma_separated_string(string);

                    configs
                        .get_mut("core")
                        .unwrap()
                        .as_map_mut()
                        .unwrap()
                        .insert(out_key.into(), values);
                }
                _ => {}
            }
        }

        Self {
            raw: configs,
            dialect: dialect_selector("ansi").unwrap(),
            extra_config_path,
            _configs: AHashMap::new(),
            indentation: indentation.unwrap_or_default(),
            sql_file_exts: vec![".sql".into()],
        }
    }

    pub fn with_sql_file_exts(mut self, exts: Vec<String>) -> Self {
        self.sql_file_exts = exts;
        self
    }

    /// Loads a config object just based on the root directory.
    // TODO This is not a translation that is particularly accurate.
    pub fn from_root(
        extra_config_path: Option<String>,
        ignore_local_config: bool,
        _overrides: Option<AHashMap<String, String>>,
    ) -> Result<FluffConfig, SQLFluffUserError> {
        let loader = ConfigLoader {};
        let config =
            loader.load_config_up_to_path(".", extra_config_path.clone(), ignore_local_config);

        Ok(FluffConfig::new(config, extra_config_path, None))
    }

    pub fn from_kwargs(
        config: Option<FluffConfig>,
        dialect: Option<Dialect>,
        rules: Option<Vec<String>>,
    ) -> Self {
        if (dialect.is_some() || rules.is_some()) && config.is_some() {
            panic!(
                "Cannot specify `config` with `dialect` or `rules`. Any config object specifies \
                 its own dialect and rules."
            )
        } else {
            config.unwrap()
        }
    }

    /// Process a full raw file for inline config and update self.
    pub fn process_raw_file_for_config(&self, raw_str: &str) {
        // Scan the raw file for config commands
        for raw_line in raw_str.lines() {
            if raw_line.to_string().starts_with("-- sqlfluff") {
                // Found a in-file config command
                self.process_inline_config(raw_line)
            }
        }
    }

    /// Process an inline config command and update self.
    pub fn process_inline_config(&self, _config_line: &str) {
        panic!("Not implemented")
    }

    /// Check if the config specifies a dialect, raising an error if not.
    pub fn verify_dialect_specified(&self) -> Option<SQLFluffUserError> {
        if self._configs.get("core")?.get("dialect").is_some() {
            return None;
        }
        // Get list of available dialects for the error message. We must
        // import here rather than at file scope in order to avoid a circular
        // import.
        Some(SQLFluffUserError::new(format!(
            "No dialect was specified. You must configure a dialect or
specify one on the command line using --dialect after the
command. Available dialects: {}",
            dialect_readout().join(", ").as_str()
        )))
    }

    pub fn get_dialect(&self) -> &Dialect {
        &self.dialect
    }

    pub fn sql_file_exts(&self) -> &[String] {
        self.sql_file_exts.as_ref()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FluffConfigIndentation {
    pub template_blocks_indent: bool,
}

impl Default for FluffConfigIndentation {
    fn default() -> Self {
        Self { template_blocks_indent: true }
    }
}

pub struct ConfigLoader;

impl ConfigLoader {
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

    #[allow(unused_variables)]
    pub fn load_config_up_to_path(
        &self,
        path: impl AsRef<Path>,
        _extra_config_path: Option<String>,
        ignore_local_config: bool,
    ) -> AHashMap<String, Value> {
        let path = path.as_ref();

        let config_paths: Box<dyn Iterator<Item = PathBuf>> = if ignore_local_config {
            Box::new(std::iter::empty())
        } else {
            Box::new(Self::iter_config_locations_up_to_path(path, None, ignore_local_config))
        };

        let config_stack = if ignore_local_config {
            Vec::new()
        } else {
            config_paths.into_iter().map(|path| self.load_config_at_path(path)).collect_vec()
        };

        nested_combine(config_stack)
    }

    pub fn load_config_at_path(&self, path: impl AsRef<Path>) -> AHashMap<String, Value> {
        let path = path.as_ref();

        let filename_options = [
            /* "setup.cfg", "tox.ini", "pep8.ini", */
            ".sqlfluff", /* "pyproject.toml" */
        ];

        let path = if path.is_dir() { path } else { path.parent().unwrap() };
        let mut configs = AHashMap::new();

        for fname in filename_options {
            let path = path.join(fname);
            if path.exists() {
                self.load_config_file(path, &mut configs);
            }
        }

        configs
    }

    pub fn load_config_file(&self, path: impl AsRef<Path>, configs: &mut AHashMap<String, Value>) {
        let elems = self.get_config_elems_from_file(path.as_ref().into(), None);
        self.incorporate_vals(configs, elems);
    }

    fn get_config_elems_from_file(
        &self,
        path: Option<&Path>,
        config_string: Option<&str>,
    ) -> Vec<(Vec<String>, Value)> {
        let mut buff = Vec::new();
        let mut config = Ini::new();

        let content = match (path, config_string) {
            (None, None) | (Some(_), Some(_)) => {
                unimplemented!("One of fpath or config_string is required.")
            }
            (None, Some(text)) => text.to_owned(),
            (Some(path), None) => std::fs::read_to_string(path).unwrap(),
        };

        config.read(content).unwrap();

        for section in config.sections() {
            let key = if section == "sqlfluff" {
                vec!["core".to_owned()]
            } else if let Some(key) = section.strip_prefix("sqlfluff:") {
                key.split(':').map(ToOwned::to_owned).collect()
            } else {
                continue;
            };

            let config_map = config.get_map_ref();
            if let Some(section) = config_map.get(&section) {
                for (name, value) in section {
                    let value: Value = value.as_ref().unwrap().parse().unwrap();
                    let name_lowercase = name.to_lowercase();

                    if name_lowercase == "load_macros_from_path" {
                        unimplemented!()
                    } else if name_lowercase.ends_with("_path") || name_lowercase.ends_with("_dir")
                    {
                        unimplemented!()
                    }

                    let mut key = key.clone();
                    key.push(name.clone());
                    buff.push((key, value));
                }
            }
        }

        buff
    }

    fn incorporate_vals(
        &self,
        ctx: &mut AHashMap<String, Value>,
        values: Vec<(Vec<String>, Value)>,
    ) {
        for (path, value) in values {
            let mut current_map = &mut *ctx;
            for key in path.iter().take(path.len() - 1) {
                match current_map
                    .entry(key.to_string())
                    .or_insert_with(|| Value::Map(AHashMap::new()))
                    .as_map_mut()
                {
                    Some(slot) => current_map = slot,
                    None => panic!("Overriding config value with section! [{path:?}]"),
                }
            }

            let last_key = path.last().expect("Expected at least one element in path");
            current_map.insert(last_key.to_string(), value);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i32),
    Bool(bool),
    Float(f64),
    String(Box<str>),
    Map(AHashMap<String, Value>),
    Array(Vec<Value>),
    None,
}

impl Value {
    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        if let Self::Array(v) = self { Some(v) } else { None }
    }
}

impl Index<&str> for Value {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        match self {
            Value::Map(map) => map.get(index).unwrap_or(&Value::None),
            _ => unreachable!(),
        }
    }
}

impl Value {
    pub fn to_bool(&self) -> bool {
        match *self {
            Value::Int(v) => v != 0,
            Value::Bool(v) => v,
            Value::Float(v) => v != 0.0,
            Value::String(ref v) => !v.is_empty(),
            Value::Map(ref v) => !v.is_empty(),
            Value::None => false,
            Value::Array(ref v) => !v.is_empty(),
        }
    }

    pub fn as_map(&self) -> Option<&AHashMap<String, Value>> {
        if let Self::Map(map) = self { Some(map) } else { None }
    }

    pub fn as_map_mut(&mut self) -> Option<&mut AHashMap<String, Value>> {
        if let Self::Map(map) = self { Some(map) } else { None }
    }

    pub fn as_int(&self) -> Option<i32> {
        if let Self::Int(v) = self { Some(*v) } else { None }
    }

    pub fn as_string(&self) -> Option<&str> {
        if let Self::String(v) = self { Some(v) } else { None }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self { Some(*v) } else { None }
    }
}

impl FromStr for Value {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use unicase::UniCase;

        static KEYWORDS: phf::Map<UniCase<&'static str>, Value> = phf::phf_map! {
            UniCase::ascii("true") => Value::Bool(true),
            UniCase::ascii("false") => Value::Bool(false),
            UniCase::ascii("none") => Value::None,
        };

        if let Ok(value) = s.parse() {
            return Ok(Value::Int(value));
        }

        if let Ok(value) = s.parse() {
            return Ok(Value::Float(value));
        }

        let key = UniCase::ascii(s);
        let value = KEYWORDS.get(&key).cloned().unwrap_or_else(|| Value::String(Box::from(s)));

        Ok(value)
    }
}

fn nested_combine(config_stack: Vec<AHashMap<String, Value>>) -> AHashMap<String, Value> {
    let capacity = config_stack.len();
    let mut result = AHashMap::with_capacity(capacity);

    for dict in config_stack {
        for (key, value) in dict {
            result.insert(key, value);
        }
    }

    result
}
