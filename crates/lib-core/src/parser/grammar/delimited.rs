use std::ops::{Deref, DerefMut};

use ahash::AHashSet;

use super::anyof::{AnyNumberOf, one_of};
use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxSet;
use crate::helpers::ToMatchable;
use crate::parser::grammar::Ref;
use crate::parser::matchable::{
    Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key,
};

/// Match an arbitrary number of elements separated by a delimiter.
///
/// Note that if there are multiple elements passed in that they will be treated
/// as different options of what can be delimited, rather than a sequence.
#[derive(Clone, Debug)]
pub struct Delimited {
    pub base: AnyNumberOf,
    pub allow_trailing: bool,
    pub(crate) delimiter: Matchable,
    pub min_delimiters: usize,
    pub optional_delimiter: bool,
    optional: bool,
    cache_key: MatchableCacheKey,
}

impl Delimited {
    pub fn new(elements: Vec<Matchable>) -> Self {
        Self {
            base: one_of(elements),
            allow_trailing: false,
            delimiter: Ref::new("CommaSegment").to_matchable(),
            min_delimiters: 0,
            optional_delimiter: false,
            optional: false,
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn allow_trailing(&mut self) {
        self.allow_trailing = true;
    }

    pub fn optional_delimiter(&mut self) {
        self.optional_delimiter = true;
    }

    pub fn delimiter(&mut self, delimiter: impl ToMatchable) {
        self.delimiter = delimiter.to_matchable();
    }
}

impl PartialEq for Delimited {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
            && self.allow_trailing == other.allow_trailing
            && self.optional_delimiter == other.optional_delimiter
        // && self.delimiter == other.delimiter
    }
}

impl MatchableTrait for Delimited {
    fn elements(&self) -> &[Matchable] {
        &self.elements
    }

    fn is_optional(&self) -> bool {
        self.optional || self.base.is_optional()
    }

    fn simple(
        &self,
        dialect: &Dialect,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        super::anyof::simple(&self.elements, dialect, crumbs)
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }

    #[track_caller]
    fn copy(
        &self,
        insert: Option<Vec<Matchable>>,
        at: Option<usize>,
        before: Option<Matchable>,
        remove: Option<Vec<Matchable>>,
        terminators: Vec<Matchable>,
        replace_terminators: bool,
    ) -> Matchable {
        let mut new_elements = self.elements.clone();

        if let Some(insert_elements) = insert {
            if let Some(before_element) = before {
                if let Some(index) = self.elements.iter().position(|e| e == &before_element) {
                    new_elements.splice(index..index, insert_elements);
                } else {
                    panic!("Element for insertion before not found");
                }
            } else if let Some(at_index) = at {
                new_elements.splice(at_index..at_index, insert_elements);
            } else {
                new_elements.extend(insert_elements);
            }
        }

        if let Some(remove_elements) = remove {
            new_elements.retain(|elem| !remove_elements.contains(elem));
        }

        let mut new_grammar = self.clone();

        new_grammar.base.elements = new_elements;
        new_grammar.base.terminators = if replace_terminators {
            terminators
        } else {
            [self.terminators.clone(), terminators].concat()
        };

        new_grammar.to_matchable()
    }
}

impl Deref for Delimited {
    type Target = AnyNumberOf;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Delimited {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
