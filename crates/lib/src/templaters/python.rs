use ahash::AHashMap;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PySlice};
use pyo3::{Py, PyAny, Python};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::{RawFileSlice, TemplatedFile, TemplatedFileSlice};

use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;

use super::Templater;

const PYTHON_FILE: &str = include_str!("python_templater.py");

#[derive(Default)]
pub struct PythonTemplater;

impl Templater for PythonTemplater {
    fn name(&self) -> &'static str {
        "python"
    }

    fn description(&self) -> &'static str {
        r"**Note:** This templater currently does not work by default in the CLI and needs custom set up to work.

The Python templater uses native Python f-strings. An example would be as follows:

```sql
SELECT * FROM {blah}
```

With the following config:

```
[sqruff]
templater = python

[sqruff:templater:python:context]
blah = foo
```

Before parsing the sql will be transformed to:

```sql
SELECT * FROM foo
```

At the moment, dot notation is not supported in the templater."
    }

    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        config: Option<&FluffConfig>,
        _formatter: Option<&OutputStreamFormatter>,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        let empty_hash = AHashMap::new();
        let context = config
            .map(|config| config.get_section("templater"))
            .unwrap_or(&empty_hash);
        let python = context.get("python").ok_or(SQLFluffUserError::new(
            "Python templater requires a python section in the config".to_string(),
        ))?;
        let python = python.as_map().ok_or(SQLFluffUserError::new(
            "Python templater requires a python section in the config".to_string(),
        ))?;
        let python = python.get("context").ok_or(SQLFluffUserError::new(
            "Python templater requires a context section in the python section of the config"
                .to_string(),
        ))?;
        let python = python.as_map().ok_or(SQLFluffUserError::new(
            "Python templater requires a context section in the python section of the config"
                .to_string(),
        ))?;

        let hashmap = python
            .iter()
            .map(|(k, v)| {
                let value = v.as_string().ok_or(SQLFluffUserError::new(
                    "Python templater context values must be strings".to_string(),
                ))?;
                Ok((k.to_string(), value.to_string()))
            })
            .collect::<Result<AHashMap<String, String>, SQLFluffUserError>>();

        // Need to pull context out of config
        let templated_file = Python::with_gil(|py| -> PyResult<TemplatedFile> {
            let fun: Py<PyAny> = PyModule::from_code_bound(py, PYTHON_FILE, "", "")?
                .getattr("process_from_rust")?
                .into();

            // pass object with Rust tuple of positional arguments
            let py_dict = PyDict::new_bound(py);
            for (k, v) in hashmap.unwrap() {
                py_dict.set_item(k, v)?;
            }
            let args = (in_str.to_string(), f_name.to_string(), py_dict);
            let returned = fun.call1(py, args);

            // Parse the returned value
            let returned = returned?;
            let templated_file: PythonTemplatedFile = returned.extract(py)?;
            Ok(templated_file.to_templated_file())
        })
        .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {:?}", e)))?;

        Ok(templated_file)
    }
}

#[derive(Debug)]
struct PythonTemplatedFileSlice {
    slice_type: String,
    source_slice: std::ops::Range<usize>,
    templated_slice: std::ops::Range<usize>,
}

impl<'py> FromPyObject<'py> for PythonTemplatedFileSlice {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        // Get attributes directly from the object
        let slice_type = ob.getattr("slice_type")?.extract::<String>()?;
        let binding = ob.getattr("source_slice")?;
        let source_slice_obj = binding.downcast::<PySlice>()?;
        let bindig = ob.getattr("templated_slice")?;
        let templated_slice_obj = bindig.downcast::<PySlice>()?;

        // Extract start and stop indices from the slices
        let source_start = source_slice_obj
            .getattr("start")?
            .extract::<Option<usize>>()?
            .unwrap_or(0);
        let source_stop = source_slice_obj
            .getattr("stop")?
            .extract::<Option<usize>>()?
            .unwrap_or(0);
        let source_slice = source_start..source_stop;

        let templated_start = templated_slice_obj
            .getattr("start")?
            .extract::<Option<usize>>()?
            .unwrap_or(0);
        let templated_stop = templated_slice_obj
            .getattr("stop")?
            .extract::<Option<usize>>()?
            .unwrap_or(0);
        let templated_slice = templated_start..templated_stop;

        Ok(PythonTemplatedFileSlice {
            slice_type,
            source_slice,
            templated_slice,
        })
    }
}

impl PythonTemplatedFileSlice {
    fn to_templated_file_slice(&self) -> TemplatedFileSlice {
        TemplatedFileSlice::new(
            &self.slice_type,
            self.source_slice.clone(),
            self.templated_slice.clone(),
        )
    }
}

struct PythonRawFileSlice {
    raw: String,
    slice_tpe: String,
    source_idx: usize,
    block_idx: usize,
}

impl PythonRawFileSlice {
    fn to_raw_file_slice(&self) -> RawFileSlice {
        RawFileSlice::new(
            self.raw.to_string(),
            self.slice_tpe.to_string(),
            self.source_idx,
            None,
            Some(self.block_idx),
        )
    }
}

impl<'py> FromPyObject<'py> for PythonRawFileSlice {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let raw = ob.getattr("raw")?.extract::<String>()?;
        let slice_tpe = ob.getattr("slice_type")?.extract::<String>()?;
        let source_idx = ob.getattr("source_idx")?.extract::<usize>()?;
        let block_idx = ob.getattr("block_idx")?.extract::<usize>()?;

        Ok(PythonRawFileSlice {
            raw,
            slice_tpe,
            source_idx,
            block_idx,
        })
    }
}

#[derive(FromPyObject)]
struct PythonTemplatedFile {
    source_str: String,
    fname: String,
    templated_str: Option<String>,
    sliced_file: Option<Vec<PythonTemplatedFileSlice>>,
    raw_sliced: Option<Vec<PythonRawFileSlice>>,
}

impl PythonTemplatedFile {
    fn to_templated_file(&self) -> TemplatedFile {
        TemplatedFile::new(
            self.source_str.to_string(),
            self.fname.to_string(),
            self.templated_str.clone(),
            self.sliced_file
                .as_ref()
                .map(|s| s.iter().map(|s| s.to_templated_file_slice()).collect()),
            self.raw_sliced
                .as_ref()
                .map(|s| s.iter().map(|s| s.to_raw_file_slice()).collect()),
        )
        .unwrap()
    }
}

// Working on tests
#[cfg(test)]
mod tests {
    use super::*;

    const PYTHON_STRING: &str = "SELECT * FROM {blah}";

    #[test]
    // test the python templater
    fn test_templater_python() {
        let source = r"
[sqruff]
templater = python

[sqruff:templater:python:context]
blah = foo
";
        let config = FluffConfig::from_source(source);

        let templater = PythonTemplater;

        let templated_file = templater
            .process(PYTHON_STRING, "test.sql", Some(&config), None)
            .unwrap();

        assert_eq!(templated_file.to_string(), "SELECT * FROM foo");
    }

    #[test]
    fn templater_python_error() {
        let source = r"
[sqruff]
templater = python

[sqruff:templater:python:context]
noblah = foo
";
        let config = FluffConfig::from_source(source);

        let templater = PythonTemplater;

        let templated_file = templater.process(PYTHON_STRING, "test.sql", Some(&config), None);

        assert!(templated_file.is_err())
    }
}
