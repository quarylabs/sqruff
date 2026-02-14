use std::iter::zip;
use std::ops::{Deref, DerefMut};

use ahash::AHashSet;

use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxSet;
use crate::helpers::ToMatchable;
use crate::parser::matchable::{
    Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key,
};
use crate::parser::types::ParseMode;

#[derive(Debug, Clone)]
pub struct Sequence {
    elements: Vec<Matchable>,
    pub parse_mode: ParseMode,
    pub allow_gaps: bool,
    is_optional: bool,
    pub terminators: Vec<Matchable>,
    cache_key: MatchableCacheKey,
}

impl Sequence {
    pub fn disallow_gaps(&mut self) {
        self.allow_gaps = false;
    }
}

impl Sequence {
    pub fn new(elements: Vec<Matchable>) -> Self {
        Self {
            elements,
            allow_gaps: true,
            is_optional: false,
            parse_mode: ParseMode::Strict,
            terminators: Vec::new(),
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn optional(&mut self) {
        self.is_optional = true;
    }

    pub fn terminators(mut self, terminators: Vec<Matchable>) -> Self {
        self.terminators = terminators;
        self
    }

    pub fn parse_mode(&mut self, mode: ParseMode) {
        self.parse_mode = mode;
    }

    pub fn allow_gaps(mut self, allow_gaps: bool) -> Self {
        self.allow_gaps = allow_gaps;
        self
    }
}

impl PartialEq for Sequence {
    fn eq(&self, other: &Self) -> bool {
        zip(&self.elements, &other.elements).all(|(a, b)| a == b)
    }
}

impl MatchableTrait for Sequence {
    fn elements(&self) -> &[Matchable] {
        &self.elements
    }

    fn is_optional(&self) -> bool {
        self.is_optional
    }

    fn simple(
        &self,
        dialect: &Dialect,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        let mut simple_raws = AHashSet::new();
        let mut simple_types = SyntaxSet::EMPTY;

        for opt in &self.elements {
            let (raws, types) = opt.simple(dialect, crumbs.clone())?;

            simple_raws.extend(raws);
            simple_types.extend(types);

            if !opt.is_optional() {
                return Some((simple_raws, simple_types));
            }
        }

        (simple_raws, simple_types).into()
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }

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

        new_grammar.elements = new_elements;
        new_grammar.terminators = if replace_terminators {
            terminators
        } else {
            [self.terminators.clone(), terminators].concat()
        };

        new_grammar.to_matchable()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bracketed {
    pub bracket_type: &'static str,
    pub bracket_pairs_set: &'static str,
    allow_gaps: bool,
    pub this: Sequence,
}

impl Bracketed {
    pub fn new(args: Vec<Matchable>) -> Self {
        Self {
            bracket_type: "round",
            bracket_pairs_set: "bracket_pairs",
            allow_gaps: true,
            this: Sequence::new(args),
        }
    }
}

type BracketInfo = Result<(Matchable, Matchable, bool), String>;

impl Bracketed {
    pub fn bracket_type(&mut self, bracket_type: &'static str) {
        self.bracket_type = bracket_type;
    }

    pub(crate) fn outer_allow_gaps(&self) -> bool {
        self.allow_gaps
    }

    fn get_bracket_from_dialect(&self, dialect: &Dialect) -> BracketInfo {
        let bracket_pairs = dialect.bracket_sets(self.bracket_pairs_set);
        for (bracket_type, start_ref, end_ref, persists) in bracket_pairs {
            if bracket_type == self.bracket_type {
                let start_bracket = dialect.r#ref(start_ref);
                let end_bracket = dialect.r#ref(end_ref);

                return Ok((start_bracket, end_bracket, persists));
            }
        }
        Err(format!(
            "bracket_type {:?} not found in bracket_pairs ({}) of {:?} dialect.",
            self.bracket_type, self.bracket_pairs_set, dialect.name
        ))
    }
}

impl Deref for Bracketed {
    type Target = Sequence;

    fn deref(&self) -> &Self::Target {
        &self.this
    }
}

impl DerefMut for Bracketed {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.this
    }
}

impl MatchableTrait for Bracketed {
    fn elements(&self) -> &[Matchable] {
        &self.elements
    }

    fn is_optional(&self) -> bool {
        self.this.is_optional()
    }

    fn simple(
        &self,
        dialect: &Dialect,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        let (start_bracket, _, _) = self.get_bracket_from_dialect(dialect).unwrap();
        start_bracket.simple(dialect, crumbs)
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.this.cache_key()
    }
}
