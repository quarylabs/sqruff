use crate::core::config::FluffConfig;

pub trait Formatter{
    fn dispatch_template_header(&self, f_name: String, linter_config: FluffConfig, file_config: FluffConfig);
}