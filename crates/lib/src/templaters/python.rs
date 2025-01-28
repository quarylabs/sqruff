use pyo3::prelude::*;
use pyo3::types::PySlice;
use pyo3::{Py, PyAny, Python};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::{RawFileSlice, TemplatedFile, TemplatedFileSlice};
use std::ffi::CString;

use super::Templater;
use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::templaters::python_shared::PythonFluffConfig;
use std::sync::Arc;

const PYTHON_FILE: &str = include_str!("sqruff_templaters/python_templater.py");

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
        config: &FluffConfig,
        _formatter: &Option<Arc<dyn Formatter>>,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        // Need to pull context out of config
        let templated_file = Python::with_gil(|py| -> PyResult<TemplatedFile> {
            let file = CString::new(PYTHON_FILE).unwrap();
            let fun: Py<PyAny> = PyModule::from_code(py, &file, c"", c"")?
                .getattr("process_from_rust")?
                .into();

            // pass object with Rust tuple of positional arguments
            let py_dict = config.to_python_context(py, "python").unwrap();
            let python_fluff_config: PythonFluffConfig = config.clone().into();
            let args = (
                in_str.to_string(),
                f_name.to_string(),
                python_fluff_config.to_json_string(),
                py_dict,
            );
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

#[derive(Debug)]
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

#[derive(FromPyObject, Debug)]
pub struct PythonTemplatedFile {
    source_str: String,
    fname: String,
    templated_str: Option<String>,
    sliced_file: Option<Vec<PythonTemplatedFileSlice>>,
    raw_sliced: Option<Vec<PythonRawFileSlice>>,
}

impl PythonTemplatedFile {
    pub fn to_templated_file(&self) -> TemplatedFile {
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
        let config = FluffConfig::from_source(source, None);

        let templater = PythonTemplater;

        let templated_file = templater
            .process(PYTHON_STRING, "test.sql", &config, &None)
            .unwrap();

        assert_eq!(templated_file.templated(), "SELECT * FROM foo");
    }

    #[test]
    fn templater_python_error() {
        let source = r"
[sqruff]
templater = python

[sqruff:templater:python:context]
noblah = foo
";
        let config = FluffConfig::from_source(source, None);

        let templater = PythonTemplater;

        let templated_file = templater.process(PYTHON_STRING, "test.sql", &config, &None);

        assert!(templated_file.is_err())
    }
}
