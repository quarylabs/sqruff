pub mod common;
pub mod init;
pub mod syntax;

use std::borrow::Cow;
use std::fmt::Debug;

use hashbrown::hash_map::Entry;
use hashbrown::{HashMap, HashSet};

use crate::dialects::init::DialectKind;
use crate::dialects::syntax::SyntaxKind;
use crate::helpers::ToMatchable;
use crate::parser::lexer::{Lexer, Matcher};
use crate::parser::matchable::Matchable;
use crate::parser::parsers::StringParser;
use crate::parser::types::DialectElementType;

#[derive(Debug, Clone, Default)]
pub struct Dialect {
    pub name: DialectKind,
    lexer_matchers: Vec<Matcher>,
    library: HashMap<Cow<'static, str>, DialectElementType>,
    sets: HashMap<&'static str, HashSet<&'static str>>,
    pub bracket_collections: HashMap<&'static str, HashSet<BracketPair>>,
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

    pub fn add(&mut self, iter: impl IntoIterator<Item = (Cow<'static, str>, DialectElementType)>) {
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
        match self.library.entry(Cow::Borrowed(name)) {
            Entry::Occupied(entry) => {
                let target = entry.into_mut();
                match target {
                    DialectElementType::Matchable(matchable) => {
                        if let Some(node_matcher) = matchable.as_node_matcher() {
                            node_matcher.replace(match_grammar);
                        } else {
                            *target = DialectElementType::Matchable(match_grammar);
                        }
                    }
                    DialectElementType::SegmentGenerator(_) => {
                        *target = DialectElementType::Matchable(match_grammar);
                    }
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(match_grammar.into());
            }
        }
    }

    pub fn lexer_matchers(&self) -> &[Matcher] {
        &self.lexer_matchers
    }

    pub fn insert_lexer_matchers(&mut self, lexer_patch: Vec<Matcher>, before: &str) {
        assert!(
            !self.lexer_matchers.is_empty(),
            "Lexer struct must be defined before it can be patched!"
        );

        let mut buff = Vec::new();
        let mut found = false;

        for elem in std::mem::take(&mut self.lexer_matchers) {
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

        assert!(
            found,
            "Lexer struct insert before '{before}' failed because tag never found."
        );

        self.lexer_matchers = buff;
    }

    pub fn patch_lexer_matchers(&mut self, lexer_patch: Vec<Matcher>) {
        assert!(
            !self.lexer_matchers.is_empty(),
            "Lexer struct must be defined before it can be patched!"
        );

        let mut buff = Vec::with_capacity(self.lexer_matchers.len());

        let patch_dict: HashMap<&'static str, Matcher> = lexer_patch
            .into_iter()
            .map(|elem| (elem.name(), elem))
            .collect();

        for elem in std::mem::take(&mut self.lexer_matchers) {
            if let Some(patch) = patch_dict.get(elem.name()) {
                buff.push(patch.clone());
            } else {
                buff.push(elem);
            }
        }

        self.lexer_matchers = buff;
    }

    pub fn set_lexer_matchers(&mut self, lexer_matchers: Vec<Matcher>) {
        self.lexer_matchers = lexer_matchers;
    }

    pub fn sets(&self, label: &str) -> HashSet<&'static str> {
        match label {
            "bracket_pairs" | "angle_bracket_pairs" => {
                panic!("Use `bracket_sets` to retrieve {label} set.");
            }
            _ => (),
        }

        self.sets.get(label).cloned().unwrap_or_default()
    }

    pub fn sets_mut(&mut self, label: &'static str) -> &mut HashSet<&'static str> {
        assert!(
            label != "bracket_pairs" && label != "angle_bracket_pairs",
            "Use `bracket_sets` to retrieve {label} set."
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

    pub fn bracket_sets(&self, label: &str) -> HashSet<BracketPair> {
        assert!(
            label == "bracket_pairs" || label == "angle_bracket_pairs",
            "Invalid bracket set. Consider using another identifier instead."
        );

        self.bracket_collections
            .get(label)
            .cloned()
            .unwrap_or_default()
    }

    pub fn bracket_sets_mut(&mut self, label: &'static str) -> &mut HashSet<BracketPair> {
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

    #[track_caller]
    pub fn r#ref(&self, name: &str) -> Matchable {
        match self.library.get(name) {
            Some(DialectElementType::Matchable(matchable)) => matchable.clone(),
            Some(DialectElementType::SegmentGenerator(_)) => {
                panic!("Unexpected SegmentGenerator while fetching '{name}'");
            }
            None => {
                panic!("Grammar refers to '{name}' which was not found in the dialect.",);
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

        for keyword_set in [
            "unreserved_keywords",
            "reserved_keywords",
            "future_reserved_keywords",
        ] {
            if let Some(keywords) = self.sets.get(keyword_set) {
                for &kw in keywords {
                    if !self.library.contains_key(kw) {
                        let parser = StringParser::new(kw, SyntaxKind::Keyword);

                        self.library.insert(
                            kw.into(),
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
