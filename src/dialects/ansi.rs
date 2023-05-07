use crate::core::dialects::base::Dialect;
use crate::core::parser::lexer::{Matcher, RegexLexer};
use crate::core::parser::segments::base::{
    SegmentConstructorFn, WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use std::sync::Arc;

#[derive(Debug)]
pub struct AnsiDialect;

impl Dialect for AnsiDialect {
    fn get_lexer_matchers(&self) -> Vec<Arc<dyn Matcher>> {
        lexer_matchers()
    }
}

fn lexer_matchers() -> Vec<Arc<dyn Matcher>> {
    vec![
        // Match all forms of whitespace except newlines and carriage returns:
        // https://stackoverflow.com/questions/3469080/match-whitespace-but-not-newlines
        // This pattern allows us to also match non-breaking spaces (#2189).
        Arc::new(
            RegexLexer::new(
                "whitespace",
                r"[^\S\r\n]+",
                &WhitespaceSegment::new as SegmentConstructorFn<WhitespaceSegmentNewArgs>,
                WhitespaceSegmentNewArgs {},
            )
            .unwrap(),
        ),
    ]
}
