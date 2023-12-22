use super::matchable::Matchable;
use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::init::dialect_selector;

#[derive(Debug)]
pub struct ParseContext {
    dialect: Dialect,
    tqdm: Option<()>,
    match_segment: String,
    match_stack: Vec<String>,
    match_depth: usize,
    track_progress: bool,
    pub terminators: Vec<Box<dyn Matchable>>,
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
            tqdm: None,
            match_segment: String::from("File"),
            match_stack: Vec::new(),
            match_depth: 0,
            track_progress: true,
            terminators: Vec::new(),
        }
    }

    pub fn dialect(&self) -> &Dialect {
        &self.dialect
    }

    pub fn from_config(_config: FluffConfig) -> Self {
        let dialect = dialect_selector("ansi").unwrap();
        Self::new(dialect)
    }

    pub fn progress_bar<T>(&mut self, mut f: impl FnMut(&mut Self) -> T) -> T {
        assert!(self.tqdm.is_none(), "Attempted to re-initialise progressbar.");

        // TODO:
        self.tqdm = Some(());

        // try
        let ret = f(self);
        // finally
        // self.tqdm.unwrap().close();

        ret
    }

    pub(crate) fn deeper_match<T>(
        &mut self,
        name: impl ToString,
        clear_terminators: bool,
        push_terminators: &[Box<dyn Matchable>],
        track_progress: Option<bool>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.match_stack.push(self.match_segment.clone());
        self.match_segment = name.to_string();
        self.match_depth += 1;

        self.set_terminators(clear_terminators, push_terminators);

        // _append, _terms = self._set_terminators(clear_terminators, push_terminators)
        let _track_progress = self.track_progress;

        match track_progress {
            Some(true) => {
                // # We can't go from False to True. Raise an issue if not.
                assert!(self.track_progress, "Cannot set tracking from False to True")
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

    fn set_terminators(
        &mut self,
        clear_terminators: bool,
        push_terminators: &[Box<dyn Matchable>],
    ) {
        if clear_terminators && !self.terminators.is_empty() {
            self.terminators =
                if !push_terminators.is_empty() { push_terminators.to_vec() } else { Vec::new() }
        } else if !push_terminators.is_empty() {
            for terminator in push_terminators {
                let terminator_owned = terminator.clone();
                let terminator = &*terminator_owned;

                if self.terminators.iter().find(|item| item.dyn_eq(terminator)).is_none() {
                    self.terminators.push(terminator_owned);
                }
            }
        }
    }
}
