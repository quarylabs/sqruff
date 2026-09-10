use hashbrown::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib::core::linter::linted_file::LintedFile;
use sqruff_lib::core::rules::RuleGroups;
use sqruff_lib::templaters::types::PlaceholderStyle;
use sqruff_lib::templaters::TemplaterKind;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::errors::SQLBaseError;
use sqruff_lib_core::value::Value;
use std::hash::{DefaultHasher, Hash, Hasher};
use strum::IntoEnumIterator;

// ── helpers ──────────────────────────────────────────────────────────────────

fn value_to_py(py: Python, v: &Value) -> Py<PyAny> {
    match v {
        Value::Int(i) => (*i).into_pyobject(py).unwrap().into_any().unbind(),
        Value::Bool(b) => pyo3::types::PyBool::new(py, *b).as_any().clone().unbind(),
        Value::Float(f) => (*f).into_pyobject(py).unwrap().into_any().unbind(),
        Value::String(s) => s.as_ref().into_pyobject(py).unwrap().into_any().unbind(),
        Value::Map(map) => map_to_py_dict(py, map).into_any(),
        Value::Array(arr) => {
            let items: Vec<Py<PyAny>> = arr.iter().map(|v| value_to_py(py, v)).collect();
            PyList::new(py, items).unwrap().unbind().into_any()
        }
        Value::None => py.None(),
    }
}

fn map_to_py_dict(py: Python, map: &HashMap<String, Value>) -> Py<PyDict> {
    let dict = PyDict::new(py);
    for (k, v) in map {
        dict.set_item(k, value_to_py(py, v)).unwrap();
    }
    dict.unbind()
}

fn violation_into_py(py: Python, v: &SQLBaseError) -> Py<Violation> {
    Py::new(
        py,
        Violation {
            code: v.rule.as_ref().map(|r| r.code.to_string()).unwrap_or_default(),
            line_no: v.line_no,
            line_pos: v.line_pos,
            description: v.description.clone(),
            fixable: v.fixable,
        },
    )
    .unwrap()
}



// ── Violation ────────────────────────────────────────────────────────────────

/// A single linting violation.
#[pyclass(name = "Violation")]
struct Violation {
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    line_no: usize,
    #[pyo3(get)]
    line_pos: usize,
    #[pyo3(get)]
    description: String,
    #[pyo3(get)]
    fixable: bool,
}

#[pymethods]
impl Violation {
    fn __repr__(&self) -> String {
        format!(
            "Violation(code={:?}, line_no={}, line_pos={}, fixable={}, description={:?})",
            self.code, self.line_no, self.line_pos, self.fixable, self.description
        )
    }
}

// ── DialectKind ──────────────────────────────────────────────────────────────

/// Represents a SQL dialect. Construct with `DialectKind("snowflake")` or use
/// `DialectKind.available()` to list all supported dialects.
#[pyclass(name = "DialectKind", from_py_object)]
#[derive(Clone)]
struct PyDialectKind(DialectKind);

#[pymethods]
impl PyDialectKind {
    #[new]
    fn new(dialect: &str) -> PyResult<Self> {
        DialectKind::from_str(dialect).map(PyDialectKind).map_err(|_| {
            let valid = Self::available().join(", ");
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Unknown dialect: {dialect:?}. Valid dialects are: {valid}"
            ))
        })
    }

    fn __repr__(&self) -> String {
        format!("DialectKind({:?})", self.0.as_ref())
    }

    fn __str__(&self) -> &str {
        self.0.as_ref()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn __hash__(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.0.as_ref().hash(&mut s);
        s.finish()
    }

    /// List all available dialect names.
    #[staticmethod]
    fn available() -> Vec<String> {
        DialectKind::iter().map(|d| d.as_ref().to_string()).collect()
    }
}

// ── RuleGroups ────────────────────────────────────────────────────────────────

/// A rule category. Use `RuleGroups.available()` to list all groups.
#[pyclass(name = "RuleGroups", from_py_object)]
#[derive(Clone)]
struct PyRuleGroups(RuleGroups);

#[pymethods]
impl PyRuleGroups {
    #[new]
    fn new(group: &str) -> PyResult<Self> {
        RuleGroups::iter()
            .find(|g| g.as_ref() == group)
            .map(PyRuleGroups)
            .ok_or_else(|| {
                let valid = Self::available().join(", ");
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown rule group: {group:?}. Valid groups are: {valid}"
                ))
            })
    }

    fn __str__(&self) -> &str {
        self.0.as_ref()
    }

    fn __repr__(&self) -> String {
        format!("RuleGroups({:?})", self.0.as_ref())
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn __hash__(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.0.as_ref().hash(&mut s);
        s.finish()
    }

    /// List all available rule group names.
    #[staticmethod]
    fn available() -> Vec<String> {
        RuleGroups::iter().map(|g| g.as_ref().to_string()).collect()
    }
}

// ── PlaceholderStyle ──────────────────────────────────────────────────────────

/// A placeholder templater syntax style (e.g. `:var`, `$var`, `%(var)s`).
/// Use `PlaceholderStyle.available()` to list all styles.
#[pyclass(name = "PlaceholderStyle", from_py_object)]
#[derive(Clone)]
struct PyPlaceholderStyle(PlaceholderStyle);

#[pymethods]
impl PyPlaceholderStyle {
    #[new]
    fn new(style: &str) -> PyResult<Self> {
        PlaceholderStyle::from_name(style)
            .map(PyPlaceholderStyle)
            .map_err(|_| {
                let valid = Self::available().join(", ");
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown placeholder style: {style:?}. Valid styles are: {valid}"
                ))
            })
    }

    fn __str__(&self) -> &str {
        self.0.as_str()
    }

    fn __repr__(&self) -> String {
        format!("PlaceholderStyle({:?})", self.0.as_str())
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn __hash__(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.0.as_str().hash(&mut s);
        s.finish()
    }

    /// The regex pattern used to match placeholders of this style.
    #[getter]
    fn regex_pattern(&self) -> &str {
        self.0.regex_pattern()
    }

    /// List all available placeholder style names.
    #[staticmethod]
    fn available() -> Vec<&'static str> {
        PlaceholderStyle::all().iter().map(|s| s.as_str()).collect()
    }
}

// ── TemplaterKind ─────────────────────────────────────────────────────────────

/// A templater engine (e.g. "raw", "jinja", "dbt"). Use `TemplaterKind.available()` to list all.
#[pyclass(name = "TemplaterKind", from_py_object)]
#[derive(Clone)]
struct PyTemplaterKind(TemplaterKind);

#[pymethods]
impl PyTemplaterKind {
    #[new]
    fn new(templater: &str) -> PyResult<Self> {
        TemplaterKind::from_name(templater)
            .map(PyTemplaterKind)
            .map_err(|_| {
                let valid = Self::available().join(", ");
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown templater: {templater:?}. Valid templaters are: {valid}"
                ))
            })
    }

    fn __str__(&self) -> &str {
        self.0.as_str()
    }

    fn __repr__(&self) -> String {
        format!("TemplaterKind({:?})", self.0.as_str())
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn __hash__(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.0.as_str().hash(&mut s);
        s.finish()
    }

    /// List all available templater names.
    #[staticmethod]
    fn available() -> Vec<&'static str> {
        TemplaterKind::available_names()
    }
}

// ── FluffConfig ───────────────────────────────────────────────────────────────

/// Linter configuration.
///
/// Can be constructed several ways:
///     FluffConfig()                          # defaults (dialect: ansi)
///     FluffConfig(dialect="snowflake")       # shorthand kwargs
///     FluffConfig(rules=["LT01"], exclude_rules=["AM01"])
///     FluffConfig.from_source("[sqruff]\\ndialect = snowflake\\n")
///     FluffConfig.from_file(open(".sqruff"))
///     FluffConfig.from_path("/path/to/.sqruff")  # str or pathlib.Path
///     FluffConfig.from_root()                    # walk up from cwd
#[pyclass(name = "FluffConfig", from_py_object)]
#[derive(Clone)]
struct PyFluffConfig(FluffConfig);

#[pymethods]
impl PyFluffConfig {
    /// Create a config, optionally setting common options via keyword arguments.
    #[new]
    #[pyo3(signature = (dialect=None, rules=None, exclude_rules=None))]
    fn new(
        dialect: Option<&str>,
        rules: Option<Vec<String>>,
        exclude_rules: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let mut core = HashMap::<String, Value>::new();
        if let Some(d) = dialect {
            PyDialectKind::new(d)?;
            core.insert("dialect".into(), Value::String(d.into()));
        }
        if let Some(r) = rules {
            core.insert("rules".into(), Value::String(r.join(",").into()));
        }
        if let Some(er) = exclude_rules {
            core.insert("exclude_rules".into(), Value::String(er.join(",").into()));
        }
        let mut configs = HashMap::new();
        configs.insert("core".into(), Value::Map(core));
        Ok(PyFluffConfig(FluffConfig::new(configs, None, None)))
    }

    /// Create a config from an INI-format string.
    ///
    /// Example:
    ///     FluffConfig.from_source("[sqruff]\\ndialect = snowflake\\n")
    #[staticmethod]
    #[pyo3(signature = (source, path = None))]
    fn from_source(source: &str, path: Option<&str>) -> Self {
        PyFluffConfig(FluffConfig::from_source(source, path.map(Path::new)))
    }

    /// Create a config by reading from a file-like object (anything with a `.read()` method).
    ///
    /// Example:
    ///     with open(".sqruff") as f:
    ///         config = FluffConfig.from_file(f)
    #[staticmethod]
    fn from_file(file: &Bound<PyAny>) -> PyResult<Self> {
        let content: String = file.call_method0("read")?.extract()?;
        let path = file.getattr("name").ok().and_then(|n| n.extract::<String>().ok());
        Ok(PyFluffConfig(FluffConfig::from_source(&content, path.as_deref().map(Path::new))))
    }

    /// Create a config by loading a `.sqruff` file at the given path (str or pathlib.Path).
    #[staticmethod]
    fn from_path(path: PathBuf) -> Self {
        PyFluffConfig(FluffConfig::from_file(&path))
    }

    /// Load config by walking up from a directory, merging any `.sqruff` files found.
    ///
    /// Args:
    ///     extra_config_path: An additional config file to load.
    ///     ignore_local_config: If True, skip any `.sqruff` files on disk.
    ///     overrides: Dict of key→value overrides applied on top (e.g. `{"dialect": "snowflake"}`).
    #[staticmethod]
    #[pyo3(signature = (extra_config_path=None, ignore_local_config=false, overrides=None))]
    fn from_root(
        extra_config_path: Option<String>,
        ignore_local_config: bool,
        overrides: Option<std::collections::HashMap<String, String>>,
    ) -> PyResult<Self> {
        let overrides = overrides.map(|m| m.into_iter().collect::<HashMap<_, _>>());
        FluffConfig::from_root(extra_config_path, ignore_local_config, overrides)
            .map(PyFluffConfig)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.value))
    }

    /// The currently configured dialect.
    #[getter]
    fn dialect(&self) -> PyDialectKind {
        PyDialectKind(self.0.dialect_kind())
    }

    #[setter]
    fn set_dialect(&mut self, dialect: &str) -> PyResult<()> {
        self.0
            .override_dialect(PyDialectKind::new(dialect)?.0)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))
    }

    /// The currently configured templater.
    #[getter]
    fn templater(&self) -> PyResult<PyTemplaterKind> {
        self.0
            .templater_kind()
            .map(PyTemplaterKind)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e))
    }

    /// Get a single config value by key and section name.
    fn get(&self, py: Python, key: &str, section: &str) -> Py<PyAny> {
        value_to_py(py, self.0.get(key, section))
    }

    /// Get an entire config section as a dict.
    fn get_section(&self, py: Python, section: &str) -> Py<PyDict> {
        map_to_py_dict(py, self.0.get_section(section))
    }

    /// File extensions that sqruff will consider as SQL files.
    #[getter]
    fn sql_file_exts(&self) -> Vec<String> {
        self.0.sql_file_exts().to_vec()
    }

    #[setter]
    fn set_sql_file_exts(&mut self, exts: Vec<String>) {
        self.0 = std::mem::take(&mut self.0).with_sql_file_exts(exts);
    }

    /// Raise a ValueError if no dialect has been configured.
    fn verify_dialect_specified(&self) -> PyResult<()> {
        match self.0.verify_dialect_specified() {
            None => Ok(()),
            Some(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.value)),
        }
    }

    /// Get a value from the templater's root config section.
    fn get_templater_root_value(&self, py: Python, key: &str) -> Py<PyAny> {
        match self.0.templater_root_value(key) {
            Some(v) => value_to_py(py, v),
            None => py.None(),
        }
    }

    /// Get the full config section for a given templater as a dict, or None.
    fn get_templater_section(&self, py: Python, templater: &str) -> PyResult<Py<PyAny>> {
        let kind = PyTemplaterKind::new(templater)?.0;
        Ok(match self.0.templater_section(kind) {
            Some(map) => map_to_py_dict(py, map).into_any(),
            None => py.None(),
        })
    }

    /// Get a single value from a templater's config section, or None.
    fn get_templater_value(&self, py: Python, templater: &str, key: &str) -> PyResult<Py<PyAny>> {
        let kind = PyTemplaterKind::new(templater)?.0;
        Ok(match self.0.templater_value(kind, key) {
            Some(v) => value_to_py(py, v),
            None => py.None(),
        })
    }

    /// Get the context dict for a templater, or None.
    fn get_templater_context(&self, py: Python, templater: &str) -> PyResult<Py<PyAny>> {
        let kind = PyTemplaterKind::new(templater)?.0;
        Ok(match self.0.templater_context(kind) {
            Some(map) => map_to_py_dict(py, map).into_any(),
            None => py.None(),
        })
    }

    /// Recompute the reflow config from the current raw config.
    fn reload_reflow(&mut self) {
        self.0.reload_reflow();
    }
}

// ── LintedFile ────────────────────────────────────────────────────────────────

/// The result of linting a single file or string.
#[pyclass(name = "LintedFile")]
struct PyLintedFile {
    inner: LintedFile,
}

#[pymethods]
impl PyLintedFile {
    /// The file path (or `"<string input>"` when linting a string).
    #[getter]
    fn path(&self) -> &str {
        self.inner.path()
    }

    /// List of all violations found.
    #[getter]
    fn violations(&self, py: Python) -> Vec<Py<Violation>> {
        self.inner.violations().iter().map(|v| violation_into_py(py, v)).collect()
    }

    #[getter]
    fn has_violations(&self) -> bool {
        self.inner.has_violations()
    }

    #[getter]
    fn has_unfixable_violations(&self) -> bool {
        self.inner.has_unfixable_violations()
    }

    #[getter]
    fn has_fixes(&self) -> bool {
        self.inner.has_fixes()
    }

    /// Return the fixed SQL string. Only meaningful when `fix=True` was passed to
    /// `lint_string` / `lint_paths`.
    fn fix_string(&self) -> String {
        self.inner.clone().fix_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "LintedFile(path={:?}, violations={})",
            self.inner.path(),
            self.inner.violations().len()
        )
    }
}

// ── LintingResult ─────────────────────────────────────────────────────────────

/// The result of linting multiple files.
#[pyclass(name = "LintingResult")]
struct PyLintingResult {
    files: Vec<Py<PyLintedFile>>,
    has_violations: bool,
    has_unfixable_violations: bool,
}

#[pymethods]
impl PyLintingResult {
    /// Iterate over the individual `LintedFile` results.
    fn __iter__(&self, py: Python) -> PyResult<Py<PyAny>> {
        let list = PyList::new(py, &self.files)?;
        Ok(list.call_method0("__iter__")?.unbind())
    }

    fn __len__(&self) -> usize {
        self.files.len()
    }

    #[getter]
    fn has_violations(&self) -> bool {
        self.has_violations
    }

    #[getter]
    fn has_unfixable_violations(&self) -> bool {
        self.has_unfixable_violations
    }

    fn __repr__(&self) -> String {
        format!("LintingResult(files={})", self.files.len())
    }
}

// ── Linter ────────────────────────────────────────────────────────────────────

/// The main linting engine.
///
/// Example:
///     linter = Linter()
///     result = linter.lint_string("select 1", fix=True)
///     print(result.fix_string())
#[pyclass(name = "Linter")]
struct PyLinter {
    // Mutex because lint_paths takes &mut self
    inner: Mutex<Linter>,
}

#[pymethods]
impl PyLinter {
    /// Create a Linter. Accepts an optional `FluffConfig`; defaults are used otherwise.
    #[new]
    #[pyo3(signature = (config = None, include_parse_errors = false))]
    fn new(config: Option<PyFluffConfig>, include_parse_errors: bool) -> PyResult<Self> {
        let config = config.map(|c| c.0).unwrap_or_default();
        let linter = Linter::new(config, None, None, include_parse_errors)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e))?;
        Ok(PyLinter { inner: Mutex::new(linter) })
    }

    /// Lint (and optionally fix) a SQL string.
    ///
    /// Args:
    ///     sql: The SQL to lint.
    ///     filename: Optional filename for error messages.
    ///     fix: If True, apply automatic fixes.
    ///
    /// Returns:
    ///     A LintedFile.
    #[pyo3(signature = (sql, filename = None, fix = false))]
    fn lint_string(
        &self,
        py: Python,
        sql: &str,
        filename: Option<String>,
        fix: bool,
    ) -> PyResult<Py<PyLintedFile>> {
        let linter = self.inner.lock().unwrap();
        let linted = linter
            .lint_string(sql, filename, fix)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Py::new(py, PyLintedFile { inner: linted })
    }

    /// Lint (and optionally fix) a list of file paths.
    ///
    /// Args:
    ///     paths: File or directory paths to lint.
    ///     fix: If True, compute fixes (does NOT write files; call fix_string() on each
    ///          result and write yourself).
    ///
    /// Returns:
    ///     A LintingResult iterable of LintedFile objects.
    #[pyo3(signature = (paths, fix = false))]
    fn lint_paths(
        &self,
        py: Python,
        paths: Vec<String>,
        fix: bool,
    ) -> PyResult<Py<PyLintingResult>> {
        let mut linter = self.inner.lock().unwrap();
        let result = linter
            .lint_paths(
                paths.into_iter().map(PathBuf::from).collect(),
                fix,
                &|_: &Path| false,
            )
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let has_violations = result.has_violations();
        let has_unfixable = result.has_unfixable_violations();
        let files: Vec<Py<PyLintedFile>> = result
            .into_iter()
            .map(|f| Py::new(py, PyLintedFile { inner: f }).unwrap())
            .collect();

        Py::new(
            py,
            PyLintingResult { files, has_violations, has_unfixable_violations: has_unfixable },
        )
    }
}

// ── run_cli ───────────────────────────────────────────────────────────────────

/// Parse CLI args and execute the tool. Exposed to Python as `run_cli`.
#[pyfunction]
fn run_cli(args: Vec<String>) -> PyResult<i32> {
    let mut argv = vec!["sqruff".to_string()];
    argv.extend(args);
    let exit_code = sqruff_cli_lib::run_with_args(argv);
    Ok(exit_code)
}

// ── module ────────────────────────────────────────────────────────────────────

#[pymodule]
#[pyo3(name = "_sqruff")]
fn sqruff(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_cli, m)?)?;
    m.add_class::<Violation>()?;
    m.add_class::<PyDialectKind>()?;
    m.add_class::<PyRuleGroups>()?;
    m.add_class::<PyPlaceholderStyle>()?;
    m.add_class::<PyTemplaterKind>()?;
    m.add_class::<PyFluffConfig>()?;
    m.add_class::<PyLintedFile>()?;
    m.add_class::<PyLintingResult>()?;
    m.add_class::<PyLinter>()?;
    Ok(())
}
