use crate::core::config::FluffConfig;
use crate::core::errors::ValueError;

/// An element matched during lexing.
#[derive(Debug, Clone)]
pub struct LexedElement {
    raw: String,
}

impl LexedElement {
    pub fn matcher() -> &'static str {
        "StringLexer"
    }
}

/// A LexedElement, bundled with it's position in the templated file.
pub struct TemplateElement {
    raw: String,
    // TODO Figure out how to do this
    // template_slice: slice
}

impl TemplateElement {
    pub fn matcher() -> &'static str {
        "StringLexer"
    }
}

/// A class to hold matches from the lexer.
#[derive(Debug, Clone)]
pub struct LexMatch {
    forward_string: String,
    elements: Vec<LexedElement>,
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
