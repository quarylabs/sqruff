#[cfg(feature = "templater-jinja")]
fn main() {
    pyo3_build_config::use_pyo3_cfgs();
}

#[cfg(not(feature = "templater-jinja"))]
fn main() {
    pyo3_build_config::use_pyo3_cfgs();
}
