use std::collections::BTreeSet;

use ahash::AHashSet;
use fancy_regex::Regex;
use uuid::Uuid;

use super::context::ParseContext;
use super::match_result::MatchResult;
use super::matchable::Matchable;
use super::segments::base::{ErasedSegment, Segment};
use crate::core::errors::SQLParseError;
use crate::helpers::HashableFancyRegex;

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct TypedParser {
    template: String,
    target_types: BTreeSet<String>,
    instance_types: Vec<String>,
    /* raw_class: RawSegment, // Type for raw_class */
    optional: bool,
    trim_chars: Option<Vec<char>>,

    factory: fn(&dyn Segment) -> ErasedSegment,
}

impl TypedParser {
    pub fn new(
        template: &str,
        factory: fn(&dyn Segment) -> ErasedSegment,
        /* raw_class: RawSegment, */
        type_: Option<String>,
        optional: bool,
        trim_chars: Option<Vec<char>>,
    ) -> TypedParser {
        let mut instance_types = Vec::new();
        let target_types = [template.to_string()].iter().cloned().collect();

        if let Some(t) = type_.clone() {
            instance_types.push(t);
        }

        // TODO:
        // if type_.as_ref() != Some(&raw_class.get_type()) {
        //     instance_types.push(raw_class.get_type());
        // }
        // if !raw_class.class_is_type(template) {
        //     instance_types.push(template.to_string());
        // }

        TypedParser {
            template: template.to_string(),
            factory,
            target_types,
            instance_types,
            /* raw_class, */
            optional,
            trim_chars,
        }
    }

    fn match_single(&self, segment: &dyn Segment) -> Option<ErasedSegment> {
        // Check if the segment matches the first condition.
        if !self.is_first_match(segment) {
            return None;
        }

        // // Check if the segment is already of the correct type.
        // // Assuming RawSegment has a `get_type` method and `_instance_types` is a
        // Vec<String> if segment.is_type(&self.raw_class) && segment.get_type()
        // == self._instance_types[0] {     return Some(segment.clone()); //
        // Assuming BaseSegment implements Clone }

        // Otherwise, create a new match segment.
        // Assuming _make_match_from_segment is a method that returns RawSegment
        // Some(self.make_match_from_segment(segment))
        (self.factory)(segment).into()
    }

    pub fn is_first_match(&self, segment: &dyn Segment) -> bool {
        self.target_types.iter().any(|typ| segment.is_type(typ))
    }
}

impl Segment for TypedParser {}

impl Matchable for TypedParser {
    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        let _ = (parse_context, crumbs);
        (AHashSet::new(), self.target_types.clone().into_iter().collect()).into()
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

// Assuming RawSegment and BaseSegment are defined elsewhere in your Rust code.
#[derive(Hash, Clone, Debug, PartialEq)]
pub struct StringParser {
    template: String,
    simple: BTreeSet<String>,
    factory: fn(&dyn Segment) -> ErasedSegment,
    type_: Option<String>, /* Renamed `type` to `type_` because `type` is a reserved keyword in
                            * Rust */
    optional: bool,
    trim_chars: Option<Vec<char>>,
    cache_key: String,
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
        let simple_set = [template_upper.clone()].iter().cloned().collect();

        StringParser {
            template: template_upper,
            simple: simple_set,
            factory,
            type_,
            optional,
            trim_chars,
            cache_key: Uuid::new_v4().hyphenated().to_string(),
        }
    }

    pub fn simple(&self, _parse_cx: &ParseContext) -> (AHashSet<String>, AHashSet<String>) {
        // Assuming SimpleHintType is a type alias for (&AHashSet<String>,
        // AHashSet<String>)
        (self.simple.clone().into_iter().collect(), AHashSet::new())
    }

    pub fn is_first_match(&self, segment: &dyn Segment) -> bool {
        // Assuming BaseSegment has methods `raw_upper` and `is_code`
        Some(&self.template) == segment.get_raw_upper().as_ref() && segment.is_code()
    }
}

impl StringParser {
    fn match_single(&self, segment: &dyn Segment) -> Option<ErasedSegment> {
        // Check if the segment matches the first condition.
        if !self.is_first_match(segment) {
            return None;
        }

        // // Check if the segment is already of the correct type.
        // // Assuming RawSegment has a `get_type` method and `_instance_types` is a
        // Vec<String> if segment.is_type(&self.raw_class) && segment.get_type()
        // == self._instance_types[0] {     return Some(segment.clone()); //
        // Assuming BaseSegment implements Clone }

        // Otherwise, create a new match segment.
        // Assuming _make_match_from_segment is a method that returns RawSegment
        // Some(self.make_match_from_segment(segment))
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
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        (self.simple.clone().into_iter().collect(), <_>::default()).into()
    }

    #[allow(unused_variables)]
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

    fn cache_key(&self) -> String {
        self.cache_key.clone()
    }
}

#[derive(Hash, Debug, Clone)]
#[allow(clippy::derived_hash_with_manual_eq)]
pub struct RegexParser {
    template: String,
    anti_template: Option<String>,
    _template: HashableFancyRegex,
    _anti_template: HashableFancyRegex,
    factory: fn(&dyn Segment) -> ErasedSegment,
    cache_key: String, // Add other fields as needed
}

impl PartialEq for RegexParser {
    fn eq(&self, other: &Self) -> bool {
        self.template == other.template
            && self.anti_template == other.anti_template
            // && self._template == other._template
            // && self._anti_template == other._anti_template
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
        _trim_chars: Option<Vec<String>>, // Assuming trim_chars is a vector of strings
    ) -> Self {
        let anti_template_or_empty = anti_template.clone().unwrap_or_default();
        let anti_template_pattern = Regex::new(&format!("(?i){anti_template_or_empty}")).unwrap();
        let template_pattern = Regex::new(&format!("(?i){}", template)).unwrap();

        Self {
            template: template.to_string(),
            anti_template,
            _template: HashableFancyRegex(template_pattern),
            _anti_template: HashableFancyRegex(anti_template_pattern),
            factory, // Initialize other fields here
            cache_key: Uuid::new_v4().hyphenated().to_string(),
        }
    }

    fn is_first_match(&self, segment: &dyn Segment) -> bool {
        if segment.get_raw().unwrap().is_empty() {
            // TODO: Handle this case
            return false;
        }

        let segment_raw_upper = segment.get_raw().unwrap().to_ascii_uppercase();
        if let Some(result) = self._template.find(&segment_raw_upper).ok().flatten() {
            if result.as_str() == segment_raw_upper {
                if let Some(_anti_template) = &self.anti_template {
                    if self._anti_template.is_match(&segment_raw_upper).unwrap_or_default() {
                        return false;
                    }
                }
                return true;
            }
        }
        false
    }

    fn match_single(&self, segment: &dyn Segment) -> Option<ErasedSegment> {
        // Check if the segment matches the first condition.
        if !self.is_first_match(segment) {
            return None;
        }

        // // Check if the segment is already of the correct type.
        // // Assuming RawSegment has a `get_type` method and `_instance_types` is a
        // Vec<String> if segment.is_type(&self.raw_class) && segment.get_type()
        // == self._instance_types[0] {     return Some(segment.clone()); //
        // Assuming BaseSegment implements Clone }

        // Otherwise, create a new match segment.
        // Assuming _make_match_from_segment is a method that returns RawSegment
        // Some(self.make_match_from_segment(segment))
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
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
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

    fn cache_key(&self) -> String {
        self.cache_key.clone()
    }
}

#[derive(Hash, Clone, Debug, PartialEq)]
pub struct MultiStringParser {
    templates: BTreeSet<String>,
    _simple: BTreeSet<String>,
    factory: fn(&dyn Segment) -> ErasedSegment,
    // Add other fields as needed
}

impl MultiStringParser {
    pub fn new(
        templates: Vec<String>,
        factory: fn(&dyn Segment) -> ErasedSegment, // Assuming RawSegment is defined elsewhere
        _type_: Option<String>,
        _optional: bool,
        _trim_chars: Option<Vec<String>>, // Assuming trim_chars is a vector of strings
    ) -> Self {
        let templates = templates
            .iter()
            .map(|template| template.to_ascii_uppercase())
            .collect::<AHashSet<String>>();
        let _simple = templates.clone();

        Self {
            templates: templates.into_iter().collect(),
            _simple: _simple.into_iter().collect(),
            factory,
            // Initialize other fields here
        }
    }

    #[allow(dead_code)]
    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<String>>,
    ) -> (AHashSet<String>, AHashSet<String>) {
        // Return the simple options (templates) and an empty set of hints
        (self._simple.clone().into_iter().collect(), AHashSet::new())
    }

    fn is_first_match(&self, segment: &dyn Segment) -> bool {
        // Check if the segment is code and its raw_upper is in the templates
        segment.is_code()
            && self.templates.contains(&segment.get_raw().unwrap().to_ascii_uppercase())
    }

    fn match_single(&self, segment: &dyn Segment) -> Option<ErasedSegment> {
        // Check if the segment matches the first condition.
        if !self.is_first_match(segment) {
            return None;
        }

        // // Check if the segment is already of the correct type.
        // // Assuming RawSegment has a `get_type` method and `_instance_types` is a
        // Vec<String> if segment.is_type(&self.raw_class) && segment.get_type()
        // == self._instance_types[0] {     return Some(segment.clone()); //
        // Assuming BaseSegment implements Clone }

        // Otherwise, create a new match segment.
        // Assuming _make_match_from_segment is a method that returns RawSegment
        // Some(self.make_match_from_segment(segment))
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
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        (self._simple.clone().into_iter().collect(), <_>::default()).into()
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

    fn cache_key(&self) -> String {
        todo!()
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
            (AHashSet::new(), ["single_quote".into()].into()).into()
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
                    segment.get_raw().unwrap(),
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

        assert_eq!(result1.get_raw().unwrap(), "foo");

        // Doesn't match when it shouldn't
        let result = parser.match_segments(&segments[1..], &mut ctx).unwrap();
        assert_eq!(result.matched_segments, &[]);
    }

    // This function will contain the common test logic
    //  fn test_parser_typedparser_rematch_impl(new_type: Option<&str>) {
    //     struct ExampleSegment; // Example definition of ExampleSegment
    //     struct TypedParser;    // Example definition of TypedParser
    //     struct ParseContext;   // Example definition of ParseContext

    //     // Example implementations for these structs/functions will be needed

    //     let pre_match_types: AHashSet<&str> = ["single_quote", "raw",
    // "base"].iter().cloned().collect();     let mut post_match_types:
    // AHashSet<&str> = ["example", "single_quote", "raw",
    // "base"].iter().cloned().collect();

    //     let mut kwargs = AHashMap::new();
    //     let mut expected_type = "example";
    //     if let Some(t) = new_type {
    //         post_match_types.insert(t);
    //         kwargs.insert("type", t);
    //         expected_type = t;
    //     }

    //     let segments = generate_test_segments_func(["'foo'"]); // Placeholder
    // for actual implementation

    //     assert_eq!(segments[0].class_types(), &pre_match_types);

    //     let parser = TypedParser::new("single_quote", ExampleSegment,
    // kwargs);     let ctx = ParseContext::new();

    //     let match1 = parser.match(&segments, &ctx);
    //     assert!(match1.is_some());
    //     let match1 = match1.unwrap();
    //     assert_eq!(match1.matched_segments()[0].class_types(),
    // &post_match_types);     assert_eq!(match1.matched_segments()[0].
    // get_type(), expected_type);     assert_eq!(match1.
    // matched_segments()[0].to_tuple(true), (expected_type, "'foo'"));

    //     let match2 = parser.match(match1.matched_segments(), &ctx);
    //     assert!(match2.is_some());
    //     let match2 = match2.unwrap();
    //     assert_eq!(match2.matched_segments()[0].class_types(),
    // &post_match_types);     assert_eq!(match2.matched_segments()[0].
    // get_type(), expected_type);     assert_eq!(match2.
    // matched_segments()[0].to_tuple(true), (expected_type, "'foo'")); }

    // #[test]
    // fn test_parser_typedparser_rematch_none() {
    //     test_parser_typedparser_rematch_impl(None);
    // }

    // #[test]
    // fn test_parser_typedparser_rematch_bar() {
    //     test_parser_typedparser_rematch_impl(Some("bar"));
    // }
}
