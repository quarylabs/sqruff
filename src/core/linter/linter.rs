use std::time::Instant;
use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::core::errors::SQLBaseError;
use crate::core::linter::common::{ParsedString, RenderedFile};
use crate::core::linter::linted_file::LintedFile;
use crate::core::linter::linting_result::LintingResult;
use crate::core::parser::segments::base::BaseSegment;

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

        let mut violations: Vec<SQLBaseError> = vec![];
        // Dispatch the output for the template header (including the config diff)
        if let Some(formatter) = &self.formatter {
            if let unwrapped_config = config.unwrap() {
                formatter.dispatch_template_header(
                    defaulted_f_name.clone(),
                    self.config.clone(),
                    unwrapped_config.clone(),
                )
            } else {
                panic!("config cannot be Option in this case")
            }
        }

        // Just use the local config from here
        let binding = self.config.clone();
        let mut config = config.unwrap_or(&binding).clone();
        // Scan the raw file for config commands.
        config.process_raw_file_for_config(&in_str);
        let rendered = self.render_string(
            in_str,
            Some(defaulted_f_name),
            config,
            Some(defaulted_encoding),
        );

        violations.append(rendered.templater_violations.clone().as_mut());

        // Dispatch the output for the parse header
        if let Some(formatter) = &self.formatter {
            formatter.dispatch_parse_header(defaulted_f_name.clone());
        }
        return self.parse_rendered(rendered, recurse);
    }

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

    /// Template the file.
    pub fn render_string(
        &self,
        in_str: String,
        f_name: Option<String>,
        config: FluffConfig,
        encoding: Option<String>,
    ) -> RenderedFile {
        panic!("Not implemented");
    }

    /// Parse a rendered file.
    pub fn parse_rendered(
        rendered: RenderedFile,
        recurse: bool,
    ) -> ParsedString {
        let t0 = Instant::now();
        let mut violations = rendered.templater_violations.clone();
        let tokens: Option<Vec<BaseSegment>>;
        if let Some(templated_file) = rendered.templated_file {
            let (t, lvs, config) = Self::_lex_templated_file(templated_file, &rendered.config);
            tokens = Some(t);
            violations.extend(lvs);
        } else {
            tokens = None;
        }

        // TODO Add the timing and linting
        // let t1 = Instant::now();
        // let linter_logger = log::logger();
        // linter_logger.info("PARSING ({})", rendered.fname);

        let parsed: Option<Box<BaseSegment>>;
        if let Some(token_list) = tokens {
            let (p, pvs) = Self::_parse_tokens(
                token_list,
                &rendered.config,
                recurse,
                &rendered.fname,
            );
            parsed = p;
            violations.extend(pvs);
        } else {
            parsed = None;
        }
        panic!("Not implemented");
        //
        // let mut time_dict = rendered.time_dict.clone();
        // time_dict.insert("lexing".to_string(), (t1 - t0).as_secs_f64());
        // time_dict.insert("parsing".to_string(), (Instant::now() - t1).as_secs_ff64());
        // ParsedString {
        // tree: parsed,
        // violations,
        // time_dict,
        // templated_file: rendered.templated_file,
        // config: rendered.config,
        // fname: rendered.fname,
        // source_str: rendered.source_str,
        // }
    }
}
