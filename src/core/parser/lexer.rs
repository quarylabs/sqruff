// TODO Can the string lexers be pointers and is that better?

use crate::core::config::FluffConfig;
use crate::core::errors::ValueError;
use std::ops::Range;
use crate::core::parser::markers::PositionMarker;

/// An element matched during lexing.
#[derive(Debug, Clone)]
pub struct LexedElement {
    raw: String,
    matcher: StringLexer
}

/// A LexedElement, bundled with it's position in the templated file.
pub struct TemplateElement {
    raw: String,
    template_slice: Range<usize>,
    matcher: StringLexer
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

    /// Create a segment from this lexed element.
    pub fn to_segment(&self, pos_marker: PositionMarker, subslice: Option<Slice>) -> Segment {
        let raw = match subslice {
            Some(slice) => &self.raw[slice],
            None => &self.raw,
        };
        self.matcher.construct_segment(raw, pos_marker)
    }
}

/// A class to hold matches from the lexer.
#[derive(Debug, Clone)]
pub struct LexMatch {
    forward_string: String,
    elements: Vec<LexedElement>,
}

impl LexMatch {
    /// new creates a LexMatch.
    pub fn new(forward_string: String, elements: Vec<LexedElement>) -> Self {
        LexMatch {
            forward_string,
            elements,
        }
    }

    /// A LexMatch is truthy if it contains a non-zero number of matched elements.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

/// This singleton matcher matches strings exactly.
/// This is the simplest usable matcher, but it also defines some of the
/// mechanisms for more complicated matchers, which may simply override the
/// `_match` function rather than the public `match` function.  This acts as
/// the base class for matchers.
#[derive(Debug, Clone)]
pub struct StringLexer {
    name: String,
    template: String,
}

impl StringLexer {
    /// The private match function. Just look for a literal string.
    pub fn _match(self: &Self, forward_string: &String) -> Option<LexedElement> {
        if forward_string.starts_with(&self.template) {
            Some(LexedElement {
                raw: self.template.clone(),
            })
        } else {
            None
        }
    }

    /// Use string methods to find a substring.
    pub fn search(self: &Self, forward_string: String) -> Option<(usize, usize)> {
        let start = forward_string.find(&self.template);
        if start.is_some() {
            Some((start.unwrap(), start.unwrap() + self.template.len()))
        } else {
            None
        }
    }

    /// Given a string, trim if we are allowed to.
    pub fn _trim_match(self: &Self, matched_string: String) -> Vec<LexedElement> {
        panic!("Not implemented")
    }

    /// Given a string, match what we can and return the rest.
    pub fn match_(self: &Self, forward_string: String) -> Result<LexMatch, ValueError> {
        if forward_string.len() == 0 {
            return Err(ValueError::new(String::from("Unexpected empty string!")));
        };
        let matched = self._match(&forward_string);
        match matched {
            Some(matched) => {
                let new_elements = self._subdivide(matched.clone());
                Ok(LexMatch {
                    forward_string: forward_string[matched.raw.len()..].to_string(),
                    elements: new_elements,
                })
            }
            None => Ok(LexMatch {
                forward_string: forward_string.to_string(),
                elements: vec![],
            }),
        }
    }

    /// Given a string, subdivide if we area allowed to.
    pub fn _subdivide(self: &Self, matched: LexedElement) -> Vec<LexedElement> {
        panic!("Not implemented")
    }
}

/// The Lexer class actually does the lexing step.
pub struct Lexer {
    config: FluffConfig,
    lexer_matchers: Vec<StringLexer>,
    last_resort_lexer: StringLexer,
}

impl Lexer {
    fn new(
        config: Option<FluffConfig>,
        last_resort_lexer: Option<StringLexer>,
        dialect: Option<String>,
    ) -> Self {
        let config = FluffConfig::from_kwargs(config, dialect);
        let lexer_matchers = config.get("dialect_obj").get_lexer_matchers();
        let last_resort_lexer = last_resort_lexer
            .unwrap_or_else(|| RegexLexer::new("<unlexable>", r"[^\t\n\ ]*", UnlexableSegment));

        Lexer {
            config,
            lexer_matchers,
            last_resort_lexer,
        }
    }
}
