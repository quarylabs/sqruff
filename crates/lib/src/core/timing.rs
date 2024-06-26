/// Timing summary class
use ahash::AHashMap;

/// An object for tracking the timing of similar steps across many files.
#[allow(unused_variables, dead_code)]
pub struct TimingSummary {
    steps: Option<Vec<&'static str>>,
    timings: Vec<AHashMap<&'static str, f64>>,
}

impl TimingSummary {
    #[allow(dead_code)]
    fn new(steps: Option<Vec<&'static str>>) -> TimingSummary {
        TimingSummary { steps, timings: Vec::new() }
    }

    /// Add a timing to the summary.
    #[allow(dead_code)]
    fn add_timing(&mut self, _step: &'static str, _timing: f64) {
        if self.steps.is_none() {
            self.steps = Some(Vec::new());
        }
    }

    /// Generate summary for display.
    #[allow(dead_code)]
    fn summary(&self) -> Summary {
        panic!("Not implemented")
    }
}

pub struct Summary {}
