use sqruff_lib_core::errors::{SQLBaseError, SQLTemplaterError};
use sqruff_lib_core::parser::segments::base::ErasedSegment;
use sqruff_lib_core::templaters::base::TemplatedFile;

/// An object to store the result of a templated file/string.
///
/// This is notable as it's the intermediate state between what happens
/// in the main process and the child processes when running in parallel mode.
#[derive(Debug, Clone)]
pub struct RenderedFile {
    pub templated_file: TemplatedFile,
    pub templater_violations: Vec<SQLTemplaterError>,
    pub(crate) filename: String,
    pub source_str: String,
}

/// An object to store the result of parsing a string.
#[derive(Debug, Clone)]
pub struct ParsedString {
    pub tree: Option<ErasedSegment>,
    pub violations: Vec<SQLBaseError>,
    pub templated_file: TemplatedFile,
    pub filename: String,
    pub source_str: String,
}
