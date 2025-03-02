use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::fmt::Debug;

use ahash::{AHashMap, AHashSet};

use crate::dialects::init::DialectKind;
use crate::dialects::syntax::SyntaxKind;
use crate::helpers::{ToMatchable, capitalize};
use crate::parser::lexer::{Lexer, Matcher};
use crate::parser::matchable::Matchable;
use crate::parser::parsers::StringParser;
use crate::parser::types::DialectElementType;

#[derive(Debug, Clone, Default)]
pub struct Dialect {
    pub name: DialectKind,
    lexer_matchers: Option<Vec<Matcher>>,
    // TODO: Can we use PHF here? https://crates.io/crates/phf
    library: AHashMap<Cow<'static, str>, DialectElementType>,
    sets: AHashMap<&'static str, AHashSet<&'static str>>,
    pub bracket_collections: AHashMap<&'static str, AHashSet<BracketPair>>,
    lexer: Option<Lexer>,
}

impl PartialEq for Dialect {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Dialect {
    pub fn new() -> Self {
        Dialect {
            name: DialectKind::Ansi,
            ..Default::default()
        }
    }

    pub fn name(&self) -> DialectKind {
        self.name
    }

    pub fn add(
        &mut self,
        iter: impl IntoIterator<Item = (Cow<'static, str>, DialectElementType)> + Clone,
    ) {
        self.library.extend(iter);
    }

    pub fn grammar(&self, name: &str) -> Matchable {
        match self
            .library
            .get(name)
            .unwrap_or_else(|| panic!("not found {name}"))
        {
            DialectElementType::Matchable(matchable) => matchable.clone(),
            DialectElementType::SegmentGenerator(_) => {
                unreachable!("Attempted to fetch non grammar [{name}] with `Dialect::grammar`.")
            }
        }
    }

    #[track_caller]
    pub fn replace_grammar(&mut self, name: &'static str, match_grammar: Matchable) {
        match self
            .library
            .get_mut(name)
            .unwrap_or_else(|| panic!("Failed to get mutable reference for {name}"))
        {
            DialectElementType::Matchable(matchable) => {
                matchable.as_node_matcher().unwrap().match_grammar = match_grammar;
            }
            DialectElementType::SegmentGenerator(_) => {
                unreachable!("Attempted to fetch non grammar [{name}] with `Dialect::grammar`.")
            }
        }
    }

    pub fn lexer_matchers(&self) -> &[Matcher] {
        match &self.lexer_matchers {
            Some(lexer_matchers) => lexer_matchers,
            None => panic!("Lexing struct has not been set for dialect {self:?}"),
        }
    }

    pub fn insert_lexer_matchers(&mut self, lexer_patch: Vec<Matcher>, before: &str) {
        let mut buff = Vec::new();
        let mut found = false;

        if self.lexer_matchers.is_none() {
            panic!("Lexer struct must be defined before it can be patched!");
        }

        for elem in self.lexer_matchers.take().unwrap() {
            if elem.name() == before {
                found = true;
                for patch in lexer_patch.clone() {
                    buff.push(patch);
                }
                buff.push(elem);
            } else {
                buff.push(elem);
            }
        }

        if !found {
            panic!("Lexer struct insert before '{before}' failed because tag never found.");
        }

        self.lexer_matchers = Some(buff);
    }

    pub fn patch_lexer_matchers(&mut self, lexer_patch: Vec<Matcher>) {
        let mut buff = Vec::with_capacity(self.lexer_matchers.as_ref().map_or(0, Vec::len));
        if self.lexer_matchers.is_none() {
            panic!("Lexer struct must be defined before it can be patched!");
        }

        let patch_dict: AHashMap<&'static str, Matcher> = lexer_patch
            .into_iter()
            .map(|elem| (elem.name(), elem))
            .collect();

        for elem in self.lexer_matchers.take().unwrap() {
            if let Some(patch) = patch_dict.get(elem.name()) {
                buff.push(patch.clone());
            } else {
                buff.push(elem);
            }
        }

        self.lexer_matchers = Some(buff);
    }

    pub fn set_lexer_matchers(&mut self, lexer_matchers: Vec<Matcher>) {
        self.lexer_matchers = lexer_matchers.into();
    }

    pub fn sets(&self, label: &str) -> AHashSet<&'static str> {
        match label {
            "bracket_pairs" | "angle_bracket_pairs" => {
                panic!("Use `bracket_sets` to retrieve {} set.", label);
            }
            _ => (),
        }

        self.sets.get(label).cloned().unwrap_or_default()
    }

    pub fn sets_mut(&mut self, label: &'static str) -> &mut AHashSet<&'static str> {
        assert!(
            label != "bracket_pairs" && label != "angle_bracket_pairs",
            "Use `bracket_sets` to retrieve {} set.",
            label
        );

        match self.sets.entry(label) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(<_>::default()),
        }
    }

    pub fn update_keywords_set_from_multiline_string(
        &mut self,
        set_label: &'static str,
        values: &'static str,
    ) {
        let keywords = values.lines().map(str::trim);
        self.sets_mut(set_label).extend(keywords);
    }

    pub fn add_keyword_to_set(&mut self, set_label: &'static str, value: &'static str) {
        self.sets_mut(set_label).insert(value);
    }

    pub fn bracket_sets(&self, label: &str) -> AHashSet<BracketPair> {
        assert!(
            label == "bracket_pairs" || label == "angle_bracket_pairs",
            "Invalid bracket set. Consider using another identifier instead."
        );

        self.bracket_collections
            .get(label)
            .cloned()
            .unwrap_or_default()
    }

    pub fn bracket_sets_mut(&mut self, label: &'static str) -> &mut AHashSet<BracketPair> {
        assert!(
            label == "bracket_pairs" || label == "angle_bracket_pairs",
            "Invalid bracket set. Consider using another identifier instead."
        );

        self.bracket_collections.entry(label).or_default()
    }

    pub fn update_bracket_sets(&mut self, label: &'static str, pairs: Vec<BracketPair>) {
        let set = self.bracket_sets_mut(label);
        for pair in pairs {
            set.insert(pair);
        }
    }

    pub fn r#ref(&self, name: &str) -> Matchable {
        match self.library.get(name) {
            Some(DialectElementType::Matchable(matchable)) => matchable.clone(),
            Some(DialectElementType::SegmentGenerator(_)) => {
                panic!("Unexpected SegmentGenerator while fetching '{}'", name);
            }
            None => {
                if let Some(keyword) = name.strip_suffix("KeywordSegment") {
                    let keyword_tip = "\
                        \n\nThe syntax in the query is not (yet?) supported. Try to \
                        narrow down your query to a minimal, reproducible case and \
                        raise an issue on GitHub.\n\n\
                        Or, even better, see this guide on how to help contribute \
                        keyword and/or dialect updates:\n\
                        https://github.com/quarylabs/sqruff";
                    panic!(
                        "Grammar refers to the '{keyword}' keyword which was not found in the \
                         dialect.{keyword_tip}",
                    );
                } else {
                    panic!("Grammar refers to '{name}' which was not found in the dialect.",);
                }
            }
        }
    }

    pub fn expand(&mut self) {
        // Temporarily take ownership of 'library' from 'self' to avoid borrow checker
        // errors during mutation.
        let mut library = std::mem::take(&mut self.library);
        for element in library.values_mut() {
            if let DialectElementType::SegmentGenerator(generator) = element {
                *element = DialectElementType::Matchable(generator.expand(self));
            }
        }
        self.library = library;

        for keyword_set in ["unreserved_keywords", "reserved_keywords"] {
            if let Some(keywords) = self.sets.get(keyword_set) {
                for kw in keywords {
                    let n = format!("{}KeywordSegment", capitalize(kw));
                    if !self.library.contains_key(n.as_str()) {
                        let parser = StringParser::new(&kw.to_lowercase(), SyntaxKind::Keyword);

                        self.library.insert(
                            n.into(),
                            DialectElementType::Matchable(parser.to_matchable()),
                        );
                    }
                }
            }
        }

        self.lexer = Lexer::new(self.lexer_matchers()).into();
    }

    pub fn lexer(&self) -> &Lexer {
        self.lexer.as_ref().unwrap()
    }
}

pub type BracketPair = (&'static str, &'static str, &'static str, bool);
