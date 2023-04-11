use crate::core::errors::ValueError;
use crate::core::parser::markers::PositionMarker;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Range;
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

/// A class to hold matches from the lexer.
#[derive(Debug)]
pub struct LexMatch {
    forward_string: String,
    elements: Vec<LexedElement>,
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

impl LexMatch {
    /// A LexMatch is truthy if it contains a non-zero number of matched elements.
    pub fn __bool__(self: &Self) -> bool {
        self.elements.len() > 0
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

    fn search(self: &Self, forward_string: String) -> Option<(usize, usize)> {
        let start = forward_string.find(&self.template);
        if start.is_some() {
            Some((start.unwrap(), start.unwrap() + self.template.len()))
        } else {
            None
        }
    }
}

pub trait Matcher: Debug {
    /// The name of the matcher.
    fn get_name(self: &Self) -> String;
    /// Given a string, match what we can and return the rest.
    fn match_(self: &Self, forward_string: String) -> Result<LexMatch, ValueError>;
    /// Use regex to find a substring.
    fn search(self: &Self, forward_string: String) -> Option<(usize, usize)>;
}

impl Display for dyn Matcher {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Matcher({})", self.get_name())
    }
}
