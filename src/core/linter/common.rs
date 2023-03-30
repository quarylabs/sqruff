use crate::core::config::FluffConfig;
use crate::core::errors::SQLBaseError;
use crate::core::parser::segments::base::BaseSegment;
use crate::core::templaters::base::TemplatedFile;

/// An object to store the result of parsing a string.
pub struct ParsedString {
    tree: Option<BaseSegment>,
    violations: Vec<SQLBaseError>,
    // TODO Implement time dict
    /// `time_dict` is a :obj:`dict` containing timings for how long each step took in the process.
    // time_dict: dict
    /// `templated_file` is a :obj:`TemplatedFile` containing the details of the templated file.
    templated_file: TemplatedFile,
    config: FluffConfig,
    f_name: String,
    source_str: String,
}
