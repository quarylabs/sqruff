use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::errors::{SQLLexError, ValueError};
use crate::core::parser::segments::base::{unlexable_segment_constructor, Segment, SegmentConstructor, SegmentConstructorFunc, UnlexableSegmentNewArgs, UnlexableSegment};
use crate::core::templaters::base::TemplatedFile;
use regex::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

/// An element matched during lexing.
#[derive(Debug)]
pub struct LexedElement {
    raw: String,
    matcher: Arc<dyn Matcher>,
}

/// A LexedElement, bundled with it's position in the templated file.
pub struct TemplateElement {
    raw: String,
    template_slice: Range<usize>,
    matcher: Arc<dyn Matcher>,
}

impl TemplateElement {
    /// Make a TemplateElement from a LexedElement.
    pub fn from_element(element: &LexedElement, template_slice: Range<usize>) -> Self {
        TemplateElement {
            raw: element.raw.clone(),
            template_slice,
            matcher: element.matcher.clone(),
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

pub trait Matcher: Debug {
    /// The name of the matcher.
    fn get_name(self: &Self) -> String;
    /// Given a string, match what we can and return the rest.
    fn match_(self: &Self, forward_string: String) -> Result<LexMatch, ValueError>;
    /// Use regex to find a substring.
    fn search(self: &Self, forward_string: &str) -> Option<Range<usize>>;
}

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
#[derive(Debug, Clone)]
pub struct StringLexer {
    template: String,
}

impl StringLexer {
    /// The private match function. Just look for a literal string.
    pub fn _match(self: &Self, forward_string: &str) -> Option<LexedElement> {
        if forward_string.starts_with(&self.template) {
            Some(LexedElement {
                raw: self.template.clone(),
                matcher: Arc::new(self.clone()),
            })
        } else {
            None
        }
    }

    /// Given a string, trim if we are allowed to.
    pub fn _trim_match(self: &Self, matched_string: String) -> Vec<LexedElement> {
        panic!("Not implemented")
    }

    /// Given a string, subdivide if we area allowed to.
    pub fn _subdivide(self: &Self, matched: LexedElement) -> Vec<LexedElement> {
        panic!("Not implemented")
    }
}

impl Display for StringLexer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "StringLexer({})", self.template)
    }
}

impl Matcher for StringLexer {
    fn get_name(self: &Self) -> String {
        self.template.clone()
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
pub struct RegexLexer<S: Segment + Debug, Args: Debug + Clone> {
    name: String,
    template: regex::Regex,
    segment_constructor: Arc<SegmentConstructorFunc<S, Args>>,
    segment_constructor_args: Args,
}

pub fn new_regex_lexer<S: Segment, Args: Debug + Clone>(
    name: String,
    regex: String,
    segment_constructor: Arc<SegmentConstructorFunc<S, Args>>,
    segment_constructor_args: Args,
) -> Result<RegexLexer<S, Args>, Error> {
    Ok(RegexLexer {
        name: name.to_string(),
        template: regex::Regex::new(regex.as_str())?,
        segment_constructor,
        segment_constructor_args,
    })
}

impl<S: Segment, Args: Debug + Clone> RegexLexer<S, Args> {
    fn regex_lexer_match(self: &Self, forward_string: &str) -> Option<LexedElement> {
        if let Some(matched) = self.template.find(forward_string) {
            if matched.as_str().len() != 0 {
                panic!("RegexLexer matched a non-zero start: {}", matched.start());
            }
            Some({
                let new_lexer = new_regex_lexer(
                    self.name.to_string(),
                    self.template.as_str().to_string(),
                    self.segment_constructor.clone(),
                    self.segment_constructor_args.clone(),
                ).unwrap();
                LexedElement {
                    raw: matched.as_str().to_string(),
                    matcher: Arc::new(new_lexer),
                }
            })
        } else {
            None
        }
    }

    fn regex_lexer_subdivide(self: &Self, matched: LexedElement, ) -> Vec<LexedElement> {
        panic!("Not implemented")
    }

}

impl<S: Segment, Args: Debug + Clone> Display for RegexLexer<S, Args> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegexLexer({})", self.get_name())
    }
}

impl<S: Segment, Args: Debug + Clone> Matcher for RegexLexer<S, Args> {
    fn get_name(self: &Self) -> String {
        self.template.as_str().to_string()
    }

    /// Given a string, match what we can and return the rest.
    fn match_(self: &Self, forward_string: String) -> Result<LexMatch, ValueError> {
        if forward_string.len() == 0 {
            return Err(ValueError::new(String::from("Unexpected empty string!")));
        };
        let matched = self.regex_lexer_match( &forward_string);
        match matched {
            Some(matched) => {
                let length = matched.raw.len();
                let new_elements = self.regex_lexer_subdivide( matched);
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
        if let Some(matched) = self.template.find(forward_string) {
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

impl<S: Segment, Args: Debug + Clone> Debug for RegexLexer<S, Args> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        panic!("implement")
    }
}

/// The Lexer class actually does the lexing step.
pub struct Lexer {
    config: FluffConfig,
    last_resort_lexer: Arc<dyn Matcher>,
}

pub enum StringOrTemplate {
    String(String),
    Template(TemplatedFile),
}

impl Lexer {
    /// Create a new lexer.
    pub fn new(config: FluffConfig, dialect: Option<Box<dyn Dialect>>) -> Self {
        let fluff_config = FluffConfig::from_kwargs(Some(config), dialect, None);
        let last_resort_lexer = new_regex_lexer::<UnlexableSegment, UnlexableSegmentNewArgs>(
            "last_resort".to_string(),
            "[^\t\n.]*".to_string(),
            Arc::new(unlexable_segment_constructor),
            UnlexableSegmentNewArgs {},
        )
            .expect("Unable to create last resort lexer");
        Lexer {
            config: fluff_config,
            last_resort_lexer: Arc::new(last_resort_lexer),
        }
    }

    pub fn lex(
        &self,
        raw: StringOrTemplate,
    ) -> Result<(Rc<dyn Segment>, Vec<SQLLexError>), ValueError> {
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
        lexer_matchers: Vec<Arc<dyn Matcher>>,
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
                    for element in res.elements {
                        elem_buff.push(LexedElement {
                            raw: element.raw.clone(),
                            matcher: element.matcher,
                        });
                    }

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
            let lexed_element = LexedElement {
                raw: element.raw.clone(),
                matcher: element.matcher,
            };
            templated_buff.push(TemplateElement::from_element(
                &lexed_element,
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
