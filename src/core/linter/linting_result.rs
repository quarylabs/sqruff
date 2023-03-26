use crate::core::linter::linted_dir::LintedDir;
use std::time::Instant;

pub struct LintingResult {
    paths: Vec<LintedDir>,
    start_time: Instant,
    total_time: f64,
}

impl LintingResult {
    pub fn new() -> Self {
        LintingResult {
            paths: vec![],
            start_time: Instant::now(),
            total_time: 0.0,
        }
    }
}
