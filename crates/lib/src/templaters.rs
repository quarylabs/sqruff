use crate::core::templaters::base::Templater;
use crate::templaters::placeholder::PlaceholderTemplater;
use crate::templaters::raw::RawTemplater;

pub mod placeholder;
pub mod raw;

// templaters returns all the templaters that are available in the library
pub fn templaters() -> Vec<Box<dyn Templater>> {
    vec![Box::new(RawTemplater), Box::new(PlaceholderTemplater)]
}
