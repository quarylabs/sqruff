use ahash::AHashMap;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use super::match_result::MatchResult;
use super::matchable::{Matchable, MatchableCacheKey};
use crate::dialects::base::Dialect;
use crate::dialects::syntax::SyntaxKind;
use crate::helpers::IndexSet;
use crate::parser::parser::Parser;

type LocKey = u32;
type LocKeyData = (SmolStr, (usize, usize), SyntaxKind, u32);

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    loc: LocKey,
    key: MatchableCacheKey,
}

impl CacheKey {
    pub fn new(loc: LocKey, key: MatchableCacheKey) -> Self {
        Self { loc, key }
    }
}

#[derive(Debug)]
pub struct ParseContext<'a> {
    dialect: &'a Dialect,
    pub(crate) terminators: Vec<Matchable>,
    loc_keys: IndexSet<LocKeyData>,
    parse_cache: FxHashMap<CacheKey, MatchResult>,
    pub(crate) indentation_config: &'a AHashMap<String, bool>,
}

impl<'a> From<&'a Parser<'a>> for ParseContext<'a> {
    fn from(parser: &'a Parser) -> Self {
        let dialect = parser.dialect();
        let indentation_config = &parser.indentation_config;
        Self::new(dialect, indentation_config)
    }
}

impl<'a> ParseContext<'a> {
    pub fn new(dialect: &'a Dialect, indentation_config: &'a AHashMap<String, bool>) -> Self {
        Self {
            dialect,
            terminators: Vec::new(),
            loc_keys: IndexSet::default(),
            parse_cache: FxHashMap::default(),
            indentation_config,
        }
    }

    pub fn dialect(&self) -> &Dialect {
        self.dialect
    }

    pub(crate) fn deeper_match<T>(
        &mut self,
        clear_terminators: bool,
        push_terminators: &[Matchable],
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let (appended, terms) = self.set_terminators(clear_terminators, push_terminators);

        let ret = f(self);
        self.reset_terminators(appended, terms, clear_terminators);

        ret
    }

    fn set_terminators(
        &mut self,
        clear_terminators: bool,
        push_terminators: &[Matchable],
    ) -> (usize, Vec<Matchable>) {
        let mut appended = 0;
        let terminators = self.terminators.clone();

        if clear_terminators && !self.terminators.is_empty() {
            self.terminators = if !push_terminators.is_empty() {
                push_terminators.to_vec()
            } else {
                Vec::new()
            };
        } else if !push_terminators.is_empty() {
            for terminator in push_terminators {
                let terminator_owned = terminator.clone();

                if !self.terminators.contains(terminator) {
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
        terminators: Vec<Matchable>,
        clear_terminators: bool,
    ) {
        if clear_terminators {
            self.terminators = terminators;
        } else {
            let new_len = self.terminators.len().saturating_sub(appended);
            self.terminators.truncate(new_len);
        }
    }

    pub(crate) fn loc_key(&mut self, data: LocKeyData) -> LocKey {
        let (key, _) = self.loc_keys.insert_full(data);
        key as u32
    }

    pub(crate) fn check_parse_cache(
        &self,
        loc_key: LocKey,
        matcher_key: MatchableCacheKey,
    ) -> Option<MatchResult> {
        self.parse_cache
            .get(&CacheKey::new(loc_key, matcher_key))
            .cloned()
    }

    pub(crate) fn put_parse_cache(
        &mut self,
        loc_key: LocKey,
        matcher_key: MatchableCacheKey,
        match_result: MatchResult,
    ) {
        self.parse_cache
            .insert(CacheKey::new(loc_key, matcher_key), match_result);
    }
}
