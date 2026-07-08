use sqruff_lib_core::templaters::TemplatedFile;

use crate::api::SourceId;
use crate::core::config::FluffConfig;
use crate::templaters::{
    ProcessingMode, Templater, TemplaterError, TemplaterInput, TemplaterOutput, source_id_name,
};

#[derive(Default)]
pub struct RawTemplater;

impl RawTemplater {
    fn process_single(
        &self,
        in_str: &str,
        source_id: &SourceId,
    ) -> Result<TemplatedFile, TemplaterError> {
        let f_name = source_id_name(source_id);
        TemplatedFile::new(in_str.to_string(), f_name.to_string(), None, None, None).map_err(|e| {
            TemplaterError::Failed(sqruff_lib_core::errors::SQLFluffUserError::new(format!(
                "Raw templater error: {e}"
            )))
        })
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
        files: &[TemplaterInput<'_>],
        _config: &FluffConfig,
    ) -> Vec<Result<TemplaterOutput, TemplaterError>> {
        files
            .iter()
            .map(|file| {
                self.process_single(file.source, file.source_id)
                    .map(TemplaterOutput::Rendered)
            })
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

        let source_id = SourceId::Virtual("test.sql".into());
        let results = templater.process(
            &[TemplaterInput {
                source: in_str,
                source_id: &source_id,
            }],
            &FluffConfig::from_source("", None),
        );

        assert_eq!(results.len(), 1);
        let outstr = match results.into_iter().next().unwrap().unwrap() {
            TemplaterOutput::Rendered(file) => file,
            TemplaterOutput::Skipped(_) => panic!("raw templater should render"),
        };
        assert_eq!(outstr.templated_str, Some(in_str.to_string()));
    }
}
