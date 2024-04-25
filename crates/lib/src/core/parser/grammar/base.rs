use std::borrow::Cow;
use std::cell::OnceCell;
use std::ops::Deref;
use std::rc::Rc;

use ahash::AHashSet;
use itertools::enumerate;
use uuid::Uuid;

use crate::core::dialects::base::Dialect;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::helpers::trim_non_code_segments;
use crate::core::parser::match_algorithms::{greedy_match, prune_options};
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::{ErasedSegment, Segment};
use crate::core::parser::types::ParseMode;
use crate::helpers::{capitalize, ToMatchable};
use crate::stack::ensure_sufficient_stack;

#[derive(Clone, Debug, Hash)]
#[allow(clippy::derived_hash_with_manual_eq)]
pub struct BaseGrammar {
    elements: Vec<Rc<dyn Matchable>>,
    allow_gaps: bool,
    optional: bool,
    terminators: Vec<Rc<dyn Matchable>>,
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
        elements: Vec<Rc<dyn Matchable>>,
        allow_gaps: bool,
        optional: bool,
        terminators: Vec<Rc<dyn Matchable>>,
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
    fn _resolve_ref(elem: Rc<dyn Matchable>) -> Rc<dyn Matchable> {
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
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        // Placeholder implementation
        None
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        // Placeholder implementation
        Ok(MatchResult::new(Vec::new(), Vec::new()))
    }

    fn cache_key(&self) -> String {
        self.cache_key.clone()
    }
}

#[derive(Clone)]
#[allow(clippy::derived_hash_with_manual_eq)]
pub struct Ref {
    reference: Cow<'static, str>,
    exclude: Option<Rc<dyn Matchable>>,
    terminators: Vec<Rc<dyn Matchable>>,
    reset_terminators: bool,
    allow_gaps: bool,
    optional: bool,
    cache_key: String,
    simple_cache: OnceCell<Option<(AHashSet<String>, AHashSet<String>)>>,
}

impl std::fmt::Debug for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<Ref: {}{}>", self.reference, if self.is_optional() { " [opt]" } else { "" })
    }
}

impl Ref {
    // Constructor function
    pub fn new(reference: impl Into<Cow<'static, str>>) -> Self {
        Ref {
            reference: reference.into(),
            exclude: None,
            terminators: Vec::new(),
            reset_terminators: false,
            allow_gaps: true,
            optional: false,
            cache_key: Uuid::new_v4().hyphenated().to_string(),
            simple_cache: OnceCell::new(),
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
    fn _get_elem(&self, dialect: &Dialect) -> Rc<dyn Matchable> {
        dialect.r#ref(&self.reference)
    }

    // Static method to create a Ref instance for a keyword
    pub fn keyword(keyword: &str) -> Self {
        let name = capitalize(keyword) + "KeywordSegment";
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
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        self.simple_cache
            .get_or_init(|| {
                if let Some(ref c) = crumbs {
                    if c.contains(&self.reference.deref()) {
                        let loop_string = c.join(" -> ");
                        panic!("Self referential grammar detected: {}", loop_string);
                    }
                }

                let mut new_crumbs = crumbs.unwrap_or_default();
                new_crumbs.push(&self.reference);

                self._get_elem(parse_context.dialect()).simple(parse_context, Some(new_crumbs))
            })
            .clone()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
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
                        .match_segments(segments, this)
                        .map_err(|e| dbg!(e))
                        .map_or(false, |match_result| !match_result.has_match())
                    {
                        return Some(MatchResult::from_unmatched(segments.to_vec()));
                    }

                    None
                },
            );

            if let Some(ctx) = ctx {
                return Ok(ctx);
            }
        }

        ensure_sufficient_stack(|| {
            // Match against that. NB We're not incrementing the match_depth here.
            // References shouldn't really count as a depth of match.
            parse_context.deeper_match(
                &self.reference,
                self.reset_terminators,
                &self.terminators,
                None,
                |this| elem.match_segments(segments, this),
            )
        })
    }

    fn cache_key(&self) -> String {
        self.cache_key.clone()
    }
}

#[derive(Clone, Debug, Hash)]
#[allow(clippy::derived_hash_with_manual_eq)]
pub struct Anything {
    terminators: Vec<Rc<dyn Matchable>>,
}

impl PartialEq for Anything {
    #[allow(unused_variables)]
    fn eq(&self, other: &Self) -> bool {
        unimplemented!()
    }
}

impl Default for Anything {
    fn default() -> Self {
        Self::new()
    }
}

impl Anything {
    pub fn new() -> Self {
        Self { terminators: Vec::new() }
    }

    pub fn terminators(mut self, terminators: Vec<Rc<dyn Matchable>>) -> Self {
        self.terminators = terminators;
        self
    }
}

impl Segment for Anything {}

impl Matchable for Anything {
    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if self.terminators.is_empty() {
            return Ok(MatchResult::from_matched(segments.to_vec()));
        }

        greedy_match(segments.to_vec(), parse_context, self.terminators.clone(), false)
    }
}

#[derive(Clone, Debug, PartialEq, Hash)]
pub struct Nothing {}

impl Default for Nothing {
    fn default() -> Self {
        Self::new()
    }
}

impl Nothing {
    pub fn new() -> Self {
        Self {}
    }
}

impl Segment for Nothing {}

impl Matchable for Nothing {
    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        Ok(MatchResult::from_unmatched(segments.to_vec()))
    }
}

pub fn longest_trimmed_match(
    mut segments: &[ErasedSegment],
    matchers: Vec<Rc<dyn Matchable>>,
    parse_context: &mut ParseContext,
    trim_noncode: bool,
) -> Result<(MatchResult, Option<Rc<dyn Matchable>>), SQLParseError> {
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

    let loc_key = (
        segments[0].get_raw().unwrap(),
        segments[0].get_position_marker().unwrap().working_loc(),
        segments[0].get_type(),
        segments.len(),
    );

    let mut best_match_length = 0;
    let mut best_match = None;

    for (_idx, matcher) in enumerate(available_options) {
        let matcher_key = matcher.cache_key();

        let match_result = match parse_context
            .check_parse_cache(loc_key.clone(), matcher_key.clone())
        {
            Some(match_result) => match_result,
            None => {
                let match_result = matcher.match_segments(segments, parse_context)?;
                parse_context.put_parse_cache(loc_key.clone(), matcher_key, match_result.clone());
                match_result
            }
        };

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
    use crate::helpers::ToErasedSegment;

    #[test]
    fn test__parser__grammar__ref_eq() {
        // Assuming Ref implements Clone and PartialEq
        let r1 = Ref::new("foo".to_string());
        let r2 = Ref::new("foo".to_string());

        // Rust does not directly compare object identities like Python's `is`,
        // but we can ensure they are not the same object by comparing memory addresses
        assert_ne!(&r1 as *const _, &r2 as *const _);
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
        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        // Assert ABS does not match, due to the exclude
        assert!(ni.match_segments(&[ts[0].clone()], &mut ctx).unwrap().matched_segments.is_empty());

        // Assert ABSOLUTE does match
        assert!(
            !ni.match_segments(&[ts[1].clone()], &mut ctx).unwrap().matched_segments.is_empty()
        );
    }

    #[test]
    fn test_parser_grammar_nothing() {
        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        assert!(
            Nothing::new()
                .match_segments(&test_segments(), &mut ctx)
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

        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());
        for (segments_slice, matcher_keyword, trim_noncode, result_slice) in cases {
            let matchers = vec![
                StringParser::new(
                    matcher_keyword,
                    |segment| {
                        KeywordSegment::new(
                            segment.get_raw().unwrap(),
                            segment.get_position_marker().unwrap().into(),
                        )
                        .to_erased_segment()
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
        let bs = Rc::new(StringParser::new(
            "bar",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        )) as Rc<dyn Matchable>;

        let fs = Rc::new(StringParser::new(
            "foo",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        )) as Rc<dyn Matchable>;

        let matchers: Vec<Rc<dyn Matchable>> = vec![
            bs.clone(),
            fs.clone(),
            Rc::new(Sequence::new(vec![bs.clone(), fs.clone()])),
            Rc::new(one_of(vec![bs.clone(), fs.clone()])),
            Rc::new(Sequence::new(vec![bs, fs])),
        ];

        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());
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
