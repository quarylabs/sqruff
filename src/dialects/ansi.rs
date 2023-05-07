use crate::core::dialects::base::Dialect;
use crate::core::parser::lexer::{Matcher, RegexLexer};
use crate::core::parser::segments::base::{CommentSegment, CommentSegmentNewArgs, SegmentConstructorFn, WhitespaceSegment, WhitespaceSegmentNewArgs};

#[derive(Debug)]
pub struct AnsiDialect;

impl Dialect for AnsiDialect {
    fn get_lexer_matchers(&self) -> Vec<Box<dyn Matcher>> {
        lexer_matchers()
    }
}

fn lexer_matchers() -> Vec<Box<dyn Matcher>> {
    vec![
        // Match all forms of whitespace except newlines and carriage returns:
        // https://stackoverflow.com/questions/3469080/match-whitespace-but-not-newlines
        // This pattern allows us to also match non-breaking spaces (#2189).
        Box::new(
            RegexLexer::new(
                "whitespace",
                r"[^\S\r\n]+",
                &WhitespaceSegment::new,
                WhitespaceSegmentNewArgs {},
            )
                .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "inline_comment",
                r"(--|#)[^\n]*",
                &CommentSegment::new,
                CommentSegmentNewArgs {
                    trim_start: vec!["--", "#"],
                    comment_type: "inline_comment",
                },
            )
                .unwrap()
        ),
    ]
}
