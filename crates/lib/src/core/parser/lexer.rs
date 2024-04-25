use std::fmt::{Debug, Display, Formatter};
use std::ops::Range;

use dyn_clone::DynClone;
use fancy_regex::{Error, Regex};

use super::markers::PositionMarker;
use super::segments::base::ErasedSegment;
use super::segments::meta::EndOfFile;
use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::errors::{SQLLexError, ValueError};
use crate::core::parser::segments::base::{
    Segment, SegmentConstructorFn, UnlexableSegment, UnlexableSegmentNewArgs,
};
use crate::core::slice_helpers::{is_zero_slice, offset_slice};
use crate::core::templaters::base::TemplatedFile;

/// An element matched during lexing.
#[derive(Debug, Clone)]
pub struct LexedElement {
    raw: String,
    matcher: Box<dyn Matcher>,
}

impl LexedElement {
    pub fn new(raw: String, matcher: Box<dyn Matcher>) -> Self {
        LexedElement { raw, matcher }
    }
}

/// A LexedElement, bundled with it's position in the templated file.
#[derive(Debug)]
pub struct TemplateElement {
    raw: String,
    template_slice: Range<usize>,
    matcher: Box<dyn Matcher>,
}

impl TemplateElement {
    /// Make a TemplateElement from a LexedElement.
    pub fn from_element(element: LexedElement, template_slice: Range<usize>) -> Self {
        TemplateElement { raw: element.raw, template_slice, matcher: element.matcher }
    }

    pub fn to_segment(
        &self,
        pos_marker: PositionMarker,
        subslice: Option<Range<usize>>,
    ) -> ErasedSegment {
        let slice = subslice.map_or_else(|| self.raw.clone(), |slice| self.raw[slice].to_string());
        self.matcher.construct_segment(slice, pos_marker)
    }
}

/// A class to hold matches from the lexer.
#[derive(Debug)]
pub struct LexMatch {
    forward_string: String,
    pub elements: Vec<LexedElement>,
}

#[allow(clippy::needless_arbitrary_self_type)]
impl LexMatch {
    /// A LexMatch is truthy if it contains a non-zero number of matched
    /// elements.
    pub fn is_non_empty(self: &Self) -> bool {
        !self.elements.is_empty()
    }
}

pub trait CloneMatcher {
    fn clone_box(&self) -> Box<dyn Matcher>;
}

impl<T: Matcher + DynClone> CloneMatcher for T {
    fn clone_box(&self) -> Box<dyn Matcher> {
        Box::new(dyn_clone::clone(self))
    }
}

#[allow(clippy::needless_arbitrary_self_type)]
pub trait Matcher: Debug + DynClone + CloneMatcher + 'static {
    /// The name of the matcher.
    fn get_name(self: &Self) -> String;
    /// Given a string, match what we can and return the rest.
    fn match_(self: &Self, forward_string: String) -> Result<LexMatch, ValueError>;
    /// Use regex to find a substring.
    fn search(self: &Self, forward_string: &str) -> Option<Range<usize>>;

    /// Access methods that need to be implemented by the subclass.

    /// Get the sub-divider for this matcher.
    fn get_sub_divider(self: &Self) -> Option<Box<dyn Matcher>>;

    fn get_trim_post_subdivide(self: &Self) -> Option<Box<dyn Matcher>>;

    fn _subdivide(self: &Self, matched: LexedElement) -> Vec<LexedElement> {
        if let Some(sub_divider) = &self.get_sub_divider() {
            let mut elem_buff: Vec<LexedElement> = vec![];

            let mut str_buff = matched.raw;
            while !str_buff.is_empty() {
                // Iterate through subdividing as appropriate
                let div_pos = sub_divider.clone().search(&str_buff);
                if let Some(div_pos) = div_pos {
                    // Found a division
                    let trimmed_elems =
                        self._trim_match(str_buff[..div_pos.start].to_string().as_str());
                    let div_elem = LexedElement::new(
                        str_buff[div_pos.start..div_pos.end].to_string(),
                        sub_divider.clone(),
                    );
                    elem_buff.extend_from_slice(&trimmed_elems);
                    elem_buff.push(div_elem);
                    str_buff = str_buff[div_pos.end..].to_string();
                } else {
                    // No more division matches. Trim?
                    let trimmed_elems = self._trim_match(&str_buff);
                    elem_buff.extend_from_slice(&trimmed_elems);
                    break;
                }
            }
            elem_buff
        } else {
            vec![matched]
        }
    }

    /// Given a string, trim if we are allowed to.
    fn _trim_match(self: &Self, matched_str: &str) -> Vec<LexedElement> {
        let mut elem_buff = Vec::new();
        let mut content_buff = String::new();
        let mut str_buff = String::from(matched_str);

        if let Some(trim_post_subdivide) = self.get_trim_post_subdivide() {
            while !str_buff.is_empty() {
                if let Some(trim_pos) = trim_post_subdivide.clone().search(&str_buff) {
                    let start = trim_pos.start;
                    let end = trim_pos.end;

                    if start == 0 {
                        elem_buff.push(LexedElement::new(
                            str_buff[..end].to_string(),
                            trim_post_subdivide.clone(),
                        ));
                        str_buff = str_buff[end..].to_string();
                    } else if end == str_buff.len() {
                        elem_buff.push(LexedElement::new(
                            format!("{}{}", content_buff, &str_buff[..start]),
                            trim_post_subdivide.clone(),
                        ));
                        elem_buff.push(LexedElement::new(
                            str_buff[start..end].to_string(),
                            trim_post_subdivide.clone(),
                        ));
                        content_buff.clear();
                        str_buff.clear();
                    } else {
                        content_buff.push_str(&str_buff[..end]);
                        str_buff = str_buff[end..].to_string();
                    }
                } else {
                    break;
                }
            }
            if !content_buff.is_empty() || !str_buff.is_empty() {
                elem_buff.push(LexedElement::new(
                    format!("{}{}", content_buff, str_buff),
                    self.clone_box(),
                ));
            }
        }

        elem_buff
    }

    fn construct_segment(&self, _raw: String, _pos_marker: PositionMarker) -> ErasedSegment {
        unimplemented!("{}", std::any::type_name::<Self>());
    }
}

dyn_clone::clone_trait_object!(Matcher);

impl Display for dyn Matcher {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Matcher({})", self.get_name())
    }
}

/// This singleton matcher matches strings exactly.
/// This is the simplest usable matcher, but it also defines some of the
/// mechanisms for more complicated matchers, which may simply override the
/// `_match` function rather than the public `match` function.  This acts as
/// the base class for matchers.
#[derive(Clone)]
pub struct StringLexer<SegmentArgs: 'static + Clone> {
    name: &'static str,
    template: &'static str,
    segment_constructor: SegmentConstructorFn<SegmentArgs>,
    segment_args: SegmentArgs,
    sub_divider: Option<Box<dyn Matcher>>,
    trim_post_subdivide: Option<Box<dyn Matcher>>,
}

impl<SegmentArgs: Clone + Debug> StringLexer<SegmentArgs> {
    pub fn new(
        name: &'static str,
        template: &'static str,
        segment_constructor: SegmentConstructorFn<SegmentArgs>,
        segment_args: SegmentArgs,
        sub_divider: Option<Box<dyn Matcher>>,
        trim_post_subdivide: Option<Box<dyn Matcher>>,
    ) -> Self {
        StringLexer {
            name,
            template,
            segment_constructor,
            segment_args,
            sub_divider,
            trim_post_subdivide,
        }
    }

    /// The private match function. Just look for a literal string.
    fn _match(&self, forward_string: &str) -> Option<LexedElement> {
        if forward_string.starts_with(self.template) {
            Some(LexedElement { raw: self.template.to_string(), matcher: Box::new(self.clone()) })
        } else {
            None
        }
    }

    /// Given a string, trim if we are allowed to.
    fn _trim_match(&self, _matched_string: String) -> Vec<LexedElement> {
        panic!("Not implemented")
    }

    /// Given a string, subdivide if we area allowed to.
    fn _subdivide(&self, matched: LexedElement) -> Vec<LexedElement> {
        if let Some(sub_divider) = &self.sub_divider {
            let mut elem_buff: Vec<LexedElement> = vec![];

            let mut str_buff = matched.raw;
            while !str_buff.is_empty() {
                // Iterate through subdividing as appropriate
                let div_pos = self.sub_divider.clone().unwrap().search(&str_buff);
                if let Some(div_pos) = div_pos {
                    // Found a division
                    let trimmed_elems = self._trim_match(str_buff[..div_pos.start].to_string());
                    let div_elem = LexedElement::new(
                        str_buff[div_pos.start..div_pos.end].to_string(),
                        sub_divider.clone(),
                    );
                    elem_buff.extend_from_slice(&trimmed_elems);
                    elem_buff.push(div_elem);
                    str_buff = str_buff[div_pos.end..].to_string();
                } else {
                    // No more division matches. Trim?
                    let trimmed_elems = self._trim_match(str_buff);
                    elem_buff.extend_from_slice(&trimmed_elems);
                    break;
                }
            }
            elem_buff
        } else {
            vec![matched]
        }
    }
}

impl<SegmentArgs: Debug + Clone> Debug for StringLexer<SegmentArgs> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "StringLexer({})", self.name)
    }
}

impl<SegmentArgs: Clone + Debug> Display for StringLexer<SegmentArgs> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "StringLexer({})", self.template)
    }
}

impl<SegmentArgs: Clone + Debug> Matcher for StringLexer<SegmentArgs> {
    fn get_name(&self) -> String {
        self.template.to_string()
    }

    /// Given a string, match what we can and return the rest.
    fn match_(&self, forward_string: String) -> Result<LexMatch, ValueError> {
        if forward_string.is_empty() {
            return Err(ValueError::new(String::from("Unexpected empty string!")));
        };
        let matched = self._match(&forward_string);
        match matched {
            Some(matched) => {
                let length = matched.raw.len();
                let new_elements = self._subdivide(matched);
                Ok(LexMatch {
                    forward_string: forward_string[length..].to_string(),
                    elements: new_elements,
                })
            }
            None => Ok(LexMatch { forward_string: forward_string.to_string(), elements: vec![] }),
        }
    }

    fn search(&self, forward_string: &str) -> Option<Range<usize>> {
        forward_string.find(self.template).map(|start| start..start + self.template.len())
    }

    fn get_sub_divider(&self) -> Option<Box<dyn Matcher>> {
        self.sub_divider.clone()
    }

    fn get_trim_post_subdivide(&self) -> Option<Box<dyn Matcher>> {
        self.trim_post_subdivide.clone()
    }

    fn construct_segment(&self, raw: String, pos_marker: PositionMarker) -> ErasedSegment {
        (self.segment_constructor)(&raw, &pos_marker, self.segment_args.clone())
    }
}

/// This RegexLexer matches based on regular expressions.
#[derive(Clone)]
pub struct RegexLexer<SegmentArgs: 'static + Clone> {
    name: &'static str,
    template: Regex,
    segment_constructor: SegmentConstructorFn<SegmentArgs>,
    segment_args: SegmentArgs,
    sub_divider: Option<Box<dyn Matcher>>,
    trim_post_subdivide: Option<Box<dyn Matcher>>,
}

impl<SegmentArgs: Clone + Debug> RegexLexer<SegmentArgs> {
    #[allow(clippy::result_large_err)]
    pub fn new(
        name: &'static str,
        regex: &str,
        segment_constructor: SegmentConstructorFn<SegmentArgs>,
        segment_args: SegmentArgs,
        sub_divider: Option<Box<dyn Matcher>>,
        trim_post_subdivide: Option<Box<dyn Matcher>>,
    ) -> Result<Self, Error> {
        Ok(RegexLexer {
            name,
            template: Regex::new(regex)?,
            segment_constructor,
            segment_args,
            sub_divider,
            trim_post_subdivide,
        })
    }

    /// Use regexes to match chunks.
    pub fn _match(&self, forward_string: &str) -> Option<LexedElement> {
        if let Ok(Some(matched)) = self.template.find(forward_string) {
            if matched.start() == 0 {
                return Some(LexedElement {
                    raw: matched.as_str().to_string(),
                    matcher: Box::new(self.clone()),
                });
            }
        }
        None
    }
}

impl<SegmentArgs: Debug + Clone> Debug for RegexLexer<SegmentArgs> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegexLexer({})", self.name)
    }
}

impl<SegmentArgs: Clone + Debug> Display for RegexLexer<SegmentArgs> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegexLexer({})", self.get_name())
    }
}

impl<SegmentArgs: Clone + Debug> Matcher for RegexLexer<SegmentArgs> {
    fn get_name(&self) -> String {
        self.template.as_str().to_string()
    }

    /// Given a string, match what we can and return the rest.
    fn match_(&self, forward_string: String) -> Result<LexMatch, ValueError> {
        if forward_string.is_empty() {
            return Err(ValueError::new(String::from("Unexpected empty string!")));
        };
        let matched = self._match(&forward_string);
        match matched {
            Some(matched) => {
                let length = matched.raw.len();
                let new_elements = self._subdivide(matched);

                Ok(LexMatch {
                    forward_string: forward_string[length..].to_string(),
                    elements: new_elements,
                })
            }
            None => Ok(LexMatch { forward_string: forward_string.to_string(), elements: vec![] }),
        }
    }

    /// Use regex to find a substring.
    fn search(&self, forward_string: &str) -> Option<Range<usize>> {
        if let Ok(Some(matched)) = self.template.find(forward_string) {
            let match_str = matched.as_str();
            if !match_str.is_empty() {
                return Some(matched.range());
            } else {
                panic!(
                    "Zero length Lex item returned from '{}'. Report this as a bug.",
                    self.get_name()
                );
            }
        }
        None
    }

    fn get_sub_divider(&self) -> Option<Box<dyn Matcher>> {
        self.sub_divider.clone()
    }

    fn get_trim_post_subdivide(&self) -> Option<Box<dyn Matcher>> {
        self.trim_post_subdivide.clone()
    }

    fn construct_segment(&self, raw: String, pos_marker: PositionMarker) -> ErasedSegment {
        (self.segment_constructor)(&raw, &pos_marker, self.segment_args.clone())
    }
}

/// The Lexer class actually does the lexing step.
pub struct Lexer<'a> {
    config: &'a FluffConfig,
    last_resort_lexer: Box<dyn Matcher>,
}

pub enum StringOrTemplate {
    String(String),
    Template(TemplatedFile),
}

impl<'a> Lexer<'a> {
    /// Create a new lexer.
    pub fn new(config: &'a FluffConfig, _dialect: Option<Dialect>) -> Self {
        let last_resort_lexer = RegexLexer::new(
            "last_resort",
            "[^\t\n.]*",
            &UnlexableSegment::create,
            UnlexableSegmentNewArgs { expected: None },
            None,
            None,
        )
        .expect("Unable to create last resort lexer");
        Lexer { config, last_resort_lexer: Box::new(last_resort_lexer) }
    }

    pub fn lex(
        &self,
        raw: StringOrTemplate,
    ) -> Result<(Vec<ErasedSegment>, Vec<SQLLexError>), ValueError> {
        // Make sure we've got a string buffer and a template regardless of what was
        // passed in.
        let (mut str_buff, template) = match raw {
            StringOrTemplate::String(s) => (s.clone(), TemplatedFile::from_string(s.to_string())),
            StringOrTemplate::Template(f) => (f.to_string(), f),
        };

        // Lex the string to get a tuple of LexedElement
        let mut element_buffer: Vec<LexedElement> = Vec::new();
        loop {
            let res =
                Lexer::lex_match(&str_buff, self.config.get_dialect().lexer_matchers()).unwrap();
            element_buffer.extend(res.elements);
            if !res.forward_string.is_empty() {
                // If we STILL can't match, then just panic out.
                let resort_res = self.last_resort_lexer.match_(str_buff.to_string())?;
                str_buff = resort_res.forward_string;
                element_buffer.extend(resort_res.elements);
            } else {
                break;
            }
        }

        // Map tuple LexedElement to list of TemplateElement.
        // This adds the template_slice to the object.
        let templated_buffer = Lexer::map_template_slices(element_buffer, template.clone());
        // Turn lexed elements into segments.
        let segments = self.elements_to_segments(templated_buffer, template);

        Ok((segments, Vec::new()))
    }

    /// Generate any lexing errors for any un-lex-ables.
    ///
    /// TODO: Taking in an iterator, also can make the typing better than use
    /// unwrap.
    #[allow(dead_code)]
    fn violations_from_segments(segments: Vec<impl Segment>) -> Vec<SQLLexError> {
        segments
            .into_iter()
            .filter(|s| s.is_type("unlexable"))
            .map(|s| {
                SQLLexError::new(
                    format!(
                        "Unable to lex characters: {}",
                        s.get_raw().unwrap().chars().take(10).collect::<String>()
                    ),
                    s.get_position_marker().unwrap(),
                )
            })
            .collect()
    }

    /// Iteratively match strings using the selection of sub-matchers.
    fn lex_match(
        forward_string: &str,
        lexer_matchers: &[Box<dyn Matcher>],
    ) -> Result<LexMatch, ValueError> {
        let mut elem_buff: Vec<LexedElement> = vec![];
        let mut forward_string = forward_string.to_string();

        loop {
            if forward_string.is_empty() {
                return Ok(LexMatch {
                    forward_string: forward_string.to_string(),
                    elements: elem_buff,
                });
            };

            let mut matched = false;

            for matcher in lexer_matchers {
                let res = matcher.match_(forward_string.to_string())?;
                if !res.elements.is_empty() {
                    // If we have new segments then whoop!
                    elem_buff.append(res.elements.clone().as_mut());
                    forward_string = res.forward_string;
                    // Cycle back around again and start with the top
                    // matcher again.
                    matched = true;
                    break;
                }
            }

            // We've got so far, but now can't match. Return
            if !matched {
                return Ok(LexMatch {
                    forward_string: forward_string.to_string(),
                    elements: elem_buff,
                });
            }
        }
    }

    /// Create a tuple of TemplateElement from a tuple of LexedElement.
    ///
    /// This adds slices in the templated file to the original lexed
    /// elements. We'll need this to work out the position in the source
    /// file.
    /// TODO Can this vec be turned into an iterator and return iterator to make
    /// lazy?
    fn map_template_slices(
        elements: Vec<LexedElement>,
        template: TemplatedFile,
    ) -> Vec<TemplateElement> {
        let mut idx = 0;
        let mut templated_buff: Vec<TemplateElement> = vec![];
        for element in elements {
            let template_slice = offset_slice(idx, element.raw.len());
            idx += element.raw.len();

            templated_buff
                .push(TemplateElement::from_element(element.clone(), template_slice.clone()));

            let templated_string = template.get_templated_string().unwrap();
            if templated_string[template_slice.clone()] != element.raw {
                panic!(
                    "Template and lexed elements do not match. This should never happen {:?} != \
                     {:?}",
                    element.raw, &templated_string[template_slice]
                );
            }
        }
        templated_buff
    }

    /// Convert a tuple of lexed elements into a tuple of segments.

    fn elements_to_segments(
        &self,
        elements: Vec<TemplateElement>,
        templated_file: TemplatedFile,
    ) -> Vec<ErasedSegment> {
        let mut segments = iter_segments(elements, templated_file.clone());

        // Add an end of file marker
        let position_maker = segments
            .last()
            .map(|segment| segment.get_position_marker().unwrap())
            .unwrap_or_else(|| PositionMarker::from_point(0, 0, templated_file, None, None));
        segments.push(EndOfFile::create(position_maker));

        segments
    }
}

fn iter_segments(
    lexed_elements: Vec<TemplateElement>,
    templated_file: TemplatedFile,
) -> Vec<ErasedSegment> {
    let mut result = Vec::new();
    // An index to track where we've got to in the templated file.
    let tfs_idx = 0;
    // We keep a map of previous block locations in case they re-occur.
    // let block_stack = BlockTracker()
    let templated_file_slices = &templated_file.clone().sliced_file;

    // Now work out source slices, and add in template placeholders.
    for element in lexed_elements.into_iter() {
        let consumed_element_length = 0;
        let mut stashed_source_idx = None;

        for (mut tfs_idx, tfs) in templated_file_slices
            .iter()
            .skip(tfs_idx)
            .enumerate()
            .map(|(i, tfs)| (i + tfs_idx, tfs))
        {
            // Is it a zero slice?
            if is_zero_slice(tfs.templated_slice.clone()) {
                let _slice = if tfs_idx + 1 < templated_file_slices.len() {
                    templated_file_slices[tfs_idx + 1].clone().into()
                } else {
                    None
                };

                _handle_zero_length_slice();

                continue;
            }

            if tfs.slice_type == "literal" {
                let tfs_offset = tfs.source_slice.start - tfs.templated_slice.start;

                // NOTE: Greater than OR EQUAL, to include the case of it matching
                // length exactly.
                if element.template_slice.end <= tfs.templated_slice.end {
                    let slice_start = stashed_source_idx.unwrap_or_else(|| {
                        element.template_slice.start + consumed_element_length + tfs_offset
                    });

                    result.push(element.to_segment(
                        PositionMarker::new(
                            slice_start..element.template_slice.end + tfs_offset,
                            element.template_slice.clone(),
                            templated_file.clone(),
                            None,
                            None,
                        ),
                        Some(consumed_element_length..element.raw.len()),
                    ));

                    // If it was an exact match, consume the templated element too.
                    #[allow(unused_assignments)]
                    if element.template_slice.end == tfs.templated_slice.end {
                        tfs_idx += 1
                    }
                    // In any case, we're done with this element. Move on
                    break;
                } else if element.template_slice.start == tfs.templated_slice.end {
                    // Did we forget to move on from the last tfs and there's
                    // overlap?
                    // NOTE: If the rest of the logic works, this should never
                    // happen.
                    // lexer_logger.debug("     NOTE: Missed Skip")  # pragma: no cover
                    continue;
                } else {
                    // This means that the current lexed element spans across
                    // multiple templated file slices.
                    // lexer_logger.debug("     Consuming whole spanning literal")
                    // This almost certainly means there's a templated element
                    // in the middle of a whole lexed element.

                    // What we do here depends on whether we're allowed to split
                    // lexed elements. This is basically only true if it's whitespace.
                    // NOTE: We should probably make this configurable on the
                    // matcher object, but for now we're going to look for the
                    // name of the lexer.
                    if element.matcher.get_name() == "whitespace" {
                        if stashed_source_idx.is_some() {
                            panic!("Found literal whitespace with stashed idx!")
                        }

                        let incremental_length =
                            tfs.templated_slice.end - element.template_slice.start;
                        result.push(element.to_segment(
                            PositionMarker::new(
                                element.template_slice.start + consumed_element_length + tfs_offset
                                    ..tfs.templated_slice.end + tfs_offset,
                                element.template_slice.clone(),
                                templated_file.clone(),
                                None,
                                None,
                            ),
                            offset_slice(consumed_element_length, incremental_length).into(),
                        ));
                    } else {
                        // We can't split it. We're going to end up yielding a segment
                        // which spans multiple slices. Stash the type, and if we haven't
                        // set the start yet, stash it too.
                        // lexer_logger.debug("     Spilling over literal slice.")
                        if stashed_source_idx.is_none() {
                            stashed_source_idx = (element.template_slice.start + tfs_idx).into();
                            // lexer_logger.debug(
                            //     "     Stashing a source start. %s", stashed_source_idx
                            // )
                            continue;
                        }
                    }
                }
            } else if matches!(tfs.slice_type.as_str(), "templated" | "block_start") {
                unimplemented!();
            }
        }
    }

    result
}

fn _handle_zero_length_slice() {
    // impl me
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::segments::base::{
        CodeSegment, CodeSegmentNewArgs, NewlineSegment, NewlineSegmentNewArgs,
    };

    /// Assert that a matcher does or doesn't work on a string.
    ///
    /// The optional `matchstring` argument, which can optionally
    /// be None, allows to either test positive matching of a
    /// particular string or negative matching (that it explicitly)
    /// doesn't match.
    fn assert_matches(in_string: &str, matcher: &impl Matcher, match_string: Option<&str>) {
        let res = matcher.match_(in_string.to_string()).unwrap();
        if let Some(match_string) = match_string {
            assert_eq!(res.forward_string, in_string[match_string.len()..]);
            assert_eq!(res.elements.len(), 1);
            assert_eq!(res.elements[0].raw, match_string);
        } else {
            assert_eq!(res.forward_string, in_string);
            assert_eq!(res.elements.len(), 0);
        }
    }

    /// Test a RegexLexer with a trim_post_subdivide function.
    #[test]
    // TODO Implement Test
    fn test__parser__lexer_trim_post_subdivide() {
        let matcher: Vec<Box<dyn Matcher>> = vec![Box::new(
            RegexLexer::new(
                "function_script_terminator",
                r";\s+(?!\*)\/(?!\*)|\s+(?!\*)\/(?!\*)",
                &CodeSegment::create,
                CodeSegmentNewArgs {
                    code_type: "function_script_terminator",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                Some(Box::new(StringLexer::new(
                    "semicolon",
                    ";",
                    &CodeSegment::create,
                    CodeSegmentNewArgs {
                        code_type: "semicolon",
                        instance_types: vec![],
                        trim_start: None,
                        trim_chars: None,
                        source_fixes: None,
                    },
                    None,
                    None,
                ))),
                Some(Box::new(
                    RegexLexer::new(
                        "newline",
                        r"(\n|\r\n)+",
                        &NewlineSegment::create,
                        NewlineSegmentNewArgs {},
                        None,
                        None,
                    )
                    .unwrap(),
                )),
            )
            .unwrap(),
        )];

        let res = Lexer::lex_match(";\n/\n", &matcher).unwrap();
        assert_eq!(res.elements[0].raw, ";");
        assert_eq!(res.elements[1].raw, "\n");
        assert_eq!(res.elements[2].raw, "/");
        assert_eq!(res.elements.len(), 3);
    }

    /// Test the RegexLexer.
    #[test]
    fn test__parser__lexer_regex() {
        let tests = &[
            ("fsaljk", "f", "f"),
            ("fsaljk", r"f", "f"),
            ("fsaljk", r"[fas]*", "fsa"),
            // Matching whitespace segments
            ("   \t   fsaljk", r"[^\S\r\n]*", "   \t   "),
            // Matching whitespace segments (with a newline)
            ("   \t \n  fsaljk", r"[^\S\r\n]*", "   \t "),
            // Matching quotes containing stuff
            ("'something boring'   \t \n  fsaljk", r"'[^']*'", "'something boring'"),
            (
                "' something exciting \t\n '   \t \n  fsaljk",
                r"'[^']*'",
                "' something exciting \t\n '",
            ),
        ];

        for (raw, reg, res) in tests {
            let matcher = RegexLexer::new(
                "test",
                reg,
                &CodeSegment::create,
                CodeSegmentNewArgs {
                    code_type: "",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap();

            assert_matches(raw, &matcher, Some(res));
        }
    }

    /// Test the lexer string
    #[test]
    fn test__parser__lexer_string() {
        let matcher = StringLexer::new(
            "dot",
            ".",
            &CodeSegment::create,
            CodeSegmentNewArgs {
                code_type: "dot",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        );
        assert_matches(".fsaljk", &matcher, Some("."));
        assert_matches("fsaljk", &matcher, None);
    }

    /// Test the RepeatedMultiMatcher
    #[test]
    fn test__parser__lexer_lex_match() {
        let matchers: Vec<Box<dyn Matcher>> = vec![
            Box::new(StringLexer::new(
                "dot",
                ".",
                &CodeSegment::create,
                CodeSegmentNewArgs {
                    code_type: "",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )),
            Box::new(
                RegexLexer::new(
                    "test",
                    r"#[^#]*#",
                    &CodeSegment::create,
                    CodeSegmentNewArgs {
                        code_type: "",
                        instance_types: vec![],
                        trim_start: None,
                        trim_chars: None,
                        source_fixes: None,
                    },
                    None,
                    None,
                )
                .unwrap(),
            ),
        ];

        let res = Lexer::lex_match("..#..#..#", &matchers).unwrap();

        assert_eq!(res.forward_string, "#");
        assert_eq!(res.elements.len(), 5);
        assert_eq!(res.elements[2].raw, "#..#");
    }
}
