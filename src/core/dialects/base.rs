use crate::core::parser::lexer::Matcher;
use std::fmt::Debug;

pub struct Base {}

pub trait Dialect: Debug {
    /// Fetch the lexer struct for this dialect.
    fn get_lexer_matchers(&self) -> Vec<Box<dyn Matcher>>;
}
