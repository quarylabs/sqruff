/// Timing summary class
use std::collections::HashMap;

/// An object for tracking the timing of similar steps across many files.
#[allow(unused_variables, dead_code)]
pub struct TimingSummary {
    steps: Option<Vec<String>>,
    timings: Vec<HashMap<String, f64>>,
}

impl TimingSummary {
    #[allow(unused_variables, dead_code)]
    fn new(steps: Option<Vec<String>>) -> TimingSummary {
        TimingSummary { steps, timings: Vec::new() }
    }

    /// Add a timing to the summary.
    #[allow(unused_variables, dead_code)]
    fn add_timing(&mut self, _step: String, _timing: f64) {
        if self.steps.is_none() {
            self.steps = Some(Vec::new());
        }
    }

    /// Generate summary for display.
    #[allow(unused_variables, dead_code)]
    fn summary(&self) -> Summary {
        panic!("Not implemented")
    }
}

pub struct Summary {}
