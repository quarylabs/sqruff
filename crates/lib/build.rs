#[cfg(feature = "templater-dbt")]
fn main() {
    pyo3_build_config::use_pyo3_cfgs();
}

#[cfg(not(feature = "templater-dbt"))]
fn main() {
    // Nothing to do
}