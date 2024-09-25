use std::time::Instant;

use crate::core::linter::linted_dir::LintedDir;

#[derive(Debug)]
pub struct LintingResult {
    pub paths: Vec<LintedDir>,
    start_time: Instant,
    total_time: f64,
}

impl Default for LintingResult {
    fn default() -> Self {
        Self::new()
    }
}

impl LintingResult {
    pub fn new() -> Self {
        LintingResult {
            paths: vec![],
            start_time: Instant::now(),
            total_time: 0.0,
        }
    }

    /// Add a new `LintedDir` to this result.
    pub fn add(&mut self, path: LintedDir) -> usize {
        let idx = self.paths.len();
        self.paths.push(path);
        idx
    }

    /// Stop the linting timer.
    pub(crate) fn stop_timer(&mut self) {
        self.total_time = self.start_time.elapsed().as_secs_f64();
    }
}
