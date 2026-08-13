pub mod common;
pub mod init;
pub mod sets;
pub mod syntax;

use std::borrow::Cow;
use std::fmt::Debug;
use std::fmt::{Display, Formatter};

use hashbrown::hash_map::Entry;
use hashbrown::{HashMap, HashSet};

use crate::dialects::init::DialectKind;
use crate::dialects::sets::DialectSetKey;
use crate::dialects::syntax::SyntaxKind;
use crate::helpers::ToMatchable;
use crate::parser::lexer::{Lexer, Matcher};
use crate::parser::matchable::{Matchable, MatchableTrait};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialectValidationError {
    path: Vec<String>,
    missing_reference: String,
}

impl DialectValidationError {
    pub fn path(&self) -> &[String] {
        &self.path
    }

    pub fn missing_reference(&self) -> &str {
        &self.missing_reference
    }
}

impl Display for DialectValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Grammar reference path {} refers to '{}' which was not found in the dialect",
            self.path.join(" -> "),
            self.missing_reference,
        )
    }
}

impl std::error::Error for DialectValidationError {}

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
        match DialectSetKey::parse(label) {
            DialectSetKey::Named(label) => self.sets.get(label).cloned().unwrap_or_default(),
            DialectSetKey::BracketPairs | DialectSetKey::AngleBracketPairs => {
                panic!("Use `bracket_sets` to retrieve {label} set.");
            }
        }
    }

    pub fn sets_mut(&mut self, label: &'static str) -> &mut HashSet<&'static str> {
        match DialectSetKey::parse(label) {
            DialectSetKey::Named(label) => match self.sets.entry(label) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => entry.insert(<_>::default()),
            },
            DialectSetKey::BracketPairs | DialectSetKey::AngleBracketPairs => {
                panic!("Use `bracket_sets` to retrieve {label} set.");
            }
        }
    }

    pub fn update_keywords_set_from_multiline_string(
        &mut self,
        set_label: &'static str,
        values: &'static str,
    ) {
        let keywords = values
            .lines()
            .map(str::trim)
            .filter(|keyword| !keyword.is_empty());
        self.sets_mut(set_label).extend(keywords);
    }

    pub fn add_keyword_to_set(&mut self, set_label: &'static str, value: &'static str) {
        self.sets_mut(set_label).insert(value);
    }

    pub fn bracket_sets(&self, label: &str) -> HashSet<BracketPair> {
        let key = DialectSetKey::parse(label)
            .as_bracket_set_name()
            .unwrap_or_else(|| {
                panic!("Invalid bracket set. Consider using another identifier instead.")
            });

        self.bracket_collections
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    pub fn bracket_sets_mut(&mut self, label: &'static str) -> &mut HashSet<BracketPair> {
        let key = DialectSetKey::parse(label)
            .as_bracket_set_name()
            .unwrap_or_else(|| {
                panic!("Invalid bracket set. Consider using another identifier instead.")
            });

        self.bracket_collections.entry(key).or_default()
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

    /// Validate references reachable from the grammar used to parse a file.
    ///
    /// Dialects inherit and replace grammar extensively. Validating only the
    /// whole library would reject intentionally dormant inherited grammars, so
    /// this follows the same graph the parser can reach from `FileSegment`.
    pub fn validate(&self) -> Result<(), DialectValidationError> {
        match self.validation_errors().into_iter().next() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Return every missing reference reachable from the file grammar.
    pub fn validation_errors(&self) -> Vec<DialectValidationError> {
        let root_name = "FileSegment";
        let Some(DialectElementType::Matchable(root)) = self.library.get(root_name) else {
            return vec![DialectValidationError {
                path: vec![root_name.to_string()],
                missing_reference: root_name.to_string(),
            }];
        };

        let mut visited = HashSet::new();
        let mut path = vec![root_name.to_string()];
        let mut errors = Vec::new();
        self.validate_matchable(root, &mut visited, &mut path, &mut errors);
        errors
    }

    fn validate_matchable(
        &self,
        matchable: &Matchable,
        visited: &mut HashSet<usize>,
        path: &mut Vec<String>,
        errors: &mut Vec<DialectValidationError>,
    ) {
        if !visited.insert(matchable.identity()) {
            return;
        }

        if let Some(reference) = matchable.as_ref() {
            let name = reference.reference();
            path.push(name.to_string());
            if let Some(DialectElementType::Matchable(target)) = self.library.get(name) {
                self.validate_matchable(target, visited, path, errors);
            } else {
                errors.push(DialectValidationError {
                    path: path.clone(),
                    missing_reference: name.to_string(),
                });
            }
            path.pop();
        }

        if let Some(grammar) = matchable.match_grammar(self) {
            self.validate_matchable(&grammar, visited, path, errors);
        }

        for reference in matchable.validation_references(self) {
            path.push(reference.to_string());
            if let Some(DialectElementType::Matchable(target)) = self.library.get(reference) {
                self.validate_matchable(target, visited, path, errors);
            } else {
                errors.push(DialectValidationError {
                    path: path.clone(),
                    missing_reference: reference.to_string(),
                });
            }
            path.pop();
        }

        for child in matchable.validation_children() {
            self.validate_matchable(child, visited, path, errors);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::grammar::sequence::Bracketed;
    use crate::parser::grammar::{Anything, Ref};

    #[test]
    fn validate_reports_reachable_missing_reference_with_path() {
        let mut dialect = Dialect::new();
        dialect.add([
            (
                "FileSegment".into(),
                Ref::new("StatementSegment").to_matchable().into(),
            ),
            (
                "StatementSegment".into(),
                Ref::new("MissingSegment").to_matchable().into(),
            ),
        ]);

        let error = dialect.validate().unwrap_err();
        assert_eq!(error.missing_reference(), "MissingSegment");
        assert_eq!(
            error.path(),
            ["FileSegment", "StatementSegment", "MissingSegment"]
        );
        assert_eq!(
            error.to_string(),
            "Grammar reference path FileSegment -> StatementSegment -> MissingSegment refers to \
             'MissingSegment' which was not found in the dialect"
        );
    }

    #[test]
    fn validate_ignores_unreachable_inherited_grammar() {
        let mut dialect = Dialect::new();
        dialect.add([
            (
                "FileSegment".into(),
                Ref::new("StatementSegment").to_matchable().into(),
            ),
            (
                "StatementSegment".into(),
                Ref::new("PresentSegment").to_matchable().into(),
            ),
            (
                "PresentSegment".into(),
                Ref::keyword("SELECT").to_matchable().into(),
            ),
            (
                "DormantInheritedSegment".into(),
                Ref::new("MissingSegment").to_matchable().into(),
            ),
        ]);
        dialect.add_keyword_to_set("reserved_keywords", "SELECT");
        dialect.expand();

        assert_eq!(dialect.validate(), Ok(()));
    }

    #[test]
    fn validate_reports_missing_anything_terminator() {
        let mut dialect = Dialect::new();
        dialect.add([(
            "FileSegment".into(),
            Anything::new()
                .terminators(vec![Ref::new("MissingSegment").to_matchable()])
                .to_matchable()
                .into(),
        )]);

        let error = dialect.validate().unwrap_err();
        assert_eq!(error.missing_reference(), "MissingSegment");
        assert_eq!(error.path(), ["FileSegment", "MissingSegment"]);
    }

    #[test]
    fn validate_reports_missing_bracket_pair_reference() {
        let mut dialect = Dialect::new();
        let mut bracketed = Bracketed::new(vec![]);
        bracketed.bracket_type("angle");
        bracketed.bracket_pairs_set = "angle_bracket_pairs";
        dialect.update_bracket_sets(
            "angle_bracket_pairs",
            vec![(
                "angle",
                "StartAngleBracketSegment",
                "MissingEndSegment",
                false,
            )],
        );
        dialect.add([
            ("FileSegment".into(), bracketed.to_matchable().into()),
            (
                "StartAngleBracketSegment".into(),
                Anything::new().to_matchable().into(),
            ),
        ]);

        let error = dialect.validate().unwrap_err();
        assert_eq!(error.missing_reference(), "MissingEndSegment");
        assert_eq!(error.path(), ["FileSegment", "MissingEndSegment"]);
    }

    #[test]
    fn validate_reports_missing_default_bracket_pair_reference() {
        let mut dialect = Dialect::new();
        dialect.update_bracket_sets(
            "bracket_pairs",
            vec![("round", "MissingStartSegment", "EndBracketSegment", false)],
        );
        dialect.add([
            (
                "FileSegment".into(),
                Bracketed::new(vec![]).to_matchable().into(),
            ),
            (
                "EndBracketSegment".into(),
                Anything::new().to_matchable().into(),
            ),
        ]);

        let error = dialect.validate().unwrap_err();
        assert_eq!(error.missing_reference(), "MissingStartSegment");
        assert_eq!(error.path(), ["FileSegment", "MissingStartSegment"]);
    }
}
