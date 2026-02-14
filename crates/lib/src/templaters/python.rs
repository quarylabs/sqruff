use pyo3::prelude::*;
use pyo3::types::PySlice;
use pyo3::{Borrowed, Py, PyAny, Python};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::{
    RawFileSlice, TemplatedFile, TemplatedFileSlice, char_idx_to_byte_idx, char_to_byte_indices,
};

use super::Templater;
use crate::Formatter;
use crate::core::config::FluffConfig;
use crate::templaters::ProcessingMode;
use crate::templaters::python_shared::PythonFluffConfig;
use std::sync::Arc;

#[derive(Default)]
pub struct PythonTemplater;

impl PythonTemplater {
    fn process_single(
        &self,
        in_str: &str,
        f_name: &str,
        config: &FluffConfig,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        // Need to pull context out of config
        let templated_file = Python::attach(|py| -> PyResult<TemplatedFile> {
            let main_module = PyModule::import(py, "sqruff.templaters.python_templater")?;
            let fun: Py<PyAny> = main_module.getattr("process_from_rust")?.into();
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
        .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {e:?}")))?;

        Ok(templated_file)
    }
}

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

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::Sequential
    }

    fn process(
        &self,
        files: &[(&str, &str)],
        config: &FluffConfig,
        _formatter: &Option<Arc<dyn Formatter>>,
    ) -> Vec<Result<TemplatedFile, SQLFluffUserError>> {
        files
            .iter()
            .map(|(content, fname)| self.process_single(content, fname, config))
            .collect()
    }
}

#[derive(Debug)]
struct PythonTemplatedFileSlice {
    slice_type: String,
    source_slice: std::ops::Range<usize>,
    templated_slice: std::ops::Range<usize>,
}

impl<'a, 'py> FromPyObject<'a, 'py> for PythonTemplatedFileSlice {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // Get attributes directly from the object
        let slice_type = ob.getattr("slice_type")?.extract::<String>()?;
        let binding = ob.getattr("source_slice")?;
        let source_slice_obj: &Bound<'py, PySlice> = binding.cast()?;
        let bindig = ob.getattr("templated_slice")?;
        let templated_slice_obj: &Bound<'py, PySlice> = bindig.cast()?;

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

#[derive(Debug)]
struct PythonRawFileSlice {
    raw: String,
    slice_tpe: String,
    source_idx: usize,
    block_idx: usize,
}

impl<'a, 'py> FromPyObject<'a, 'py> for PythonRawFileSlice {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
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
        // Python uses character-based indices, but Rust uses byte-based indices.
        // Convert all indices from character offsets to byte offsets.
        let source_char_to_byte = char_to_byte_indices(&self.source_str);
        let templated_char_to_byte = self.templated_str.as_ref().map(|s| char_to_byte_indices(s));

        TemplatedFile::new(
            self.source_str.to_string(),
            self.fname.to_string(),
            self.templated_str.clone(),
            self.sliced_file.as_ref().map(|slices| {
                slices
                    .iter()
                    .map(|s| {
                        let source_start =
                            char_idx_to_byte_idx(&source_char_to_byte, s.source_slice.start);
                        let source_end =
                            char_idx_to_byte_idx(&source_char_to_byte, s.source_slice.end);
                        let (templated_start, templated_end) =
                            if let Some(ref t_map) = templated_char_to_byte {
                                (
                                    char_idx_to_byte_idx(t_map, s.templated_slice.start),
                                    char_idx_to_byte_idx(t_map, s.templated_slice.end),
                                )
                            } else {
                                (s.templated_slice.start, s.templated_slice.end)
                            };
                        TemplatedFileSlice::new(
                            &s.slice_type,
                            source_start..source_end,
                            templated_start..templated_end,
                        )
                    })
                    .collect()
            }),
            self.raw_sliced.as_ref().map(|slices| {
                slices
                    .iter()
                    .map(|s| {
                        RawFileSlice::new(
                            s.raw.to_string(),
                            s.slice_tpe.to_string(),
                            char_idx_to_byte_idx(&source_char_to_byte, s.source_idx),
                            None,
                            Some(s.block_idx),
                        )
                    })
                    .collect()
            }),
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

        let results = templater.process(&[(PYTHON_STRING, "test.sql")], &config, &None);
        let templated_file = results.into_iter().next().unwrap().unwrap();

        assert_eq!(templated_file.templated(), "SELECT * FROM foo");
    }

    #[test]
    fn test_to_templated_file_multibyte_source() {
        // Simulate what Python would return for a multi-byte source string.
        // Source: "SELECT 'あ'" (11 chars in Python, 13 bytes in Rust)
        // Templated: same as source (no templating, but with slices)
        let source = "SELECT 'あ'".to_string();
        let source_char_len = source.chars().count(); // 11
        let source_byte_len = source.len(); // 13

        let ptf = PythonTemplatedFile {
            source_str: source.clone(),
            fname: "test.sql".to_string(),
            templated_str: Some(source.clone()),
            sliced_file: Some(vec![PythonTemplatedFileSlice {
                slice_type: "literal".to_string(),
                // Python char-based indices: 0..11
                source_slice: 0..source_char_len,
                templated_slice: 0..source_char_len,
            }]),
            raw_sliced: Some(vec![PythonRawFileSlice {
                raw: source.clone(),
                slice_tpe: "literal".to_string(),
                source_idx: 0,
                block_idx: 0,
            }]),
        };

        // This should not panic - the conversion should handle multi-byte correctly
        let tf = ptf.to_templated_file();
        assert_eq!(tf.source_str, source);
        assert_eq!(tf.templated().len(), source_byte_len);
    }

    #[test]
    fn test_to_templated_file_multibyte_multiple_slices() {
        // Source: "aあb" (3 chars, 5 bytes)
        // Simulate two raw slices: "aあ" (chars 0..2) and "b" (chars 2..3)
        let source = "aあb".to_string();

        let ptf = PythonTemplatedFile {
            source_str: source.clone(),
            fname: "test.sql".to_string(),
            templated_str: Some(source.clone()),
            sliced_file: Some(vec![
                PythonTemplatedFileSlice {
                    slice_type: "literal".to_string(),
                    source_slice: 0..2, // Python: chars 0..2 ("aあ")
                    templated_slice: 0..2,
                },
                PythonTemplatedFileSlice {
                    slice_type: "literal".to_string(),
                    source_slice: 2..3, // Python: chars 2..3 ("b")
                    templated_slice: 2..3,
                },
            ]),
            raw_sliced: Some(vec![
                PythonRawFileSlice {
                    raw: "aあ".to_string(),
                    slice_tpe: "literal".to_string(),
                    source_idx: 0, // Python char index 0
                    block_idx: 0,
                },
                PythonRawFileSlice {
                    raw: "b".to_string(),
                    slice_tpe: "literal".to_string(),
                    source_idx: 2, // Python char index 2 (should become byte index 4)
                    block_idx: 0,
                },
            ]),
        };

        // Without the fix, this would panic with:
        // "TemplatedFile. Consistency fail on running source length. 4 != 2"
        // because pos (0 + "aあ".len() = 4 bytes) != source_idx (2 chars from Python)
        let tf = ptf.to_templated_file();
        assert_eq!(tf.source_str, source);
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

        let results = templater.process(&[(PYTHON_STRING, "test.sql")], &config, &None);
        let templated_file = results.into_iter().next().unwrap();

        assert!(templated_file.is_err())
    }
}
