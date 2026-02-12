use ahash::AHashSet;
use fancy_regex::Regex;

use super::matchable::{Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key};
use super::segments::ErasedSegment;
use crate::dialects::Dialect;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};

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

    pub(crate) fn template(&self) -> SyntaxKind {
        self.template
    }

    pub(crate) fn kind(&self) -> SyntaxKind {
        self.kind
    }
}

impl MatchableTrait for TypedParser {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn simple(
        &self,
        dialect: &Dialect,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        let _ = (dialect, crumbs);
        (AHashSet::new(), self.target_types.clone()).into()
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeParser {
    cache_key: MatchableCacheKey,
}

impl CodeParser {
    pub fn new() -> Self {
        Self {
            cache_key: next_matchable_cache_key(),
        }
    }
}

impl Default for CodeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchableTrait for CodeParser {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn simple(
        &self,
        _dialect: &Dialect,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        None
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

    pub(crate) fn template(&self) -> &str {
        &self.template
    }

    pub(crate) fn kind(&self) -> SyntaxKind {
        self.kind
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
        _dialect: &Dialect,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        (self.simple.clone(), SyntaxSet::EMPTY).into()
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
        let template_pattern = Regex::new(&format!("(?i){template}")).unwrap();

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

    pub(crate) fn kind(&self) -> SyntaxKind {
        self.kind
    }
}

impl MatchableTrait for RegexParser {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn is_optional(&self) -> bool {
        false
    }

    fn simple(
        &self,
        _dialect: &Dialect,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        // Does this matcher support a uppercase hash matching route?
        // Regex segment does NOT for now. We might need to later for efficiency.
        None
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

    pub(crate) fn templates(&self) -> Vec<&str> {
        self.templates.iter().map(|it| it.as_str()).collect()
    }

    pub(crate) fn kind(&self) -> SyntaxKind {
        self.kind
    }
}

impl MatchableTrait for MultiStringParser {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn is_optional(&self) -> bool {
        false
    }

    fn simple(
        &self,
        _dialect: &Dialect,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        (self.simple.clone(), SyntaxSet::EMPTY).into()
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache
    }
}
