use std::sync::Arc;

use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::TemplatedFile;

use crate::Formatter;
use crate::core::config::FluffConfig;
use crate::templaters::{ProcessingMode, Templater};

#[derive(Default)]
pub struct RawTemplater;

impl RawTemplater {
    fn process_single(
        &self,
        in_str: &str,
        f_name: &str,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        TemplatedFile::new(in_str.to_string(), f_name.to_string(), None, None, None)
            .map_err(|e| SQLFluffUserError::new(format!("Raw templater error: {e}")))
    }
}

impl Templater for RawTemplater {
    fn name(&self) -> &'static str {
        "raw"
    }

    fn description(&self) -> &'static str {
        r"The raw templater simply returns the input string as the output string. It passes through the input string unchanged and is useful if you need no templating. It is the default templater."
    }

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::Parallel
    }

    fn process(
        &self,
        files: &[(&str, &str)],
        _config: &FluffConfig,
        _formatter: &Option<Arc<dyn Formatter>>,
    ) -> Vec<Result<TemplatedFile, SQLFluffUserError>> {
        files
            .iter()
            .map(|(content, fname)| self.process_single(content, fname))
            .collect()
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

        let results = templater.process(
            &[(in_str, "test.sql")],
            &FluffConfig::from_source("", None),
            &None,
        );

        assert_eq!(results.len(), 1);
        let outstr = results.into_iter().next().unwrap().unwrap();
        assert_eq!(outstr.templated_str, Some(in_str.to_string()));
    }
}
