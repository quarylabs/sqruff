use std::collections::HashSet;

use itertools::enumerate;
use uuid::Uuid;

use crate::core::{
    dialects::base::Dialect,
    parser::{
        context::ParseContext, helpers::trim_non_code_segments, match_algorithms::prune_options,
        match_result::MatchResult, matchable::Matchable, segments::base::Segment, types::ParseMode,
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
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        if let Some(ref c) = crumbs {
            if c.contains(&self._ref.as_str()) {
                let loop_string = c.join(" -> ");
                panic!("Self referential grammar detected: {}", loop_string);
            }
        }

        let mut new_crumbs = crumbs.unwrap_or_else(Vec::new);
        new_crumbs.push(&self._ref);

        self._get_elem(parse_context.dialect())
            .simple(parse_context, Some(new_crumbs))
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
                        return Some(MatchResult::from_unmatched(segments.clone()));
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

impl Nothing {
    fn new() -> Self {
        Self {}
    }
}

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
        segments: Vec<Box<dyn Segment>>,
        _parse_context: &mut ParseContext,
    ) -> MatchResult {
        MatchResult::from_unmatched(segments)
    }

    fn cache_key(&self) -> String {
        todo!()
    }
}

pub fn longest_trimmed_match(
    mut segments: &[Box<dyn Segment>],
    matchers: Vec<Box<dyn Matchable>>,
    parse_context: &mut ParseContext,
    trim_noncode: bool,
) -> (MatchResult, Option<Box<dyn Matchable>>) {
    // Have we been passed an empty list?
    if segments.is_empty() {
        return (MatchResult::from_empty(), None);
    }
    // If presented with no options, return no match
    else if matchers.is_empty() {
        return (MatchResult::from_unmatched(segments.to_vec()), None);
    }

    let available_options = prune_options(&matchers, segments, parse_context);

    if available_options.is_empty() {
        return (MatchResult::from_unmatched(segments.to_vec()), None);
    }

    let mut pre_nc = &[][..];
    let mut post_nc = &[][..];

    if trim_noncode {
        (pre_nc, segments, post_nc) = trim_non_code_segments(segments);
    }

    let mut best_match_length = 0;
    let mut best_match = None;

    for (_idx, matcher) in enumerate(available_options) {
        let match_result = matcher.match_segments(segments.to_vec(), parse_context);

        // No match. Skip this one.
        if !match_result.has_match() {
            continue;
        }

        if match_result.is_complete() {
            // Just return it! (WITH THE RIGHT OTHER STUFF)
            return if trim_noncode {
                let mut matched_segments = pre_nc.to_vec();
                matched_segments.extend(match_result.matched_segments);
                matched_segments.extend(post_nc.to_vec());

                (MatchResult::from_matched(matched_segments), matcher.into())
            } else {
                (match_result, matcher.into())
            };
        } else if match_result.has_match()
            && match_result.trimmed_matched_length() > best_match_length
        {
            best_match_length = match_result.trimmed_matched_length();
            best_match = (match_result, matcher).into();
        }
    }

    // If we get here, then there wasn't a complete match. If we
    // has a best_match, return that.
    if best_match_length > 0 {
        let (match_result, matchable) = best_match.unwrap();
        return if trim_noncode {
            let mut matched_segments = pre_nc.to_vec();
            matched_segments.extend(match_result.matched_segments);

            let mut unmatched_segments = match_result.unmatched_segments;
            unmatched_segments.extend(post_nc.iter().cloned());

            (
                MatchResult {
                    matched_segments,
                    unmatched_segments,
                },
                matchable.into(),
            )
        } else {
            (match_result, matchable.into())
        };
    }

    // If no match at all, return nothing
    (MatchResult::from_unmatched(segments.to_vec()), None)
}

#[cfg(test)]
mod tests {
    use crate::{
        core::parser::{
            grammar::{anyof::one_of, sequence::Sequence},
            parsers::StringParser,
            segments::{
                keyword::KeywordSegment,
                test_functions::{
                    fresh_ansi_dialect, generate_test_segments_func, make_result_tuple,
                    test_segments,
                },
            },
        },
        helpers::ToMatchable,
        traits::Boxed,
    };

    use pretty_assertions::assert_eq;

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
        let mut ctx = ParseContext::new(fresh_ansi_dialect());

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

    #[test]
    fn test_parser_grammar_nothing() {
        let mut ctx = ParseContext::new(fresh_ansi_dialect());

        assert!(Nothing::new()
            .match_segments(test_segments(), &mut ctx)
            .matched_segments
            .is_empty());
    }

    #[test]
    fn test__parser__grammar__base__longest_trimmed_match__basic() {
        let test_segments = test_segments();
        let cases = [
            // Matching the first element of the list
            (0..test_segments.len(), "bar", false, (0..1).into()),
            // Matching with a bit of whitespace before
            (1..test_segments.len(), "foo", true, (1..3).into()),
            // Matching with a bit of whitespace before (not trim_noncode)
            (1..test_segments.len(), "foo", false, None),
            // Matching with whitespace after
            (0..2, "bar", true, (0..2).into()),
        ];

        let mut ctx = ParseContext::new(fresh_ansi_dialect());
        for (segments_slice, matcher_keyword, trim_noncode, result_slice) in cases {
            let matchers = vec![StringParser::new(
                matcher_keyword,
                |segment| {
                    KeywordSegment::new(
                        segment.get_raw().unwrap(),
                        segment.get_position_marker().unwrap(),
                    )
                    .boxed()
                },
                None,
                false,
                None,
            )
            .to_matchable()];

            let (m, _) = longest_trimmed_match(
                &test_segments[segments_slice],
                matchers,
                &mut ctx,
                trim_noncode,
            );

            let expected_result =
                make_result_tuple(result_slice, &[matcher_keyword], &test_segments);

            assert_eq!(expected_result, m.matched_segments);
        }
    }

    #[test]
    fn test__parser__grammar__base__longest_trimmed_match__adv() {
        let bs = StringParser::new(
            "bar",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap(),
                )
                .boxed()
            },
            None,
            false,
            None,
        )
        .boxed();

        let fs = StringParser::new(
            "foo",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap(),
                )
                .boxed()
            },
            None,
            false,
            None,
        )
        .boxed();

        let matchers: Vec<Box<dyn Matchable>> = vec![
            bs.clone(),
            fs.clone(),
            Sequence::new(vec![bs.clone(), fs.clone()]).boxed(),
            one_of(vec![bs.clone(), fs.clone()]).boxed(),
            Sequence::new(vec![bs, fs]).boxed(),
        ];

        let mut ctx = ParseContext::new(fresh_ansi_dialect());
        // Matching the first element of the list
        let (match_result, matcher) =
            longest_trimmed_match(&test_segments(), matchers.clone(), &mut ctx, true);

        // Check we got a match
        assert!(match_result.has_match());
        // Check we got the right one.
        assert!(matcher.unwrap().dyn_eq(&*matchers[2]));
        // And it matched the first three segments
        assert_eq!(match_result.len(), 3);
    }
}
