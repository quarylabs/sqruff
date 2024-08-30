use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;
use crate::core::errors::SQLFluffUserError;
use crate::core::templaters::base::{TemplatedFile, Templater};

#[derive(Default)]
pub struct RawTemplater {}

impl Templater for RawTemplater {
    fn name(&self) -> &str {
        "raw"
    }

    fn template_selection(&self) -> &str {
        "templater"
    }

    fn config_pairs(&self) -> (String, String) {
        ("templater".to_string(), self.name().to_string())
    }

    fn sequence_files(
        &self,
        f_names: Vec<String>,
        _: Option<&FluffConfig>,
        _: Option<&OutputStreamFormatter>,
    ) -> Vec<String> {
        // Default is to process in the original order.
        f_names
    }

    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        _config: Option<&FluffConfig>,
        _formatter: Option<&OutputStreamFormatter>,
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
        let templater = RawTemplater::default();
        let in_str = "SELECT * FROM {{blah}}";

        let outstr = templater.process(in_str, "test.sql", None, None).unwrap();

        assert_eq!(outstr.templated_str, Some(in_str.to_string()));
    }
}
