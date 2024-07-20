use ahash::AHashSet;
use fancy_regex::Regex;
use smol_str::SmolStr;

use super::context::ParseContext;
use super::match_result::{MatchResult, Matched, Span};
use super::matchable::{next_matchable_cache_key, Matchable, MatchableCacheKey};
use super::segments::base::{ErasedSegment, Segment};
use crate::core::errors::SQLParseError;
use crate::dialects::{SyntaxKind, SyntaxSet};

#[derive(Debug, Clone, PartialEq)]
pub struct TypedParser {
    template: SyntaxKind,
    target_types: SyntaxSet,
    instance_types: Vec<String>,
    optional: bool,
    trim_chars: Option<Vec<char>>,
    cache_key: MatchableCacheKey,
    factory: fn(&dyn Segment) -> ErasedSegment,
}

impl TypedParser {
    pub fn new(
        template: SyntaxKind,
        factory: fn(&dyn Segment) -> ErasedSegment,
        type_: Option<String>,
        optional: bool,
        trim_chars: Option<Vec<char>>,
    ) -> TypedParser {
        let mut instance_types = Vec::new();
        let target_types = SyntaxSet::new(&[template]);

        if let Some(t) = type_.clone() {
            instance_types.push(t);
        }

        TypedParser {
            template,
            factory,
            target_types,
            instance_types,
            optional,
            trim_chars,
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn is_first_match(&self, segment: &dyn Segment) -> bool {
        self.target_types.contains(segment.get_type())
    }
}

impl Segment for TypedParser {}

impl Matchable for TypedParser {
    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        let _ = (parse_context, crumbs);
        (AHashSet::new(), self.target_types).into()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let segment = &segments[idx as usize];
        if segment.is_type(self.template) {
            return Ok(MatchResult {
                span: Span { start: idx, end: idx + 1 },
                matched: Matched::ErasedSegment((self.factory)(&**segment)).into(),
                insert_segments: Vec::new(),
                child_matches: Vec::new(),
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StringParser {
    template: String,
    simple: AHashSet<String>,
    factory: fn(&dyn Segment) -> ErasedSegment,
    type_: Option<String>,
    optional: bool,
    trim_chars: Option<Vec<char>>,
    cache_key: MatchableCacheKey,
}

impl StringParser {
    pub fn new(
        template: &str,
        factory: fn(&dyn Segment) -> ErasedSegment,
        type_: Option<String>,
        optional: bool,
        trim_chars: Option<Vec<char>>,
    ) -> StringParser {
        let template_upper = template.to_uppercase();
        let simple_set = [template_upper.clone()].into();

        StringParser {
            template: template_upper,
            simple: simple_set,
            factory,
            type_,
            optional,
            trim_chars,
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn simple(&self, _parse_cx: &ParseContext) -> (AHashSet<String>, AHashSet<String>) {
        (self.simple.clone(), AHashSet::new())
    }

    pub fn is_first_match(&self, segment: &dyn Segment) -> bool {
        segment.is_code() && self.template.eq_ignore_ascii_case(&segment.raw())
    }
}

impl Segment for StringParser {}

impl Matchable for StringParser {
    fn is_optional(&self) -> bool {
        self.optional
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        (self.simple.clone(), SyntaxSet::EMPTY).into()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let segment = &segments[idx as usize];

        if segment.is_code() && self.template.eq_ignore_ascii_case(&segment.raw()) {
            return Ok(MatchResult {
                span: Span { start: idx, end: idx + 1 },
                matched: Matched::ErasedSegment((self.factory)(&**segment)).into(),
                insert_segments: Vec::new(),
                child_matches: Vec::new(),
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

#[derive(Debug, Clone)]
pub struct RegexParser {
    pub(crate) template: Regex,
    pub(crate) anti_template: Option<Regex>,
    factory: fn(&dyn Segment) -> ErasedSegment,
    cache_key: MatchableCacheKey,
}

impl PartialEq for RegexParser {
    fn eq(&self, other: &Self) -> bool {
        self.template.as_str() == other.template.as_str()
            && self
                .anti_template
                .as_ref()
                .zip(other.anti_template.as_ref())
                .map_or(false, |(lhs, rhs)| lhs.as_str() == rhs.as_str())
            && self.factory == other.factory
    }
}

impl RegexParser {
    pub fn new(
        template: &str,
        factory: fn(&dyn Segment) -> ErasedSegment,
        _type_: Option<String>,
        _optional: bool,
        anti_template: Option<String>,
        _trim_chars: Option<Vec<String>>,
    ) -> Self {
        let anti_template_pattern =
            anti_template.map(|anti_template| Regex::new(&format!("(?i){anti_template}")).unwrap());
        let template_pattern = Regex::new(&format!("(?i){}", template)).unwrap();

        Self {
            template: template_pattern,
            anti_template: anti_template_pattern,
            factory,
            cache_key: next_matchable_cache_key(),
        }
    }
}

impl Segment for RegexParser {}

impl Matchable for RegexParser {
    fn is_optional(&self) -> bool {
        unimplemented!()
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        // Does this matcher support a uppercase hash matching route?
        // Regex segment does NOT for now. We might need to later for efficiency.
        None
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let segment = &segments[idx as usize];
        let segment_raw_upper =
            SmolStr::from_iter(segment.raw().chars().map(|ch| ch.to_ascii_uppercase()));
        if let Some(result) = self.template.find(&segment_raw_upper).ok().flatten() {
            if result.as_str() == segment_raw_upper
                && !self.anti_template.as_ref().map_or(false, |anti_template| {
                    anti_template.is_match(&segment_raw_upper).unwrap_or_default()
                })
            {
                return Ok(MatchResult {
                    span: Span { start: idx, end: idx + 1 },
                    matched: Matched::ErasedSegment((self.factory)(&**segment)).into(),
                    insert_segments: Vec::new(),
                    child_matches: Vec::new(),
                });
            }
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultiStringParser {
    templates: AHashSet<String>,
    simple: AHashSet<String>,
    factory: fn(&dyn Segment) -> ErasedSegment,
    cache: MatchableCacheKey,
}

impl MultiStringParser {
    pub fn new(
        templates: Vec<String>,
        factory: fn(&dyn Segment) -> ErasedSegment,
        _type_: Option<String>,
        _optional: bool,
        _trim_chars: Option<Vec<String>>,
    ) -> Self {
        let templates = templates
            .iter()
            .map(|template| template.to_ascii_uppercase())
            .collect::<AHashSet<String>>();

        let _simple = templates.clone();

        Self {
            templates: templates.into_iter().collect(),
            simple: _simple.into_iter().collect(),
            factory,
            cache: next_matchable_cache_key(),
        }
    }
}

impl Segment for MultiStringParser {}

impl Matchable for MultiStringParser {
    fn is_optional(&self) -> bool {
        todo!()
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        (self.simple.clone(), SyntaxSet::EMPTY).into()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let segment = &segments[idx as usize];

        if segment.is_code() && self.templates.contains(&segment.raw().to_ascii_uppercase()) {
            return Ok(MatchResult {
                span: Span { start: idx, end: idx + 1 },
                matched: Matched::ErasedSegment((self.factory)(&**segment)).into(),
                ..<_>::default()
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache
    }
}
