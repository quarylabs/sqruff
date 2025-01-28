use std::sync::Arc;

use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::TemplatedFile;

use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::templaters::Templater;

#[derive(Default)]
pub struct RawTemplater;

impl Templater for RawTemplater {
    fn name(&self) -> &'static str {
        "raw"
    }

    fn description(&self) -> &'static str {
        r"The raw templater simply returns the input string as the output string. It passes through the input string unchanged and is useful if you need no templating. It is the defualt templater."
    }

    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        _config: &FluffConfig,
        _formatter: &Option<Arc<dyn Formatter>>,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        if let Ok(tf) = TemplatedFile::new(in_str.to_string(), f_name.to_string(), None, None, None)
        {
            return Ok(tf);
        }
        panic!("Not implemented")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    /// Test the raw templater
    fn test_templater_raw() {
        let templater = RawTemplater;
        let in_str = "SELECT * FROM {{blah}}";

        let outstr = templater
            .process(
                in_str,
                "test.sql",
                &FluffConfig::from_source("", None),
                &None,
            )
            .unwrap();

        assert_eq!(outstr.templated_str, Some(in_str.to_string()));
    }
}
