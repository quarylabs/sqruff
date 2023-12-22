use std::time::Instant;

use crate::core::linter::linted_dir::LintedDir;

pub struct LintingResult {
    paths: Vec<LintedDir>,
    start_time: Instant,
    total_time: f64,
}

impl LintingResult {
    pub fn new() -> Self {
        LintingResult { paths: vec![], start_time: Instant::now(), total_time: 0.0 }
    }

    /// Add a new `LintedDir` to this result.
    pub fn add(&mut self, path: LintedDir) {
        self.paths.push(path);
    }

    /// Stop the linting timer.
    pub(crate) fn stop_timer(&mut self) {
        self.total_time = self.start_time.elapsed().as_secs_f64();
    }
}
