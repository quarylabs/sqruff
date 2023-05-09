use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::errors::{SQLLexError, ValueError};
use crate::core::parser::segments::base::{
    Segment, SegmentConstructorFn, UnlexableSegment, UnlexableSegmentNewArgs,
};
use crate::core::templaters::base::TemplatedFile;
use dyn_clone::DynClone;
use fancy_regex::{Error, Regex};
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, Range};
use std::sync::Arc;

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
pub struct TemplateElement {
    raw: String,
    template_slice: Range<usize>,
    matcher: Box<dyn Matcher>,
}

impl TemplateElement {
    /// Make a TemplateElement from a LexedElement.
    pub fn from_element(element: LexedElement, template_slice: Range<usize>) -> Self {
        TemplateElement {
            raw: element.raw,
            template_slice,
            matcher: element.matcher,
        }
    }
}

/// A class to hold matches from the lexer.
#[derive(Debug)]
pub struct LexMatch {
    forward_string: String,
    pub elements: Vec<LexedElement>,
}

impl LexMatch {
    /// A LexMatch is truthy if it contains a non-zero number of matched elements.
    pub fn is_non_empty(self: &Self) -> bool {
        self.elements.len() > 0
    }
}

pub trait Matcher: Debug + DynClone {
    /// The name of the matcher.
    fn get_name(self: &Self) -> String;
    /// Given a string, match what we can and return the rest.
    fn match_(self: &Self, forward_string: String) -> Result<LexMatch, ValueError>;
    /// Use regex to find a substring.
    fn search(self: &Self, forward_string: &str) -> Option<Range<usize>>;
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
    fn _match(self: &Self, forward_string: &str) -> Option<LexedElement> {
        if forward_string.starts_with(&self.template) {
            Some(LexedElement {
                raw: self.template.to_string(),
                matcher: Box::new(self.clone()),
            })
        } else {
            None
        }
    }

    /// Given a string, trim if we are allowed to.
    fn _trim_match(self: &Self, matched_string: String) -> Vec<LexedElement> {
        panic!("Not implemented")
    }

    /// Given a string, subdivide if we area allowed to.
    fn _subdivide(self: &Self, matched: LexedElement) -> Vec<LexedElement> {
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
    fn get_name(self: &Self) -> String {
        self.template.to_string()
    }

    /// Given a string, match what we can and return the rest.
    fn match_(self: &Self, forward_string: String) -> Result<LexMatch, ValueError> {
        if forward_string.len() == 0 {
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
            None => Ok(LexMatch {
                forward_string: forward_string.to_string(),
                elements: vec![],
            }),
        }
    }

    fn search(self: &Self, forward_string: &str) -> Option<Range<usize>> {
        let start = forward_string.find(&self.template);
        if start.is_some() {
            Some(start.unwrap()..start.unwrap() + self.template.len())
        } else {
            None
        }
    }
}

/// This RegexLexer matches based on regular expressions.
#[derive(Clone)]
pub struct RegexLexer<SegmentArgs: 'static + Clone> {
    name: &'static str,
    template: Regex,
    segment_constructor: SegmentConstructorFn<SegmentArgs>,
    segment_args: SegmentArgs,
    sub_divider: Option<Arc<dyn Matcher>>,
    trim_post_subdivide: Option<Arc<dyn Matcher>>,
}

impl<SegmentArgs: Clone + Debug> RegexLexer<SegmentArgs> {
    pub fn new(
        name: &'static str,
        regex: &str,
        segment_constructor: SegmentConstructorFn<SegmentArgs>,
        segment_args: SegmentArgs,
        sub_divider: Option<Arc<dyn Matcher>>,
        trim_post_subdivide: Option<Arc<dyn Matcher>>,
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
    pub fn _match(self: &Self, forward_string: &str) -> Option<LexedElement> {
        if let Ok(Some(matched)) = self.template.find(forward_string) {
            if matched.as_str().len() != 0 {
                panic!("RegexLexer matched a non-zero start: {}", matched.start());
            }
            Some(LexedElement {
                raw: matched.as_str().to_string(),
                matcher: Box::new(self.clone()),
            })
        } else {
            None
        }
    }

    // TODO: Could be inherited from StringLexer.
    pub fn _subdivide(self: &Self, matched: LexedElement) -> Vec<LexedElement> {
        panic!("Not implemented")
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
    fn get_name(self: &Self) -> String {
        self.template.as_str().to_string()
    }

    /// Given a string, match what we can and return the rest.
    fn match_(self: &Self, forward_string: String) -> Result<LexMatch, ValueError> {
        if forward_string.len() == 0 {
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
            None => Ok(LexMatch {
                forward_string: forward_string.to_string(),
                elements: vec![],
            }),
        }
    }

    /// Use regex to find a substring.
    fn search(self: &Self, forward_string: &str) -> Option<Range<usize>> {
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
}

/// The Lexer class actually does the lexing step.
pub struct Lexer {
    config: FluffConfig,
    last_resort_lexer: Box<dyn Matcher>,
}

pub enum StringOrTemplate {
    String(String),
    Template(TemplatedFile),
}

impl Lexer {
    /// Create a new lexer.
    pub fn new(config: FluffConfig, dialect: Option<Box<dyn Dialect>>) -> Self {
        let fluff_config = FluffConfig::from_kwargs(Some(config), dialect, None);
        let last_resort_lexer = RegexLexer::new(
            "last_resort",
            "[^\t\n.]*",
            &UnlexableSegment::new,
            UnlexableSegmentNewArgs {},
            None,
            None,
        )
        .expect("Unable to create last resort lexer");
        Lexer {
            config: fluff_config,
            last_resort_lexer: Box::new(last_resort_lexer),
        }
    }

    pub fn lex(
        &self,
        raw: StringOrTemplate,
    ) -> Result<(Box<dyn Segment>, Vec<SQLLexError>), ValueError> {
        // Make sure we've got a string buffer and a template regardless of what was passed in.
        let (mut str_buff, template) = match raw {
            StringOrTemplate::String(s) => (s.clone(), TemplatedFile::from_string(s.to_string())),
            StringOrTemplate::Template(f) => (f.to_string(), f),
        };

        // Lex the string to get a tuple of LexedElement
        let mut element_buffer: Vec<LexedElement> = Vec::new();
        loop {
            let res = Lexer::lex_match(&str_buff, self.config.get_dialect().get_lexer_matchers())
                .unwrap();
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
        let templated_buffer = Lexer::map_template_slices(element_buffer, template);

        // while True:
        //     res = self.lex_match(str_buff, self.lexer_matchers)
        // element_buffer += res.elements
        // if res.forward_string:
        //     resort_res = self.last_resort_lexer.match(res.forward_string)
        // if not resort_res:  # pragma: no cover
        // # If we STILL can't match, then just panic out.
        //     raise SQLLexError(
        //     "Fatal. Unable to lex characters: {0!r}".format(
        //         res.forward_string[:10] + "..."
        //         if len(res.forward_string) > 9
        //         else res.forward_string
        //     )
        // )
        // str_buff = resort_res.forward_string
        // element_buffer += resort_res.elements
        // else:  # pragma: no cover TODO?
        // break

        panic!("Not implemented");
    }

    /// Generate any lexing errors for any un-lex-ables.
    ///
    /// TODO: Taking in an iterator, also can make the typing better than use unwrap.
    fn violations_from_segments<T: Debug + Clone>(segments: Vec<impl Segment>) -> Vec<SQLLexError> {
        segments
            .into_iter()
            .filter(|s| s.is_type("unlexable"))
            .map(|s| {
                SQLLexError::new(
                    format!(
                        "Unable to lex characters: {}",
                        s.get_raw().unwrap().chars().take(10).collect::<String>()
                    ),
                    s.get_pos_maker().unwrap(),
                )
            })
            .collect()
    }

    /// Iteratively match strings using the selection of sub-matchers.
    fn lex_match(
        forward_string: &str,
        lexer_matchers: Vec<Box<dyn Matcher>>,
    ) -> Result<LexMatch, ValueError> {
        let mut forward_str = forward_string.to_string();
        let mut elem_buff: Vec<LexedElement> = vec![];
        loop {
            if forward_string.len() == 0 {
                return Ok(LexMatch {
                    forward_string: forward_string.to_string(),
                    elements: elem_buff,
                });
            };
            for matcher in &lexer_matchers {
                let res = matcher.match_(forward_string.to_string())?;
                if res.elements.len() > 0 {
                    // If we have new segments then whoop!
                    elem_buff.append(res.elements.clone().as_mut());
                    forward_str = res.forward_string;
                    // Cycle back around again and start with the top
                    // matcher again.
                    break;
                } else {
                    // We've got so far, but now can't match. Return
                    return Ok(LexMatch {
                        forward_string: forward_string.to_string(),
                        elements: elem_buff,
                    });
                }
            }
        }
    }

    /// Create a tuple of TemplateElement from a tuple of LexedElement.
    ///
    /// This adds slices in the templated file to the original lexed
    /// elements. We'll need this to work out the position in the source
    /// file.
    /// TODO Can this vec be turned into an iterator and return iterator to make lazy?
    fn map_template_slices(
        elements: Vec<LexedElement>,
        template: TemplatedFile,
    ) -> Vec<TemplateElement> {
        let mut idx = 0;
        let mut templated_buff: Vec<TemplateElement> = vec![];
        for element in elements {
            let template_slice = idx..idx + element.raw.len();
            idx += element.raw.len();
            templated_buff.push(TemplateElement::from_element(
                element.clone(),
                template_slice,
            ));
            let templated_string = template.get_templated_string().unwrap();
            if templated_string != element.raw {
                panic!(
                    "Template and lexed elements do not match. This should never happen {} != {}",
                    element.raw, templated_string
                );
            }
        }
        return templated_buff;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::segments::base::{
        CodeSegment, CodeSegmentNewArgs, NewLineSegmentNewArgs, NewlineSegment,
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
    fn test__parser__lexer_trim_post_subdivide() {
        let matcher: Vec<Box<dyn Matcher>> = vec![Box::new(
            RegexLexer::new(
                "function_script_terminator",
                r";\s+(?!\*)\/(?!\*)|\s+(?!\*)\/(?!\*)",
                &CodeSegment::new,
                CodeSegmentNewArgs {
                    code_type: "function_script_terminator",
                },
                Some(Arc::new(StringLexer::new(
                    "semicolon",
                    ";",
                    &CodeSegment::new,
                    CodeSegmentNewArgs {
                        code_type: "semicolon",
                    },
                    None,
                    None,
                ))),
                Some(Arc::new(
                    RegexLexer::new(
                        "newline",
                        r"(\n|\r\n)+",
                        &NewlineSegment::new,
                        NewLineSegmentNewArgs {},
                        None,
                        None,
                    )
                    .unwrap(),
                )),
            )
            .unwrap(),
        )];

        let res = Lexer::lex_match(";\n/\n", matcher).unwrap();
        assert_eq!(res.elements[0].raw, ";");
        assert_eq!(res.elements[1].raw, "\n");
        assert_eq!(res.elements[2].raw, "/");
        assert_eq!(res.elements.len(), 3);
    }

    /// Test the lexer string
    #[test]
    fn test__parser__lexer_string() {
        let matcher = StringLexer::new(
            "dot",
            ".",
            &CodeSegment::new,
            CodeSegmentNewArgs { code_type: "dot" },
            None,
            None,
        );
        assert_matches(".fsaljk", &matcher, Some("."));
        assert_matches("fsaljk", &matcher, None);
    }
}
