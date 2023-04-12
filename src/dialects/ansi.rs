use crate::core::dialects::base::Dialect;
use crate::core::parser::lexer::Matcher;

#[derive(Debug)]
pub struct AnsiDialect;

impl Dialect for AnsiDialect {
    fn get_lexer_matchers(&self) -> Vec<Box<dyn Matcher>> {
        panic!("get_lexer_matchers not implemented yet");
    }
}
