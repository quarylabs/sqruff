use ahash::AHashSet;
use fancy_regex::Regex;
use smol_str::SmolStr;

use super::context::ParseContext;
use super::match_result::MatchResult;
use super::matchable::Matchable;
use super::segments::base::{ErasedSegment, Segment};
use crate::core::errors::SQLParseError;
use crate::helpers::next_cache_key;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedParser {
    template: &'static str,
    target_types: AHashSet<&'static str>,
    instance_types: Vec<String>,
    optional: bool,
    trim_chars: Option<Vec<char>>,
    cache_key: u64,
    factory: fn(&dyn Segment) -> ErasedSegment,
}

impl TypedParser {
    pub fn new(
        template: &'static str,
        factory: fn(&dyn Segment) -> ErasedSegment,
        type_: Option<String>,
        optional: bool,
        trim_chars: Option<Vec<char>>,
    ) -> TypedParser {
        let mut instance_types = Vec::new();
        let target_types = [template].into();

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
            cache_key: next_cache_key(),
        }
    }

    fn match_single(&self, segment: &dyn Segment) -> Option<ErasedSegment> {
        if !self.is_first_match(segment) {
            return None;
        }

        (self.factory)(segment).into()
    }

    pub fn is_first_match(&self, segment: &dyn Segment) -> bool {
        self.target_types.iter().any(|typ| segment.is_type(typ))
    }
}

impl Segment for TypedParser {}

impl Matchable for TypedParser {
    fn cache_key(&self) -> u64 {
        self.cache_key
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
        let _ = (parse_context, crumbs);
        (AHashSet::new(), self.target_types.clone()).into()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if !segments.is_empty() {
            let segment = &*segments[0];
            if let Some(seg) = self.match_single(segment) {
                return Ok(MatchResult::new(vec![seg], segments[1..].to_vec()));
            }
        };

        Ok(MatchResult::from_unmatched(segments.to_vec()))
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
    cache_key: u64,
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
            cache_key: next_cache_key(),
        }
    }

    pub fn simple(&self, _parse_cx: &ParseContext) -> (AHashSet<String>, AHashSet<String>) {
        (self.simple.clone(), AHashSet::new())
    }

    pub fn is_first_match(&self, segment: &dyn Segment) -> bool {
        segment.is_code() && self.template.eq_ignore_ascii_case(&segment.raw())
    }
}

impl StringParser {
    fn match_single(&self, segment: &dyn Segment) -> Option<ErasedSegment> {
        if !self.is_first_match(segment) {
            return None;
        }

        (self.factory)(segment).into()
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
    ) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
        (self.simple.clone().into_iter().collect(), <_>::default()).into()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if let Some((first, rest)) = segments.split_first()
            && let Some(seg) = self.match_single(&**first)
        {
            return Ok(MatchResult::new(vec![seg], rest.to_vec()));
        }

        Ok(MatchResult::from_unmatched(segments.to_vec()))
    }

    fn cache_key(&self) -> u64 {
        self.cache_key
    }
}

#[derive(Debug, Clone)]
pub struct RegexParser {
    template: Regex,
    anti_template: Option<Regex>,
    factory: fn(&dyn Segment) -> ErasedSegment,
    cache_key: u64,
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
            cache_key: next_cache_key(),
        }
    }

    fn is_first_match(&self, segment: &dyn Segment) -> bool {
        if segment.raw().is_empty() {
            // TODO: Handle this case
            return false;
        }

        let segment_raw_upper =
            SmolStr::from_iter(segment.raw().chars().map(|ch| ch.to_ascii_uppercase()));
        if let Some(result) = self.template.find(&segment_raw_upper).ok().flatten() {
            if result.as_str() == segment_raw_upper {
                return !self.anti_template.as_ref().map_or(false, |anti_template| {
                    anti_template.is_match(&segment_raw_upper).unwrap_or_default()
                });
            }
        }
        false
    }

    fn match_single(&self, segment: &dyn Segment) -> Option<ErasedSegment> {
        // Check if the segment matches the first condition.
        if !self.is_first_match(segment) {
            return None;
        }

        (self.factory)(segment).into()
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
    ) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
        // Does this matcher support a uppercase hash matching route?
        // Regex segment does NOT for now. We might need to later for efficiency.
        None
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if !segments.is_empty() {
            let segment = &*segments[0];
            if let Some(seg) = self.match_single(segment) {
                return Ok(MatchResult::new(vec![seg], segments[1..].to_vec()));
            }
        }

        Ok(MatchResult::from_unmatched(segments.to_vec()))
    }

    fn cache_key(&self) -> u64 {
        self.cache_key
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultiStringParser {
    templates: AHashSet<String>,
    simple: AHashSet<String>,
    factory: fn(&dyn Segment) -> ErasedSegment,
    cache_key: u64,
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
            cache_key: next_cache_key(),
        }
    }

    fn is_first_match(&self, segment: &dyn Segment) -> bool {
        segment.is_code() && self.templates.contains(&segment.raw().to_ascii_uppercase())
    }

    fn match_single(&self, segment: &dyn Segment) -> Option<ErasedSegment> {
        if !self.is_first_match(segment) {
            return None;
        }

        (self.factory)(segment).into()
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
    ) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
        (self.simple.clone(), <_>::default()).into()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if !segments.is_empty() {
            let segment = &*segments[0];
            if let Some(seg) = self.match_single(segment) {
                return Ok(MatchResult::new(vec![seg], segments[1..].to_vec()));
            }
        }

        Ok(MatchResult::from_unmatched(segments.to_vec()))
    }

    fn cache_key(&self) -> u64 {
        self.cache_key
    }
}

#[cfg(test)]
mod tests {
    use ahash::AHashSet;

    use super::TypedParser;
    use crate::core::dialects::init::dialect_selector;
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::matchable::Matchable;
    use crate::core::parser::parsers::{MultiStringParser, RegexParser, StringParser};
    use crate::core::parser::segments::keyword::KeywordSegment;
    use crate::core::parser::segments::test_functions::generate_test_segments_func;
    use crate::helpers::ToErasedSegment;

    // Test the simple method of TypedParser
    #[test]
    fn test__parser__typedparser__simple() {
        let parser = TypedParser::new(
            "single_quote",
            |_| unimplemented!(),
            <_>::default(),
            <_>::default(),
            <_>::default(),
        );

        let dialect = dialect_selector("ansi").unwrap();
        let parse_cx = ParseContext::new(&dialect, <_>::default());

        assert_eq!(
            parser.simple(&parse_cx, None),
            (AHashSet::new(), ["single_quote"].into()).into()
        );
    }

    #[test]
    fn test_stringparser_simple() {
        // Initialize an instance of StringParser
        let parser = StringParser::new("foo", |_| todo!(), None, false, None);

        // Create a dummy ParseContext
        let dialect = dialect_selector("ansi").unwrap();
        let parse_cx = ParseContext::new(&dialect, <_>::default());

        // Perform the test
        assert_eq!(parser.simple(&parse_cx), (["FOO".to_string()].into(), AHashSet::new()));
    }

    #[test]
    fn test_parser_regexparser_simple() {
        let parser = RegexParser::new("b.r", |_| todo!(), None, false, None, None);
        let dialect = dialect_selector("ansi").unwrap();
        let ctx = ParseContext::new(&dialect, <_>::default());
        assert_eq!(parser.simple(&ctx, None), None);
    }

    #[test]
    fn test_parser_multistringparser_match() {
        let parser = MultiStringParser::new(
            vec!["foo".to_string(), "bar".to_string()],
            /* KeywordSegment */
            |segment| {
                KeywordSegment::new(
                    segment.raw().into(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        );
        let dialect = dialect_selector("ansi").unwrap();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        // Check directly
        let segments = generate_test_segments_func(vec!["foo", "fo"]);

        // Matches when it should
        let result = parser.match_segments(&segments[0..1], &mut ctx).unwrap();
        let result1 = &result.matched_segments[0];

        assert_eq!(result1.raw(), "foo");

        // Doesn't match when it shouldn't
        let result = parser.match_segments(&segments[1..], &mut ctx).unwrap();
        assert_eq!(result.matched_segments, &[]);
    }
}
