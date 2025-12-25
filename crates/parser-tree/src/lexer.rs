use std::borrow::Cow;
use std::ops::Range;

use crate::dialects::Dialect;
use crate::dialects::SyntaxKind;
use crate::errors::SQLLexError;
use crate::slice_helpers::{is_zero_slice, offset_slice};
use crate::templaters::TemplatedFile;
use sqruff_parser_core::parser::token::{Token, TokenSpan};
pub use sqruff_parser_core::parser::lexer::{
    Cursor, Element, Match, Matcher, Pattern, SearchPatternKind,
};

/// A LexedElement, bundled with it's position in the templated file.
#[derive(Debug)]
pub struct TemplateElement<'a> {
    raw: Cow<'a, str>,
    template_slice: Range<usize>,
    matcher: Info,
}

#[derive(Debug)]
struct Info {
    name: &'static str,
    syntax_kind: SyntaxKind,
}

impl<'a> TemplateElement<'a> {
    /// Make a TemplateElement from a LexedElement.
    pub fn from_element(element: Element<'a>, template_slice: Range<usize>) -> Self {
        let name = element.name();
        let syntax_kind = element.syntax_kind();
        TemplateElement {
            raw: element.into_text(),
            template_slice,
            matcher: Info { name, syntax_kind },
        }
    }

    pub fn to_token(&self, span: TokenSpan, subslice: Option<Range<usize>>) -> Token {
        let slice = subslice.map_or_else(|| self.raw.as_ref(), |slice| &self.raw[slice]);
        Token::new(self.matcher.syntax_kind, slice, span)
    }
}

/// The Lexer class actually does the lexing step.
#[derive(Debug, Clone)]
pub struct Lexer {
    syntax_map: Vec<(&'static str, SyntaxKind)>,
    regex: regex_automata::meta::Regex,
    matchers: Vec<Matcher>,
    last_resort_lexer: Matcher,
}

impl<'a> From<&'a Dialect> for Lexer {
    fn from(dialect: &'a Dialect) -> Self {
        Lexer::new(dialect.lexer_matchers())
    }
}

impl Lexer {
    /// Create a new lexer.
    pub(crate) fn new(lexer_matchers: &[Matcher]) -> Self {
        let mut patterns = Vec::new();
        let mut syntax_map = Vec::new();
        let mut matchers = Vec::new();

        for matcher in lexer_matchers {
            let pattern_def = matcher.pattern();
            match pattern_def.kind() {
                SearchPatternKind::String(raw) | SearchPatternKind::Regex(raw) => {
                    let raw = *raw;
                    let pattern_str = if matches!(pattern_def.kind(), SearchPatternKind::String(_))
                    {
                        fancy_regex::escape(raw)
                    } else {
                        raw.into()
                    };

                    patterns.push(pattern_str);
                    syntax_map.push((pattern_def.name(), pattern_def.syntax_kind()));
                }
                SearchPatternKind::Legacy(_, _) | SearchPatternKind::Native(_) => {
                    matchers.push(matcher.clone());
                }
            }
        }

        Lexer {
            syntax_map,
            matchers,
            regex: regex_automata::meta::Regex::new_many(&patterns).unwrap(),
            last_resort_lexer: Matcher::legacy(
                "<unlexable>",
                |_| true,
                r"[^\t\n.]*",
                SyntaxKind::Unlexable,
            ),
        }
    }

    pub fn lex(&self, template: impl Into<TemplatedFile>) -> (Vec<Token>, Vec<SQLLexError>) {
        let template = template.into();
        let mut str_buff = template.templated_str.as_deref().unwrap();

        // Lex the string to get a tuple of LexedElement
        let mut element_buffer: Vec<Element> = Vec::new();

        loop {
            let mut res = self.lex_match(str_buff);
            element_buffer.append(&mut res.elements);

            if res.forward_string.is_empty() {
                break;
            }

            // If we STILL can't match, then just panic out.
            let mut resort_res = self.last_resort_lexer.matches(str_buff);
            if !resort_res.elements.is_empty() {
                break;
            }

            str_buff = resort_res.forward_string;
            element_buffer.append(&mut resort_res.elements);
        }

        // Map tuple LexedElement to list of TemplateElement.
        // This adds the template_slice to the object.
        let templated_buffer = Lexer::map_template_slices(element_buffer, &template);
        // Turn lexed elements into tokens.
        let tokens = self.elements_to_tokens(templated_buffer, &template);

        (tokens, Vec::new())
    }

    pub fn lex_tokens(&self, template: impl Into<TemplatedFile>) -> (Vec<Token>, Vec<SQLLexError>) {
        self.lex(template)
    }

    /// Generate any lexing errors for any un-lex-ables.
    ///
    /// TODO: Taking in an iterator, also can make the typing better than use
    /// unwrap.
    #[allow(dead_code)]
    /// Iteratively match strings using the selection of sub-matchers.
    fn lex_match<'b>(&self, mut forward_string: &'b str) -> Match<'b> {
        let mut elem_buff = Vec::new();

        'main: loop {
            if forward_string.is_empty() {
                return Match {
                    forward_string,
                    elements: elem_buff,
                };
            }

            for matcher in &self.matchers {
                let mut match_result = matcher.matches(forward_string);

                if !match_result.elements.is_empty() {
                    elem_buff.append(&mut match_result.elements);
                    forward_string = match_result.forward_string;
                    continue 'main;
                }
            }

            let input =
                regex_automata::Input::new(forward_string).anchored(regex_automata::Anchored::Yes);

            if let Some(match_) = self.regex.find(input) {
                let (name, kind) = self.syntax_map[match_.pattern().as_usize()];

                elem_buff.push(Element::new(
                    name,
                    kind,
                    &forward_string[match_.start()..match_.end()],
                ));
                forward_string = &forward_string[match_.end()..];

                continue 'main;
            }

            return Match {
                forward_string,
                elements: elem_buff,
            };
        }
    }

    /// Create a tuple of TemplateElement from a tuple of LexedElement.
    ///
    /// This adds slices in the templated file to the original lexed
    /// elements. We'll need this to work out the position in the source
    /// file.
    /// TODO Can this vec be turned into an iterator and return iterator to make
    /// lazy?
    fn map_template_slices<'b>(
        elements: Vec<Element<'b>>,
        template: &TemplatedFile,
    ) -> Vec<TemplateElement<'b>> {
        let mut idx = 0;
        let mut templated_buff: Vec<TemplateElement> = Vec::with_capacity(elements.len());

        for element in elements {
            let text = element.text();
            let template_slice = offset_slice(idx, text.len());
            idx += text.len();

            let templated_string = template.templated();
            if &templated_string[template_slice.clone()] != text {
                panic!(
                    "Template and lexed elements do not match. This should never happen {:?} != \
                     {:?}",
                    text, &templated_string[template_slice]
                );
            }

            templated_buff.push(TemplateElement::from_element(element, template_slice));
        }

        templated_buff
    }

    /// Convert a tuple of lexed elements into a tuple of tokens.
    fn elements_to_tokens(
        &self,
        elements: Vec<TemplateElement>,
        templated_file: &TemplatedFile,
    ) -> Vec<Token> {
        let mut tokens = iter_tokens(elements, templated_file);

        // Add an end of file marker
        let eof_span = match tokens.last() {
            Some(token) => TokenSpan::new(
                token.span.source_end,
                token.span.source_end,
                token.span.templated_end,
                token.span.templated_end,
            ),
            None => TokenSpan::new(0, 0, 0, 0),
        };

        tokens.push(Token::new(SyntaxKind::EndOfFile, "", eof_span));

        tokens
    }
}

fn token_span_for_element(
    element: &TemplateElement,
    source_start: usize,
    source_end: usize,
) -> TokenSpan {
    TokenSpan::new(
        source_start,
        source_end,
        element.template_slice.start,
        element.template_slice.end,
    )
}

fn iter_tokens(lexed_elements: Vec<TemplateElement>, templated_file: &TemplatedFile) -> Vec<Token> {
    let mut result: Vec<Token> = Vec::with_capacity(lexed_elements.len());
    // An index to track where we've got to in the templated file.
    let mut tfs_idx = 0;
    // We keep a map of previous block locations in case they re-occur.
    // let block_stack = BlockTracker()
    let templated_file_slices = &templated_file.sliced_file;

    // Now work out source slices, and add in template placeholders.
    for element in lexed_elements {
        let consumed_element_length = 0;
        let mut stashed_source_idx = None;

        for (idx, tfs) in templated_file_slices
            .iter()
            .skip(tfs_idx)
            .enumerate()
            .map(|(i, tfs)| (i + tfs_idx, tfs))
        {
            // Is it a zero slice?
            if is_zero_slice(&tfs.templated_slice) {
                let _slice = if idx + 1 < templated_file_slices.len() {
                    templated_file_slices[idx + 1].clone().into()
                } else {
                    None
                };

                continue;
            }

            if tfs.slice_type == "literal" {
                let tfs_offset =
                    (tfs.source_slice.start as isize) - (tfs.templated_slice.start as isize);

                // NOTE: Greater than OR EQUAL, to include the case of it matching
                // length exactly.
                if element.template_slice.end <= tfs.templated_slice.end {
                    let slice_start = stashed_source_idx.unwrap_or_else(|| {
                        let sum = element.template_slice.start as isize
                            + consumed_element_length as isize
                            + tfs_offset;
                        if sum < 0 {
                            panic!("Slice start is negative: {sum}");
                        }
                        sum.try_into()
                            .unwrap_or_else(|_| panic!("Cannot convert {sum} to usize"))
                    });

                    let source_slice_end =
                        (element.template_slice.end as isize + tfs_offset) as usize;
                    let span = token_span_for_element(&element, slice_start, source_slice_end);
                    result.push(
                        element.to_token(span, Some(consumed_element_length..element.raw.len())),
                    );

                    // If it was an exact match, consume the templated element too.
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
                    log::debug!("Missed Skip");
                    continue;
                } else {
                    // This means that the current lexed element spans across
                    // multiple templated file slices.

                    log::debug!("Consuming whole spanning literal",);

                    // This almost certainly means there's a templated element
                    // in the middle of a whole lexed element.

                    // What we do here depends on whether we're allowed to split
                    // lexed elements. This is basically only true if it's whitespace.
                    // NOTE: We should probably make this configurable on the
                    // matcher object, but for now we're going to look for the
                    // name of the lexer.
                    if element.matcher.name == "whitespace" {
                        if stashed_source_idx.is_some() {
                            panic!("Found literal whitespace with stashed idx!")
                        }

                        let incremental_length =
                            tfs.templated_slice.end - element.template_slice.start;

                        let source_slice_start = element.template_slice.start as isize
                            + consumed_element_length as isize
                            + tfs_offset;
                        let source_slice_start =
                            source_slice_start.try_into().unwrap_or_else(|_| {
                                panic!("Cannot convert {source_slice_start} to usize")
                            });
                        let source_slice_end =
                            source_slice_start as isize + incremental_length as isize;
                        let source_slice_end = source_slice_end.try_into().unwrap_or_else(|_| {
                            panic!("Cannot convert {source_slice_end} to usize")
                        });

                        let span =
                            token_span_for_element(&element, source_slice_start, source_slice_end);
                        result.push(element.to_token(
                            span,
                            offset_slice(consumed_element_length, incremental_length).into(),
                        ));
                    } else {
                        // We can't split it. We're going to end up yielding a segment
                        // which spans multiple slices. Stash the type, and if we haven't
                        // set the start yet, stash it too.
                        log::debug!("Spilling over literal slice.");
                        if stashed_source_idx.is_none() {
                            stashed_source_idx = (element.template_slice.start + idx).into();
                            log::debug!("Stashing a source start. {stashed_source_idx:?}");
                            continue;
                        }
                    }
                }
            } else if matches!(tfs.slice_type.as_str(), "templated" | "block_start") {
                // Found a templated slice. Does it have length in the templated file?
                // If it doesn't, then we'll pick it up next.
                if !is_zero_slice(&tfs.templated_slice) {
                    // If it's a block_start. Append to the block stack.
                    // NOTE: This is rare, but call blocks do occasionally
                    // have length (and so don't get picked up by
                    // _handle_zero_length_slice)
                    if tfs.slice_type == "block_start" {
                        unimplemented!()
                        // block_stack.enter(tfs.source_slice)
                    }

                    // Is our current element totally contained in this slice?
                    if element.template_slice.end <= tfs.templated_slice.end {
                        log::debug!("Contained templated slice.");
                        // Yes it is. Add lexed element with source slices as the whole
                        // span of the source slice for the file slice.
                        // If we've got an existing stashed source start, use that
                        // as the start of the source slice.
                        let slice_start = if let Some(stashed_source_idx) = stashed_source_idx {
                            stashed_source_idx
                        } else {
                            tfs.source_slice.start + consumed_element_length
                        };

                        let span =
                            token_span_for_element(&element, slice_start, tfs.source_slice.end);
                        result.push(
                            element
                                .to_token(span, Some(consumed_element_length..element.raw.len())),
                        );

                        // If it was an exact match, consume the templated element too.
                        if element.template_slice.end == tfs.templated_slice.end {
                            tfs_idx += 1
                        }
                        // Carry on to the next lexed element
                        break;
                    } else {
                        // We've got an element which extends beyond this templated slice.
                        // This means that a _single_ lexed element claims both some
                        // templated elements and some non-templated elements. That could
                        // include all kinds of things (and from here we don't know what
                        // else is yet to come, comments, blocks, literals etc...).

                        // In the `literal` version of this code we would consider
                        // splitting the literal element here, but in the templated
                        // side we don't. That's because the way that templated tokens
                        // are lexed, means that they should arrive "pre-split".

                        // Stash the source idx for later when we do make a segment.
                        if stashed_source_idx.is_none() {
                            stashed_source_idx = Some(tfs.source_slice.start);
                            continue;
                        }
                        // Move on to the next template slice
                        continue;
                    }
                }
            }
            panic!("Unable to process slice: {tfs:?}");
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert that a matcher does or doesn't work on a string.
    ///
    /// The optional `matchstring` argument, which can optionally
    /// be None, allows to either test positive matching of a
    /// particular string or negative matching (that it explicitly)
    /// doesn't match.
    fn assert_matches(in_string: &str, matcher: &Matcher, match_string: Option<&str>) {
        let res = matcher.matches(in_string);
        if let Some(match_string) = match_string {
            assert_eq!(res.forward_string, &in_string[match_string.len()..]);
            assert_eq!(res.elements.len(), 1);
            assert_eq!(res.elements[0].text, match_string);
        } else {
            assert_eq!(res.forward_string, in_string);
            assert_eq!(res.elements.len(), 0);
        }
    }

    #[test]
    fn test_parser_lexer_trim_post_subdivide() {
        let matcher: Vec<Matcher> = vec![
            Matcher::legacy(
                "function_script_terminator",
                |_| true,
                r";\s+(?!\*)\/(?!\*)|\s+(?!\*)\/(?!\*)",
                SyntaxKind::StatementTerminator,
            )
            .subdivider(Pattern::string("semicolon", ";", SyntaxKind::Semicolon))
            .post_subdivide(Pattern::legacy(
                "newline",
                |_| true,
                r"(\n|\r\n)+",
                SyntaxKind::Newline,
            )),
        ];

        let res = Lexer::new(&matcher).lex_match(";\n/\n");
        assert_eq!(res.elements[0].text, ";");
        assert_eq!(res.elements[1].text, "\n");
        assert_eq!(res.elements[2].text, "/");
        assert_eq!(res.elements.len(), 3);
    }

    /// Test the RegexLexer.
    #[test]
    fn test_parser_lexer_regex() {
        let tests = &[
            ("fsaljk", "f", "f"),
            ("fsaljk", r"f", "f"),
            ("fsaljk", r"[fas]*", "fsa"),
            // Matching whitespace segments
            ("   \t   fsaljk", r"[^\S\r\n]*", "   \t   "),
            // Matching whitespace segments (with a newline)
            ("   \t \n  fsaljk", r"[^\S\r\n]*", "   \t "),
            // Matching quotes containing stuff
            (
                "'something boring'   \t \n  fsaljk",
                r"'[^']*'",
                "'something boring'",
            ),
            (
                "' something exciting \t\n '   \t \n  fsaljk",
                r"'[^']*'",
                "' something exciting \t\n '",
            ),
        ];

        for (raw, reg, res) in tests {
            let matcher = Matcher::legacy("test", |_| true, reg, SyntaxKind::Word);

            assert_matches(raw, &matcher, Some(res));
        }
    }

    /// Test the lexer string
    #[test]
    fn test_parser_lexer_string() {
        let matcher = Matcher::string("dot", ".", SyntaxKind::Dot);

        assert_matches(".fsaljk", &matcher, Some("."));
        assert_matches("fsaljk", &matcher, None);
    }

    /// Test the RepeatedMultiMatcher
    #[test]
    fn test_parser_lexer_lex_match() {
        let matchers: Vec<Matcher> = vec![
            Matcher::string("dot", ".", SyntaxKind::Dot),
            Matcher::regex("test", "#[^#]*#", SyntaxKind::Dash),
        ];

        let res = Lexer::new(&matchers).lex_match("..#..#..#");

        assert_eq!(res.forward_string, "#");
        assert_eq!(res.elements.len(), 5);
        assert_eq!(res.elements[2].text, "#..#");
    }
}
