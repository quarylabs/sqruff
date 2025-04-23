use pyo3::prelude::*;

/// Parse CLI args and execute the tool. Exposed to Python as `run_cli`.
#[pyfunction]
fn run_cli(args: Vec<String>) -> PyResult<i32> {
    let mut argv = vec!["sqruff".to_string()];
    argv.extend(args);
    let exit_code = sqruff_cli_lib::run_with_args(argv);
    Ok(exit_code)
}

#[pymodule]
#[pyo3(name = "_lib_name")]
fn sqruff(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_cli, m)?)?;
    Ok(())
}
