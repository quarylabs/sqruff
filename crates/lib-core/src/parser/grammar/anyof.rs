use ahash::AHashSet;

use super::sequence::{Bracketed, Sequence};
use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxSet;
use crate::helpers::ToMatchable;
use crate::parser::matchable::{
    Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key,
};
use crate::parser::types::ParseMode;

pub fn simple(
    elements: &[Matchable],
    dialect: &Dialect,
    crumbs: Option<Vec<&str>>,
) -> Option<(AHashSet<String>, SyntaxSet)> {
    let option_simples: Vec<Option<(AHashSet<String>, SyntaxSet)>> = elements
        .iter()
        .map(|opt| opt.simple(dialect, crumbs.clone()))
        .collect();

    if option_simples.iter().any(Option::is_none) {
        return None;
    }

    let simple_buff: Vec<(AHashSet<String>, SyntaxSet)> =
        option_simples.into_iter().flatten().collect();

    let simple_raws: AHashSet<_> = simple_buff
        .iter()
        .flat_map(|(raws, _)| raws)
        .cloned()
        .collect();

    let simple_types: SyntaxSet = simple_buff
        .iter()
        .flat_map(|(_, types)| types.clone())
        .collect();

    Some((simple_raws, simple_types))
}

#[derive(Debug, Clone)]
pub struct AnyNumberOf {
    pub exclude: Option<Matchable>,
    pub(crate) elements: Vec<Matchable>,
    pub terminators: Vec<Matchable>,
    pub reset_terminators: bool,
    pub max_times: Option<usize>,
    pub min_times: usize,
    pub max_times_per_element: Option<usize>,
    pub allow_gaps: bool,
    pub(crate) optional: bool,
    pub parse_mode: ParseMode,
    cache_key: MatchableCacheKey,
}

impl PartialEq for AnyNumberOf {
    fn eq(&self, other: &Self) -> bool {
        self.elements
            .iter()
            .zip(&other.elements)
            .all(|(lhs, rhs)| lhs == rhs)
    }
}

impl AnyNumberOf {
    pub fn new(elements: Vec<Matchable>) -> Self {
        Self {
            elements,
            exclude: None,
            max_times: None,
            min_times: 0,
            max_times_per_element: None,
            allow_gaps: true,
            optional: false,
            reset_terminators: false,
            parse_mode: ParseMode::Strict,
            terminators: Vec::new(),
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn optional(&mut self) {
        self.optional = true;
    }

    pub fn disallow_gaps(&mut self) {
        self.allow_gaps = false;
    }

    pub fn max_times(&mut self, max_times: usize) {
        self.max_times = max_times.into();
    }

    pub fn min_times(&mut self, min_times: usize) {
        self.min_times = min_times;
    }
}

impl MatchableTrait for AnyNumberOf {
    fn elements(&self) -> &[Matchable] {
        &self.elements
    }

    fn is_optional(&self) -> bool {
        self.optional || self.min_times == 0
    }

    fn simple(
        &self,
        dialect: &Dialect,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        simple(&self.elements, dialect, crumbs)
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

        new_grammar.elements = new_elements;
        new_grammar.terminators = if replace_terminators {
            terminators
        } else {
            [self.terminators.clone(), terminators].concat()
        };

        new_grammar.to_matchable()
    }
}

pub fn one_of(elements: Vec<Matchable>) -> AnyNumberOf {
    let mut matcher = AnyNumberOf::new(elements);
    matcher.max_times(1);
    matcher.min_times(1);
    matcher
}

pub fn optionally_bracketed(elements: Vec<Matchable>) -> AnyNumberOf {
    let mut args = vec![Bracketed::new(elements.clone()).to_matchable()];

    if elements.len() == 1 {
        args.extend(elements);
    } else {
        args.push(Sequence::new(elements).to_matchable());
    }

    one_of(args)
}

pub fn any_set_of(elements: Vec<Matchable>) -> AnyNumberOf {
    let mut any_number_of = AnyNumberOf::new(elements);
    any_number_of.max_times = None;
    any_number_of.max_times_per_element = Some(1);
    any_number_of
}
