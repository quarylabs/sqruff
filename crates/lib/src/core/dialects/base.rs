use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::rc::Rc;

use ahash::{AHashMap, AHashSet};

use crate::core::parser::lexer::Matcher;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::parsers::StringParser;
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::parser::types::DialectElementType;
use crate::helpers::{capitalize, ToErasedSegment};

#[derive(Debug, Clone, Default)]
pub struct Dialect {
    pub(crate) name: &'static str,
    root_segment_name: &'static str,
    lexer_matchers: Option<Vec<Box<dyn Matcher>>>,
    // TODO: Can we use PHF here? https://crates.io/crates/phf
    library: AHashMap<Cow<'static, str>, DialectElementType>,
    sets: AHashMap<&'static str, AHashSet<&'static str>>,
    bracket_collections: AHashMap<String, AHashSet<BracketPair>>,
}

impl PartialEq for Dialect {
    fn eq(&self, other: &Self) -> bool {
        self.root_segment_name == other.root_segment_name
    }
}

impl Dialect {
    pub fn new(root_segment_name: &'static str) -> Self {
        Dialect { name: "ansi", root_segment_name, ..Default::default() }
    }

    pub fn add(
        &mut self,
        iter: impl IntoIterator<Item = (Cow<'static, str>, DialectElementType)> + Clone,
    ) {
        #[cfg(debug_assertions)]
        check_unique_names(self, &iter.clone().into_iter().collect::<Vec<_>>());

        self.library.extend(iter);
    }

    pub fn lexer_matchers(&self) -> &[Box<dyn Matcher>] {
        match &self.lexer_matchers {
            Some(lexer_matchers) => lexer_matchers,
            None => panic!("Lexing struct has not been set for dialect {self:?}"),
        }
    }

    pub fn set_lexer_matchers(&mut self, lexer_matchers: Vec<Box<dyn Matcher>>) {
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

    pub fn bracket_sets(&self, label: &str) -> AHashSet<BracketPair> {
        assert!(
            label == "bracket_pairs" || label == "angle_bracket_pairs",
            "Invalid bracket set. Consider using another identifier instead."
        );

        self.bracket_collections.get(label).cloned().unwrap_or_default()
    }

    pub fn bracket_sets_mut(&mut self, label: &str) -> &mut AHashSet<BracketPair> {
        assert!(
            label == "bracket_pairs" || label == "angle_bracket_pairs",
            "Invalid bracket set. Consider using another identifier instead."
        );

        self.bracket_collections.entry(label.to_string()).or_default()
    }

    pub fn update_bracket_sets(&mut self, label: &str, pairs: Vec<BracketPair>) {
        let set = self.bracket_sets_mut(label);
        for pair in pairs {
            set.insert(pair);
        }
    }

    pub fn r#ref(&self, name: &str) -> Rc<dyn Matchable> {
        // TODO:
        // if !self.expanded {
        //     panic!("Dialect must be expanded before use.");
        // }

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
                        let parser = StringParser::new(
                            &kw.to_lowercase(),
                            |segment| {
                                KeywordSegment::new(
                                    segment.get_raw().unwrap().clone(),
                                    segment.get_position_marker().unwrap().into(),
                                )
                                .to_erased_segment()
                            },
                            None,
                            false,
                            None,
                        );

                        self.library
                            .insert(n.into(), DialectElementType::Matchable(Rc::new(parser)));
                    }
                }
            }
        }
    }

    pub fn root_segment_name(&self) -> &'static str {
        self.root_segment_name
    }

    pub fn get_root_segment(&self) -> Rc<dyn Matchable> {
        self.r#ref(self.root_segment_name())
    }
}

#[cfg(debug_assertions)]
fn check_unique_names(dialect: &Dialect, xs: &[(Cow<'static, str>, DialectElementType)]) {
    let mut names = AHashSet::new();

    for (name, _) in xs {
        assert!(names.insert(name), "ERROR: the name {name} is already registered.");

        assert!(!dialect.library.contains_key(name), "ERROR: the name '{}' is repeated.", name);
    }
}

pub type BracketPair = (String, String, String, bool);
