use std::collections::HashSet;

use uuid::Uuid;

use crate::core::{
    dialects::base::Dialect,
    parser::{
        context::ParseContext, match_result::MatchResult, matchable::Matchable,
        segments::base::Segment, types::ParseMode,
    },
};

#[derive(Clone, Debug)]
pub struct BaseGrammar {
    elements: Vec<Box<dyn Matchable>>,
    allow_gaps: bool,
    optional: bool,
    terminators: Vec<Box<dyn Matchable>>,
    reset_terminators: bool,
    parse_mode: ParseMode,
    cache_key: String,
}

impl PartialEq for BaseGrammar {
    fn eq(&self, other: &Self) -> bool {
        // self.elements == other.elements &&
        self.allow_gaps == other.allow_gaps
            && self.optional == other.optional
        //   && self.terminators == other.terminators
            && self.reset_terminators == other.reset_terminators
            && self.parse_mode == other.parse_mode
            && self.cache_key == other.cache_key
    }
}

impl BaseGrammar {
    pub fn new(
        elements: Vec<Box<dyn Matchable>>,
        allow_gaps: bool,
        optional: bool,
        terminators: Vec<Box<dyn Matchable>>,
        reset_terminators: bool,
        parse_mode: ParseMode,
    ) -> Self {
        let cache_key = Uuid::new_v4().to_string();

        Self {
            elements,
            allow_gaps,
            optional,
            terminators,
            reset_terminators,
            parse_mode,
            cache_key,
        }
    }

    // Placeholder for the _resolve_ref method
    fn _resolve_ref(elem: Box<dyn Matchable>) -> Box<dyn Matchable> {
        // Placeholder implementation
        elem
    }

    // Placeholder for the _longest_trimmed_match method
    #[allow(unused_variables)]
    fn _longest_trimmed_match(
        segments: &[Box<dyn Segment>],
        matchers: Vec<Box<dyn Matchable>>,
        parse_context: &ParseContext,
        trim_noncode: bool,
    ) -> (MatchResult, Option<Box<dyn Matchable>>) {
        // Have we been passed an empty list?
        if segments.is_empty() {
            return (MatchResult::from_empty(), None);
        }
        // If presented with no options, return no match
        else if matchers.is_empty() {
            return (MatchResult::from_unmatched(segments), None);
        }

        unimplemented!()
    }
}

#[allow(unused_variables)]
impl Matchable for BaseGrammar {
    fn is_optional(&self) -> bool {
        self.optional
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        // Placeholder implementation
        None
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> MatchResult {
        // Placeholder implementation
        MatchResult::new(Vec::new(), Vec::new())
    }

    fn cache_key(&self) -> String {
        self.cache_key.clone()
    }
}

#[derive(Clone)]
pub struct Ref {
    _ref: String,
    exclude: Option<Box<dyn Matchable>>, // Using Box<dyn Matchable> for dynamic dispatch
    terminators: Vec<Box<dyn Matchable>>,
    reset_terminators: bool,
    allow_gaps: bool,
    optional: bool,
}

impl std::fmt::Debug for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<Ref: {}{}>",
            self._ref,
            if self.is_optional() { " [opt]" } else { "" }
        )
    }
}

impl Ref {
    // Constructor function
    pub fn new(
        reference: String,
        exclude: Option<Box<dyn Matchable>>,
        terminators: Vec<Box<dyn Matchable>>,
        reset_terminators: bool,
        allow_gaps: bool,
        optional: bool,
    ) -> Self {
        Ref {
            _ref: reference,
            exclude,
            terminators,
            reset_terminators,
            allow_gaps,
            optional,
        }
    }

    // Method to get the referenced element
    fn _get_elem(&self, dialect: &Dialect) -> Box<dyn Matchable> {
        dialect.r#ref(&self._ref)
    }

    // Static method to create a Ref instance for a keyword
    pub fn keyword(keyword: &str, optional: Option<bool>) -> Self {
        let optional = optional.unwrap_or_default();
        let name = format!("{}KeywordSegment", keyword.to_uppercase());
        Ref::new(name, None, vec![], false, true, optional)
    }
}

impl PartialEq for Ref {
    fn eq(&self, other: &Self) -> bool {
        self._ref == other._ref
            && self.reset_terminators == other.reset_terminators
            && self.allow_gaps == other.allow_gaps
            && self.optional == other.optional
    }
}

impl Eq for Ref {}

impl Matchable for Ref {
    fn is_optional(&self) -> bool {
        self.optional
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        // Implementation...
        unimplemented!()
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> MatchResult {
        // Implement the logic for `_get_elem`
        let elem = self._get_elem(parse_context.dialect());

        // Check the exclude condition
        if self.exclude.is_some() {
            let ctx = parse_context.deeper_match(
                &format!("{}-Exclude", self._ref),
                self.reset_terminators,
                &self.terminators,
                None,
                |this| {
                    if !self
                        .exclude
                        .as_ref()
                        .unwrap()
                        .match_segments(segments.clone(), this)
                        .matched_segments
                        .is_empty()
                    {
                        return Some(MatchResult::from_unmatched(&segments));
                    }

                    None
                },
            );

            if ctx.is_some() {
                return ctx.unwrap();
            }
        }

        parse_context.deeper_match(
            &self._ref,
            self.reset_terminators,
            &self.terminators,
            None,
            |this| elem.match_segments(segments, this),
        )
    }

    fn cache_key(&self) -> String {
        // Implementation...
        unimplemented!()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Anything {}

impl Matchable for Anything {
    fn is_optional(&self) -> bool {
        todo!()
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        todo!()
    }

    fn match_segments(
        &self,
        _segments: Vec<Box<dyn Segment>>,
        _parse_context: &mut ParseContext,
    ) -> MatchResult {
        todo!()
    }

    fn cache_key(&self) -> String {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Nothing {}

impl Matchable for Nothing {
    fn is_optional(&self) -> bool {
        todo!()
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        todo!()
    }

    fn match_segments(
        &self,
        _segments: Vec<Box<dyn Segment>>,
        _parse_context: &mut ParseContext,
    ) -> MatchResult {
        todo!()
    }

    fn cache_key(&self) -> String {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{
        dialects::init::{dialect_selector, get_default_dialect},
        parser::segments::test_functions::generate_test_segments_func,
    };

    use super::*; // Import necessary items from the parent module

    #[test]
    fn test__parser__grammar__ref_eq() {
        // Assuming Ref implements Clone and PartialEq
        let r1 = Ref::new("foo".to_string(), None, vec![], false, true, false);
        let r2 = Ref::new("foo".to_string(), None, vec![], false, true, false);

        // Rust does not directly compare object identities like Python's `is`,
        // but we can ensure they are not the same object by comparing memory addresses
        assert!(&r1 as *const _ != &r2 as *const _);
        assert_eq!(r1, r2);

        // For lists, we use Vec in Rust
        let mut check_list = vec![r2.clone()];

        // In Rust, we use `contains` to check for presence in a Vec
        assert!(check_list.contains(&r1));

        // Finding the index of an item in a Vec
        let index = check_list
            .iter()
            .position(|x| *x == r1)
            .expect("Item not found");
        assert_eq!(index, 0);

        // Removing an item from a Vec
        check_list.retain(|x| *x != r1);
        assert!(!check_list.contains(&r1));
    }

    #[test]
    fn test__parser__grammar__ref_repr() {
        // Assuming that Ref has a constructor that accepts a &str and an optional bool
        let r1 = Ref::new("foo".to_string(), None, vec![], false, true, false);
        assert_eq!(format!("{:?}", r1), "<Ref: foo>");

        let r2 = Ref::new("bar".to_string(), None, vec![], false, true, true);
        assert_eq!(format!("{:?}", r2), "<Ref: bar [opt]>");
    }

    #[test]
    fn test__parser__grammar_ref_exclude() {
        // Assuming 'Ref' and 'NakedIdentifierSegment' are defined elsewhere
        let ni = Ref::new(
            "NakedIdentifierSegment".to_string(),
            Some(Box::new(Ref::keyword("ABS", None))), // Exclude
            vec![], // Terminators, assuming an empty Vec for this test
            false,  // reset_terminators
            false,  // allow_gaps
            false,  // optional
        );

        // Assuming 'generate_test_segments' and 'fresh_ansi_dialect' are implemented elsewhere
        let ts = generate_test_segments_func(vec!["ABS", "ABSOLUTE"]);
        let mut ctx = ParseContext::new(dialect_selector(get_default_dialect()).unwrap());

        // Assert ABS does not match, due to the exclude
        assert!(ni
            .match_segments(vec![ts[0].clone()], &mut ctx)
            .matched_segments
            .is_empty());

        // Assert ABSOLUTE does match
        assert!(!ni
            .match_segments(vec![ts[1].clone()], &mut ctx)
            .matched_segments
            .is_empty());
    }
}
