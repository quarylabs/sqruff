use std::collections::HashSet;

use itertools::enumerate;
use uuid::Uuid;

use crate::core::dialects::base::Dialect;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::helpers::trim_non_code_segments;
use crate::core::parser::match_algorithms::prune_options;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::Segment;
use crate::core::parser::types::ParseMode;
use crate::helpers::{capitalize, ToMatchable};

#[derive(Clone, Debug, Hash)]
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

impl Segment for BaseGrammar {}

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
    ) -> Result<MatchResult, SQLParseError> {
        // Placeholder implementation
        Ok(MatchResult::new(Vec::new(), Vec::new()))
    }

    fn cache_key(&self) -> String {
        self.cache_key.clone()
    }
}

#[derive(Clone, Hash)]
pub struct Ref {
    reference: String,
    exclude: Option<Box<dyn Matchable>>,
    terminators: Vec<Box<dyn Matchable>>,
    reset_terminators: bool,
    allow_gaps: bool,
    optional: bool,
}

impl std::fmt::Debug for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<Ref: {}{}>", self.reference, if self.is_optional() { " [opt]" } else { "" })
    }
}

impl Ref {
    // Constructor function
    pub fn new(reference: impl ToString) -> Self {
        Ref {
            reference: reference.to_string(),
            exclude: None,
            terminators: Vec::new(),
            reset_terminators: false,
            allow_gaps: true,
            optional: false,
        }
    }

    pub fn exclude(mut self, exclude: impl ToMatchable) -> Self {
        self.exclude = exclude.to_matchable().into();
        self
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    // Method to get the referenced element
    fn _get_elem(&self, dialect: &Dialect) -> Box<dyn Matchable> {
        dialect.r#ref(&self.reference)
    }

    // Static method to create a Ref instance for a keyword
    pub fn keyword(keyword: &str) -> Self {
        let name = format!("{}KeywordSegment", capitalize(keyword));
        Ref::new(name)
    }
}

impl PartialEq for Ref {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference
            && self.reset_terminators == other.reset_terminators
            && self.allow_gaps == other.allow_gaps
            && self.optional == other.optional
    }
}

impl Eq for Ref {}

impl Segment for Ref {}

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
            if c.contains(&self.reference.as_str()) {
                let loop_string = c.join(" -> ");
                panic!("Self referential grammar detected: {}", loop_string);
            }
        }

        let mut new_crumbs = crumbs.unwrap_or_default();
        new_crumbs.push(&self.reference);

        self._get_elem(parse_context.dialect()).simple(parse_context, Some(new_crumbs))
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        // Implement the logic for `_get_elem`
        let elem = self._get_elem(parse_context.dialect());

        // Check the exclude condition
        if let Some(exclude) = &self.exclude {
            let ctx = parse_context.deeper_match(
                &format!("{}-Exclude", self.reference),
                self.reset_terminators,
                &self.terminators,
                None,
                |this| {
                    if !exclude
                        .match_segments(segments.clone(), this)
                        .map_err(|e| dbg!(e))
                        .map_or(false, |match_result| !match_result.has_match())
                    {
                        return Some(MatchResult::from_unmatched(segments.clone()));
                    }

                    None
                },
            );

            if let Some(ctx) = ctx {
                return Ok(ctx);
            }
        }

        // Match against that. NB We're not incrementing the match_depth here.
        // References shouldn't really count as a depth of match.
        parse_context.deeper_match(
            &self.reference,
            self.reset_terminators,
            &self.terminators,
            None,
            |this| elem.match_segments(segments, this),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Hash)]
struct Anything {}

impl Segment for Anything {}

impl Matchable for Anything {}

#[derive(Clone, Debug, PartialEq, Hash)]
pub struct Nothing {}

impl Nothing {
    pub fn new() -> Self {
        Self {}
    }
}

impl Segment for Nothing {}

impl Matchable for Nothing {
    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        Ok(MatchResult::from_unmatched(segments))
    }
}

pub fn longest_trimmed_match(
    mut segments: &[Box<dyn Segment>],
    matchers: Vec<Box<dyn Matchable>>,
    parse_context: &mut ParseContext,
    trim_noncode: bool,
) -> Result<(MatchResult, Option<Box<dyn Matchable>>), SQLParseError> {
    // Have we been passed an empty list?
    if segments.is_empty() {
        return Ok((MatchResult::from_empty(), None));
    }
    // If presented with no options, return no match
    else if matchers.is_empty() {
        return Ok((MatchResult::from_unmatched(segments.to_vec()), None));
    }

    let available_options = prune_options(&matchers, segments, parse_context);
    if available_options.is_empty() {
        return Ok((MatchResult::from_unmatched(segments.to_vec()), None));
    }

    let mut pre_nc = &[][..];
    let mut post_nc = &[][..];

    if trim_noncode {
        (pre_nc, segments, post_nc) = trim_non_code_segments(segments);
    }

    let mut best_match_length = 0;
    let mut best_match = None;

    for (_idx, matcher) in enumerate(available_options) {
        let match_result = matcher.match_segments(segments.to_vec(), parse_context)?;

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

                Ok((MatchResult::from_matched(matched_segments), matcher.into()))
            } else {
                Ok((match_result, matcher.into()))
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

            Ok((MatchResult { matched_segments, unmatched_segments }, matchable.into()))
        } else {
            Ok((match_result, matchable.into()))
        };
    }

    // If no match at all, return nothing
    Ok((MatchResult::from_unmatched(segments.to_vec()), None))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::core::parser::grammar::anyof::one_of;
    use crate::core::parser::grammar::sequence::Sequence;
    use crate::core::parser::parsers::StringParser;
    use crate::core::parser::segments::keyword::KeywordSegment;
    use crate::core::parser::segments::test_functions::{
        fresh_ansi_dialect, generate_test_segments_func, make_result_tuple, test_segments,
    };
    use crate::helpers::{Boxed, ToMatchable};

    #[test]
    fn test__parser__grammar__ref_eq() {
        // Assuming Ref implements Clone and PartialEq
        let r1 = Ref::new("foo".to_string());
        let r2 = Ref::new("foo".to_string());

        // Rust does not directly compare object identities like Python's `is`,
        // but we can ensure they are not the same object by comparing memory addresses
        assert!(&r1 as *const _ != &r2 as *const _);
        assert_eq!(r1, r2);

        // For lists, we use Vec in Rust
        let mut check_list = vec![r2.clone()];

        // In Rust, we use `contains` to check for presence in a Vec
        assert!(check_list.contains(&r1));

        // Finding the index of an item in a Vec
        let index = check_list.iter().position(|x| *x == r1).expect("Item not found");
        assert_eq!(index, 0);

        // Removing an item from a Vec
        check_list.retain(|x| *x != r1);
        assert!(!check_list.contains(&r1));
    }

    #[test]
    fn test__parser__grammar__ref_repr() {
        // Assuming that Ref has a constructor that accepts a &str and an optional bool
        let r1 = Ref::new("foo".to_string());
        assert_eq!(format!("{:?}", r1), "<Ref: foo>");

        let r2 = Ref::new("bar".to_string()).optional();
        assert_eq!(format!("{:?}", r2), "<Ref: bar [opt]>");
    }

    #[test]
    fn test__parser__grammar_ref_exclude() {
        // Assuming 'Ref' and 'NakedIdentifierSegment' are defined elsewhere
        let ni = Ref::new("NakedIdentifierSegment".to_string()).exclude(Ref::keyword("ABS"));

        // Assuming 'generate_test_segments' and 'fresh_ansi_dialect' are implemented
        // elsewhere
        let ts = generate_test_segments_func(vec!["ABS", "ABSOLUTE"]);
        let mut ctx = ParseContext::new(fresh_ansi_dialect());

        // Assert ABS does not match, due to the exclude
        assert!(
            ni.match_segments(vec![ts[0].clone()], &mut ctx).unwrap().matched_segments.is_empty()
        );

        // Assert ABSOLUTE does match
        assert!(
            !ni.match_segments(vec![ts[1].clone()], &mut ctx).unwrap().matched_segments.is_empty()
        );
    }

    #[test]
    fn test_parser_grammar_nothing() {
        let mut ctx = ParseContext::new(fresh_ansi_dialect());

        assert!(
            Nothing::new()
                .match_segments(test_segments(), &mut ctx)
                .unwrap()
                .matched_segments
                .is_empty()
        );
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
            let matchers = vec![
                StringParser::new(
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
                .to_matchable(),
            ];

            let (m, _) = longest_trimmed_match(
                &test_segments[segments_slice],
                matchers,
                &mut ctx,
                trim_noncode,
            )
            .unwrap();

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
            longest_trimmed_match(&test_segments(), matchers.clone(), &mut ctx, true).unwrap();

        // Check we got a match
        assert!(match_result.has_match());
        // Check we got the right one.
        assert!(matcher.unwrap().dyn_eq(&*matchers[2]));
        // And it matched the first three segments
        assert_eq!(match_result.len(), 3);
    }
}
