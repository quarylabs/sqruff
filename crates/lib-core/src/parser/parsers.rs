use ahash::AHashSet;
use fancy_regex::Regex;
use smol_str::SmolStr;

use super::context::ParseContext;
use super::match_result::{MatchResult, Matched, Span};
use super::matchable::{Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key};
use super::segments::base::ErasedSegment;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::errors::SQLParseError;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedParser {
    template: SyntaxKind,
    target_types: SyntaxSet,
    kind: SyntaxKind,
    optional: bool,
    cache_key: MatchableCacheKey,
}

impl TypedParser {
    pub fn new(template: SyntaxKind, kind: SyntaxKind) -> Self {
        let target_types = SyntaxSet::new(&[template]);

        Self {
            template,
            kind,
            target_types,
            optional: false,
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn is_first_match(&self, segment: &ErasedSegment) -> bool {
        self.target_types.contains(segment.get_type())
    }
}

impl MatchableTrait for TypedParser {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        let _ = (parse_context, crumbs);
        (AHashSet::new(), self.target_types.clone()).into()
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
                span: Span {
                    start: idx,
                    end: idx + 1,
                },
                matched: Matched::Newtype(self.kind).into(),
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
    kind: SyntaxKind,
    optional: bool,
    cache_key: MatchableCacheKey,
}

impl StringParser {
    pub fn new(template: &str, kind: SyntaxKind) -> StringParser {
        let template_upper = template.to_uppercase();
        let simple_set = [template_upper.clone()].into();

        StringParser {
            template: template_upper,
            simple: simple_set,
            kind,
            optional: false,
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn simple(&self, _parse_cx: &ParseContext) -> (AHashSet<String>, AHashSet<String>) {
        (self.simple.clone(), AHashSet::new())
    }

    pub fn is_first_match(&self, segment: &ErasedSegment) -> bool {
        segment.is_code() && self.template.eq_ignore_ascii_case(segment.raw())
    }
}

impl MatchableTrait for StringParser {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

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

        if segment.is_code() && self.template.eq_ignore_ascii_case(segment.raw()) {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: idx + 1,
                },
                matched: Matched::Newtype(self.kind).into(),
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
    pub template: Regex,
    pub anti_template: Option<Regex>,
    kind: SyntaxKind,
    cache_key: MatchableCacheKey,
}

impl PartialEq for RegexParser {
    fn eq(&self, other: &Self) -> bool {
        self.template.as_str() == other.template.as_str()
            && self
                .anti_template
                .as_ref()
                .zip(other.anti_template.as_ref())
                .is_some_and(|(lhs, rhs)| lhs.as_str() == rhs.as_str())
            && self.kind == other.kind
    }
}

impl RegexParser {
    pub fn new(template: &str, kind: SyntaxKind) -> Self {
        let template_pattern = Regex::new(&format!("(?i){}", template)).unwrap();

        Self {
            template: template_pattern,
            anti_template: None,
            kind,
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn anti_template(mut self, anti_template: &str) -> Self {
        self.anti_template = Regex::new(&format!("(?i){anti_template}")).unwrap().into();
        self
    }
}

impl MatchableTrait for RegexParser {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

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
                && !self.anti_template.as_ref().is_some_and(|anti_template| {
                    anti_template
                        .is_match(&segment_raw_upper)
                        .unwrap_or_default()
                })
            {
                return Ok(MatchResult {
                    span: Span {
                        start: idx,
                        end: idx + 1,
                    },
                    matched: Matched::Newtype(self.kind).into(),
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
    kind: SyntaxKind,
    cache: MatchableCacheKey,
}

impl MultiStringParser {
    pub fn new(templates: Vec<String>, kind: SyntaxKind) -> Self {
        let templates = templates
            .iter()
            .map(|template| template.to_ascii_uppercase())
            .collect::<AHashSet<String>>();

        let _simple = templates.clone();

        Self {
            templates: templates.into_iter().collect(),
            simple: _simple.into_iter().collect(),
            kind,
            cache: next_matchable_cache_key(),
        }
    }
}

impl MatchableTrait for MultiStringParser {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

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
                span: Span {
                    start: idx,
                    end: idx + 1,
                },
                matched: Matched::Newtype(self.kind).into(),
                ..<_>::default()
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache
    }
}
