use crate::core::parser::lexer::Matcher;
use std::fmt::Debug;

pub struct Base {}

pub trait Dialect: Debug + dyn_clone::DynClone {
    /// Fetch the lexer struct for this dialect.
    fn get_lexer_matchers(&self) -> Vec<Box<dyn Matcher>>;

    fn root_segment_name(&self) -> &str {
        todo!()
    }

    // Return an object which acts as a late binding reference to the element named.
    // NB: This requires the dialect to be expanded, and only returns Matchables
    // as a result.
    fn r#ref(&self, _name: &str) {
        todo!()
    }

    /// Get the root segment of the dialect
    fn get_root_segment(&self) {
        self.r#ref(self.root_segment_name())
    }
}

dyn_clone::clone_trait_object!(Dialect);
