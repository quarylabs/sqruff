use crate::core::parser::lexer::Matcher;
use std::fmt::Debug;
use std::sync::Arc;

pub struct Base {}

pub trait Dialect: Debug {
    /// Fetch the lexer struct for this dialect.
    fn get_lexer_matchers(&self) -> Vec<Arc<dyn Matcher>>;
}
