use crate::core::{
    config::FluffConfig,
    dialects::{base::Dialect, init::dialect_selector},
};

use super::matchable::Matchable;

#[derive(Debug)]
pub struct ParseContext {
    dialect: Dialect,
    match_segment: String,
    match_stack: Vec<String>,
    match_depth: usize,
    track_progress: bool,
    // recurse: bool,
    // indentation_config: HashMap<String, bool>,
    // denylist: ParseDenylist,
    // logger: Logger,
    // uuid: uuid::Uuid,
}

impl ParseContext {
    pub fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            match_segment: String::from("File"),
            match_stack: Vec::new(),
            match_depth: 0,
            track_progress: true,
        }
    }

    pub fn dialect(&self) -> &Dialect {
        &self.dialect
    }

    pub fn from_config(_config: FluffConfig) -> Self {
        let dialect = dialect_selector("ansi").unwrap();
        Self::new(dialect)
    }

    pub(crate) fn deeper_match<T>(
        &mut self,
        name: &str,
        clear_terminators: bool,
        push_terminators: &[Box<dyn Matchable>],
        track_progress: Option<bool>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.match_stack.push(self.match_segment.clone());
        self.match_segment = name.to_string();
        self.match_depth += 1;

        // _append, _terms = self._set_terminators(clear_terminators, push_terminators)
        let _track_progress = self.track_progress;

        match track_progress {
            Some(true) => {
                // # We can't go from False to True. Raise an issue if not.
                assert!(
                    self.track_progress,
                    "Cannot set tracking from False to True"
                )
            }
            Some(false) => self.track_progress = false,
            None => {}
        }

        // try
        let ret = f(self);

        // finally
        self.match_depth -= 1;
        // Reset back to old name
        self.match_segment = self.match_stack.pop().unwrap();
        // Reset back to old progress tracking.
        // self.track_progress = _track_progress;

        ret
    }
}
