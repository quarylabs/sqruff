use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::core::errors::SQLBaseError;
use crate::core::linter::common::ParsedString;
use crate::core::linter::linted_file::LintedFile;
use crate::core::linter::linting_result::LintingResult;

use super::linted_dir::LintedDir;

pub struct Linter {
    config: FluffConfig,
    formatter: Option<Box<dyn Formatter>>,
}

impl Linter {
    pub fn new(config: FluffConfig, formatter: Option<Box<dyn Formatter>>) -> Linter {
        Linter { config, formatter }
    }

    /// Lint strings directly.
    pub fn lint_string_wrapped(
        &self,
        sql: String,
        f_name: Option<String>,
        fix: Option<bool>,
    ) -> LintingResult {
        // TODO Translate LintingResult
        let result = LintingResult::new();
        let f_name_default = f_name
            .clone()
            .unwrap_or_else(|| "<string input>".to_string());
        let mut linted_path = LintedDir::new(f_name_default.clone());
        linted_path.add(self.lint_string(Some(sql), Some(f_name_default), fix, None, None));
        panic!("Not finished");
        // linted_path = LintedDir(f_name);
        // result.add(linted_path)
        // result.stop_timer()
        // return result
    }

    /// Parse a string.
    pub fn parse_string(
        &self,
        in_str: String,
        fname: Option<String>,
        recurse: Option<bool>,
        config: Option<&FluffConfig>,
        encoding: Option<String>,
    ) -> ParsedString {
        let defaulted_f_name = fname.unwrap_or("<string>".to_string());
        let defaulted_recurse = recurse.unwrap_or(true);
        let defaulted_encoding = encoding.unwrap_or("utf-8".to_string());

        let violations: Vec<SQLBaseError> = vec![];
        // Dispatch the output for the template header (including the config diff)
        match &self.formatter {
            Some(formatter) => {
                if let unwrapped_config = config.unwrap() {
                    formatter.dispatch_template_header(
                        defaulted_f_name,
                        self.config.clone(),
                        unwrapped_config.clone(),
                    )
                } else {
                    panic!("config cannot be Option in this case")
                }
            }
            _ => {}
        }

        // Just use the local config from here
        let mut binding = self.config.clone();
        panic!("not implemented");
        // let mut config = *config.clone().unwrap_or(binding);
        // Scan the raw file for config commands.
        // config.process_raw_file_for_config(&in_str);
        // rendered = self.render_string(in_str, fname, config, encoding)
        // violations += rendered.templater_violations
        // config.process_raw_file_for_config(in_str)
    }
    // ) -> ParsedString:
    // """Parse a string."""
    // violations: List[SQLBaseError] = []
    //
    // # Dispatch the output for the template header (including the config diff)
    // if self.formatter:
    // self.formatter.dispatch_template_header(fname, self.config, config)
    //
    // # Just use the local config from here:
    // config = config or self.config
    //
    // # Scan the raw file for config commands.
    // config.process_raw_file_for_config(in_str)
    // rendered = self.render_string(in_str, fname, config, encoding)
    // violations += rendered.templater_violations
    //
    // # Dispatch the output for the parse header
    // if self.formatter:
    // self.formatter.dispatch_parse_header(fname)
    //
    // return self.parse_rendered(rendered, recurse=recurse)

    /// Lint a string.
    pub fn lint_string(
        &self,
        in_str: Option<String>,
        f_name: Option<String>,
        fix: Option<bool>,
        config: Option<&FluffConfig>,
        encoding: Option<String>,
    ) -> LintedFile {
        // Sort out config, defaulting to the built in config if no override
        let defaulted_config = config.unwrap_or(&self.config);
        // Parse the string.
        let parsed = self.parse_string(
            in_str.unwrap_or("".to_string()),
            f_name,
            None,
            Some(defaulted_config),
            None,
        );
        panic!("Not implemented")
        // # Get rules as appropriate
        // rule_pack = self.get_rulepack(config=config)
        // # Lint the file and return the LintedFile
        // return self.lint_parsed(
        //     parsed,
        //     rule_pack,
        //     fix=fix,
        //     formatter=self.formatter,
        //     encoding=encoding,
        // )
    }
    // ) -> LintedFile:
    //     # Sort out config, defaulting to the built in config if no override
    //     config = config or self.config
    //     # Parse the string.
    //     parsed = self.parse_string(
    //         in_str=in_str,
    //         fname=fname,
    //         config=config,
    //     )
    //     # Get rules as appropriate
    //     rule_set = self.get_ruleset(config=config)
    //     # Lint the file and return the LintedFile
    //     return self.lint_parsed(
    //         parsed,
    //         rule_set,
    //         fix=fix,
    //         formatter=self.formatter,
    //         encoding=encoding,
    //     )
}
