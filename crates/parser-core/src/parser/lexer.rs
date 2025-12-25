use std::borrow::Cow;
use std::ops::Range;
use std::str::Chars;

use crate::dialects::syntax::SyntaxKind;

/// An element matched during lexing.
#[derive(Debug, Clone)]
pub struct Element<'a> {
    name: &'static str,
    text: Cow<'a, str>,
    syntax_kind: SyntaxKind,
}

impl<'a> Element<'a> {
    pub fn new(name: &'static str, syntax_kind: SyntaxKind, text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            name,
            syntax_kind,
            text: text.into(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn text(&self) -> &str {
        self.text.as_ref()
    }

    pub fn into_text(self) -> Cow<'a, str> {
        self.text
    }

    pub fn syntax_kind(&self) -> SyntaxKind {
        self.syntax_kind
    }
}

/// A class to hold matches from the lexer.
#[derive(Debug)]
pub struct Match<'a> {
    pub forward_string: &'a str,
    pub elements: Vec<Element<'a>>,
}

#[derive(Debug, Clone)]
pub struct Matcher {
    pattern: Pattern,
    subdivider: Option<Pattern>,
    trim_post_subdivide: Option<Pattern>,
}

impl Matcher {
    pub const fn new(pattern: Pattern) -> Self {
        Self {
            pattern,
            subdivider: None,
            trim_post_subdivide: None,
        }
    }

    pub const fn string(
        name: &'static str,
        pattern: &'static str,
        syntax_kind: SyntaxKind,
    ) -> Self {
        Self::new(Pattern::string(name, pattern, syntax_kind))
    }

    #[track_caller]
    pub fn regex(name: &'static str, pattern: &'static str, syntax_kind: SyntaxKind) -> Self {
        Self::new(Pattern::regex(name, pattern, syntax_kind))
    }

    pub fn native(name: &'static str, f: fn(&mut Cursor) -> bool, syntax_kind: SyntaxKind) -> Self {
        Self::new(Pattern::native(name, f, syntax_kind))
    }

    #[track_caller]
    pub fn legacy(
        name: &'static str,
        starts_with: fn(&str) -> bool,
        pattern: &'static str,
        syntax_kind: SyntaxKind,
    ) -> Self {
        Self::new(Pattern::legacy(name, starts_with, pattern, syntax_kind))
    }

    pub fn subdivider(mut self, subdivider: Pattern) -> Self {
        assert!(matches!(
            self.pattern.kind,
            SearchPatternKind::Legacy(_, _) | SearchPatternKind::Native(_)
        ));
        self.subdivider = Some(subdivider);
        self
    }

    pub fn post_subdivide(mut self, trim_post_subdivide: Pattern) -> Self {
        assert!(matches!(
            self.pattern.kind,
            SearchPatternKind::Legacy(_, _) | SearchPatternKind::Native(_)
        ));
        self.trim_post_subdivide = Some(trim_post_subdivide);
        self
    }

    pub fn name(&self) -> &'static str {
        self.pattern.name
    }

    pub fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    #[track_caller]
    pub fn matches<'a>(&self, forward_string: &'a str) -> Match<'a> {
        match self.pattern.matches(forward_string) {
            Some(matched) => {
                let new_elements = self.subdivide(matched, self.pattern.syntax_kind);

                Match {
                    forward_string: &forward_string[matched.len()..],
                    elements: new_elements,
                }
            }
            None => Match {
                forward_string,
                elements: Vec::new(),
            },
        }
    }

    fn subdivide<'a>(&self, matched: &'a str, matched_kind: SyntaxKind) -> Vec<Element<'a>> {
        match &self.subdivider {
            Some(subdivider) => {
                let mut elem_buff = Vec::new();
                let mut str_buff = matched;

                while !str_buff.is_empty() {
                    let Some(div_pos) = subdivider.search(str_buff) else {
                        let mut trimmed_elems = self.trim_match(str_buff);
                        elem_buff.append(&mut trimmed_elems);
                        break;
                    };

                    let mut trimmed_elems = self.trim_match(&str_buff[..div_pos.start]);
                    let div_elem = Element::new(
                        subdivider.name,
                        subdivider.syntax_kind,
                        &str_buff[div_pos.start..div_pos.end],
                    );

                    elem_buff.append(&mut trimmed_elems);
                    elem_buff.push(div_elem);

                    str_buff = &str_buff[div_pos.end..];
                }

                elem_buff
            }
            None => vec![Element::new(self.name(), matched_kind, matched)],
        }
    }

    fn trim_match<'a>(&self, matched_str: &'a str) -> Vec<Element<'a>> {
        let Some(trim_post_subdivide) = &self.trim_post_subdivide else {
            return Vec::new();
        };

        let mk_element = |text| {
            Element::new(
                trim_post_subdivide.name,
                trim_post_subdivide.syntax_kind,
                text,
            )
        };

        let mut elem_buff = Vec::new();
        let mut content_buff = String::new();
        let mut str_buff = matched_str;

        while !str_buff.is_empty() {
            let Some(trim_pos) = trim_post_subdivide.search(str_buff) else {
                break;
            };

            let start = trim_pos.start;
            let end = trim_pos.end;

            if start == 0 {
                elem_buff.push(mk_element(&str_buff[..end]));
                str_buff = str_buff[end..].into();
            } else if end == str_buff.len() {
                let raw = format!("{}{}", content_buff, &str_buff[..start]);

                elem_buff.push(Element::new(
                    trim_post_subdivide.name,
                    trim_post_subdivide.syntax_kind,
                    raw,
                ));
                elem_buff.push(mk_element(&str_buff[start..end]));

                content_buff.clear();
                str_buff = "";
            } else {
                content_buff.push_str(&str_buff[..end]);
                str_buff = &str_buff[end..];
            }
        }

        if !content_buff.is_empty() || !str_buff.is_empty() {
            let raw = format!("{content_buff}{str_buff}");
            elem_buff.push(Element::new(
                self.pattern.name,
                self.pattern.syntax_kind,
                raw,
            ));
        }

        elem_buff
    }
}

#[derive(Debug, Clone)]
pub struct Pattern {
    name: &'static str,
    syntax_kind: SyntaxKind,
    kind: SearchPatternKind,
}

#[derive(Debug, Clone)]
pub enum SearchPatternKind {
    String(&'static str),
    Regex(&'static str),
    Native(fn(&mut Cursor) -> bool),
    Legacy(fn(&str) -> bool, fancy_regex::Regex),
}

impl Pattern {
    pub const fn string(
        name: &'static str,
        template: &'static str,
        syntax_kind: SyntaxKind,
    ) -> Self {
        Self {
            name,
            syntax_kind,
            kind: SearchPatternKind::String(template),
        }
    }

    #[track_caller]
    pub fn regex(name: &'static str, regex: &'static str, syntax_kind: SyntaxKind) -> Self {
        #[cfg(debug_assertions)]
        if regex_automata::dfa::regex::Regex::new(regex).is_err() {
            panic!("Invalid regex pattern: {}", std::panic::Location::caller());
        }

        Self {
            name,
            syntax_kind,
            kind: SearchPatternKind::Regex(regex),
        }
    }

    pub fn native(name: &'static str, f: fn(&mut Cursor) -> bool, syntax_kind: SyntaxKind) -> Self {
        Self {
            name,
            syntax_kind,
            kind: SearchPatternKind::Native(f),
        }
    }

    pub fn legacy(
        name: &'static str,
        starts_with: fn(&str) -> bool,
        regex: &'static str,
        syntax_kind: SyntaxKind,
    ) -> Self {
        let regex = format!("^{regex}");
        Self {
            name,
            syntax_kind,
            kind: SearchPatternKind::Legacy(starts_with, fancy_regex::Regex::new(&regex).unwrap()),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn syntax_kind(&self) -> SyntaxKind {
        self.syntax_kind
    }

    pub fn kind(&self) -> &SearchPatternKind {
        &self.kind
    }

    fn matches<'a>(&self, forward_string: &'a str) -> Option<&'a str> {
        match self.kind {
            SearchPatternKind::String(template) => {
                if forward_string.starts_with(template) {
                    return Some(template);
                }
            }
            SearchPatternKind::Legacy(f, ref template) => {
                if !f(forward_string) {
                    return None;
                }

                if let Ok(Some(matched)) = template.find(forward_string)
                    && matched.start() == 0
                {
                    return Some(matched.as_str());
                }
            }
            SearchPatternKind::Native(f) => {
                let mut cursor = Cursor::new(forward_string);
                return f(&mut cursor).then(|| cursor.lexed());
            }
            _ => unreachable!(),
        };

        None
    }

    fn search(&self, forward_string: &str) -> Option<Range<usize>> {
        match &self.kind {
            SearchPatternKind::String(template) => forward_string
                .find(template)
                .map(|start| start..start + template.len()),
            SearchPatternKind::Legacy(_, template) => {
                if let Ok(Some(matched)) = template.find(forward_string) {
                    return Some(matched.range());
                }
                None
            }
            _ => unreachable!("{:?}", self.kind),
        }
    }
}

pub struct Cursor<'text> {
    text: &'text str,
    chars: Chars<'text>,
}

impl<'text> Cursor<'text> {
    const EOF: char = '\0';

    pub fn new(text: &'text str) -> Self {
        Self {
            text,
            chars: text.chars(),
        }
    }

    pub fn peek(&self) -> char {
        self.chars.clone().next().unwrap_or(Self::EOF)
    }

    pub fn shift(&mut self) -> char {
        self.chars.next().unwrap_or(Self::EOF)
    }

    pub fn shift_while(&mut self, f: impl Fn(char) -> bool + Copy) {
        while self.peek() != Self::EOF && f(self.peek()) {
            self.shift();
        }
    }

    fn lexed(&self) -> &'text str {
        let len = self.text.len() - self.chars.as_str().len();
        &self.text[..len]
    }
}
