use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::core::errors::{SQLFluffUserError, SQLLexError, SQLParseError, SqlError};
use crate::core::linter::common::{ParsedString, RenderedFile};
use crate::core::linter::linted_file::LintedFile;
use crate::core::linter::linting_result::LintingResult;
use crate::core::parser::segments::base::BaseSegment;
use crate::core::templaters::base::{RawTemplater, TemplatedFile, Templater};
use regex::Regex;
use std::collections::HashMap;
use std::time::Instant;

use super::linted_dir::LintedDir;

pub struct Linter {
    config: FluffConfig,
    formatter: Option<Box<dyn Formatter>>,
    templater: Box<dyn Templater>,
}

impl Linter {
    pub fn new(
        config: FluffConfig,
        formatter: Option<Box<dyn Formatter>>,
        templater: Option<Box<dyn Templater>>,
    ) -> Linter {
        if let Some(t) = templater {
            return Linter {
                config,
                formatter,
                templater: t,
            };
        } else {
            return Linter {
                config,
                formatter,
                templater: Box::new(RawTemplater::default()),
            };
        }
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
    ) -> Result<ParsedString, SQLFluffUserError> {
        let defaulted_f_name = fname.unwrap_or("<string>".to_string());
        let defaulted_recurse = recurse.unwrap_or(true);
        let defaulted_encoding = encoding.unwrap_or("utf-8".to_string());

        let mut violations: Vec<Box<dyn SqlError>> = vec![];
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
            defaulted_f_name.clone(),
            config,
            Some(defaulted_encoding),
        )?;

        for violation in &rendered.templater_violations {
            violations.push(Box::new(violation.clone()));
        }

        // Dispatch the output for the parse header
        if let Some(formatter) = &self.formatter {
            formatter.dispatch_parse_header(defaulted_f_name.clone());
        }
        return Ok(Self::parse_rendered(rendered, defaulted_recurse));
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
        f_name: String,
        config: FluffConfig,
        encoding: Option<String>,
    ) -> Result<RenderedFile, SQLFluffUserError> {
        // TODO Implement loggers eventually
        // let linter_logger = log::logger();
        // linter_logger.info!("TEMPLATING RAW [{}] ({})", self.templater.name, f_name);

        // Start the templating timer
        let t0 = Instant::now();

        // Newlines are normalised to unix-style line endings (\n).
        // The motivation is that Jinja normalises newlines during templating and
        // we want consistent mapping between the raw and templated slices.
        let in_str = Self::normalise_newlines(in_str.as_str());

        // Since Linter.__init__() does not require a dialect to be specified,
        // check for one now. (We're processing a string, not a file, so we're
        // not going to pick up a .sqlfluff or other config file to provide a
        // missing dialect at this point.)
        if let Some(error) = config.verify_dialect_specified() {
            return Err(error);
        }

        // TODO Implement linter warning
        // if config.get("templater_obj") != self.templater {
        //     linter_logger::warning(format!(
        //         "Attempt to set templater to {} failed. Using {} templater. Templater cannot be set in a .sqlfluff file in a subdirectory of the current working directory. It can be set in a .sqlfluff in the current working directory. See Nesting section of the docs for more details.",
        //         config.get("templater_obj").name,
        //         self.templater.name,
        //     ));
        // }

        let mut templated_file = None;
        let mut templater_violations = vec![];
        match self.templater.process(
            in_str.as_str(),
            f_name.as_str(),
            Some(&config),
            self.formatter.as_deref(),
        ) {
            Ok(file) => {
                templated_file = Some(file);
            }
            Err(s) => {
                // TODO Implement linter warning
                panic!("not implemented")
                // linter_logger::warning(s.to_string());
            }
        }

        if templated_file.is_none() {
            panic!("not implemented");
            // linter_logger::info(
            //     "TEMPLATING FAILED: {:?}",
            //     templater_violations,
            // );
        };

        // // Record time
        // TODO Implement time
        // let time_dict = [("templating", t0.elapsed().as_secs_f64())]
        //     .iter()
        //     .cloned()
        //     .collect();

        Ok(RenderedFile {
            templated_file: templated_file.unwrap(),
            templater_violations,
            config: config,
            time_dict: HashMap::new(),
            f_name: f_name.to_owned(),
            encoding: encoding.to_owned().unwrap(),
            source_str: f_name.to_owned(),
        })
    }

    /// Parse a rendered file.
    pub fn parse_rendered(rendered: RenderedFile, recurse: bool) -> ParsedString {
        panic!("Not implemented");
        // let t0 = Instant::now();
        // let mut violations = rendered.templater_violations.clone();
        // let tokens: Option<Vec<BaseSegment>>;
        // if let Some(templated_file) = rendered.templated_file {
        //     let (t, lvs, config) = Self::_lex_templated_file(templated_file, &rendered.config);
        //     tokens = t;
        //     violations.extend(lvs);
        // } else {
        //     tokens = None;
        // }

        // TODO Add the timing and linting
        // let t1 = Instant::now();
        // let linter_logger = log::logger();
        // linter_logger.info("PARSING ({})", rendered.fname);
        /*
        let parsed: Option<Box<BaseSegment>>;
        if let Some(token_list) = tokens {
            let (p, pvs) =
                Self::_parse_tokens(&token_list, &rendered.config, recurse, Some(rendered.f_name.to_string()));
            parsed = p;
            violations.extend(pvs);
        } else {
            parsed = None;
        }
        panic!("Not implemented");*/
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

    pub fn _parse_tokens(
        tokens: &Vec<BaseSegment>,
        config: &FluffConfig,
        recurse: bool,
        f_name: Option<String>,
    ) -> (Option<Box<BaseSegment>>, Vec<SQLParseError>) {
        panic!("Not implemented");
    }

    pub fn _lex_templated_file(
        templated_file: TemplatedFile,
        config: &FluffConfig,
    ) -> (Option<Vec<BaseSegment>>, Vec<SQLLexError>, FluffConfig) {
        panic!("Not implemented");
    }

    /// Normalise newlines to unix-style line endings.
    fn normalise_newlines(string: &str) -> String {
        let re = Regex::new(r"\r\n|\r").unwrap();
        re.replace_all(string, "\n").to_string()
    }
}
