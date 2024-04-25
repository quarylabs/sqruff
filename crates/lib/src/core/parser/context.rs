use std::rc::Rc;

use ahash::AHashMap;

use super::match_result::MatchResult;
use super::matchable::Matchable;
use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;

#[derive(Debug)]
pub struct ParseContext<'a> {
    dialect: &'a Dialect,
    tqdm: Option<()>,
    match_segment: String,
    match_stack: Vec<String>,
    match_depth: usize,
    track_progress: bool,
    pub(crate) terminators: Vec<Rc<dyn Matchable>>,
    parse_cache: AHashMap<((String, (usize, usize), &'static str, usize), String), MatchResult>,
    pub(crate) indentation_config: AHashMap<String, bool>,
}

impl<'a> ParseContext<'a> {
    pub fn new(dialect: &'a Dialect, indentation_config: AHashMap<String, bool>) -> Self {
        Self {
            dialect,
            tqdm: None,
            match_segment: String::from("File"),
            match_stack: Vec::new(),
            match_depth: 0,
            track_progress: true,
            terminators: Vec::new(),
            parse_cache: AHashMap::new(),
            indentation_config,
        }
    }

    pub fn dialect(&self) -> &Dialect {
        self.dialect
    }

    pub fn from_config(config: &'a FluffConfig) -> Self {
        let dialect = &config.dialect;
        let indentation_config = config.raw["indentation"].as_map().unwrap();
        let indentation_config: AHashMap<_, _> =
            indentation_config.iter().map(|(key, value)| (key.clone(), value.to_bool())).collect();

        Self::new(dialect, indentation_config)
    }

    pub fn progress_bar<T>(&mut self, mut f: impl FnMut(&mut Self) -> T) -> T {
        assert!(self.tqdm.is_none(), "Attempted to re-initialise progressbar.");

        // TODO:
        self.tqdm = Some(());

        f(self)
    }

    pub(crate) fn deeper_match<T>(
        &mut self,
        name: impl ToString,
        clear_terminators: bool,
        push_terminators: &[Rc<dyn Matchable>],
        track_progress: Option<bool>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.match_stack.push(self.match_segment.clone());
        self.match_segment = name.to_string();
        self.match_depth += 1;

        let (appended, terms) = self.set_terminators(clear_terminators, push_terminators);

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
        self.reset_terminators(appended, terms, clear_terminators);
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
        push_terminators: &[Rc<dyn Matchable>],
    ) -> (usize, Vec<Rc<dyn Matchable>>) {
        let mut appended = 0;
        let terminators = self.terminators.clone();

        if clear_terminators && !self.terminators.is_empty() {
            self.terminators =
                if !push_terminators.is_empty() { push_terminators.to_vec() } else { Vec::new() };
        } else if !push_terminators.is_empty() {
            for terminator in push_terminators {
                let terminator_owned = terminator.clone();
                let terminator = &*terminator_owned;

                if !self.terminators.iter().any(|item| item.dyn_eq(terminator)) {
                    self.terminators.push(terminator_owned);
                    appended += 1;
                }
            }
        }

        (appended, terminators)
    }

    fn reset_terminators(
        &mut self,
        appended: usize,
        terminators: Vec<Rc<dyn Matchable>>,
        clear_terminators: bool,
    ) {
        if clear_terminators {
            self.terminators = terminators;
        } else {
            let new_len = self.terminators.len().saturating_sub(appended);
            self.terminators.truncate(new_len);
        }
    }

    pub(crate) fn check_parse_cache(
        &self,
        loc_key: (String, (usize, usize), &'static str, usize),
        matcher_key: String,
    ) -> Option<MatchResult> {
        self.parse_cache.get(&(loc_key, matcher_key)).cloned()
    }

    pub(crate) fn put_parse_cache(
        &mut self,
        loc_key: (String, (usize, usize), &'static str, usize),
        matcher_key: String,
        match_result: MatchResult,
    ) {
        self.parse_cache.insert((loc_key, matcher_key), match_result);
    }
}
