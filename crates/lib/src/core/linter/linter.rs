use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

use ahash::{AHashMap, AHashSet};
use itertools::Itertools;
use regex::Regex;
use uuid::Uuid;
use walkdir::WalkDir;

use super::linted_dir::LintedDir;
use super::runner::RunnerContext;
use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;
use crate::core::errors::{SQLFluffUserError, SQLLexError, SQLLintError, SQLParseError, SqlError};
use crate::core::linter::common::{ParsedString, RenderedFile};
use crate::core::linter::linted_file::LintedFile;
use crate::core::linter::linting_result::LintingResult;
use crate::core::parser::lexer::{Lexer, StringOrTemplate};
use crate::core::parser::parser::Parser;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::parser::segments::fix::AnchorEditInfo;
use crate::core::rules::base::{ErasedRule, LintFix, RulePack};
use crate::core::templaters::base::{RawTemplater, TemplatedFile, Templater};
use crate::rules::get_ruleset;

pub struct Linter {
    config: FluffConfig,
    pub formatter: Option<OutputStreamFormatter>,
    templater: Box<dyn Templater>,
    _rules: Vec<ErasedRule>,
}

impl Linter {
    pub fn new(
        config: FluffConfig,
        formatter: Option<OutputStreamFormatter>,
        templater: Option<Box<dyn Templater>>,
    ) -> Linter {
        let rules = crate::rules::layout::rules();
        match templater {
            Some(templater) => Linter { config, formatter, templater, _rules: rules },
            None => Linter {
                config,
                formatter,
                templater: Box::<RawTemplater>::default(),
                _rules: rules,
            },
        }
    }

    /// Lint strings directly.
    pub fn lint_string_wrapped(
        &mut self,
        sql: String,
        f_name: Option<String>,
        fix: Option<bool>,
        rules: Vec<ErasedRule>,
    ) -> LintingResult {
        let f_name = f_name.unwrap_or_else(|| "<string input>".into());

        let mut linted_path = LintedDir::new(f_name.clone());
        linted_path.add(self.lint_string(
            Some(sql),
            Some(f_name),
            fix,
            None,
            None,
            rules,
            fix.unwrap_or_default(),
        ));

        let mut result = LintingResult::new();
        result.add(linted_path);
        result.stop_timer();
        result
    }

    /// Parse a string.
    #[allow(unused_variables)]
    pub fn parse_string(
        &self,
        in_str: String,
        f_name: Option<String>,
        encoding: Option<String>,
        parse_statistics: Option<bool>,
    ) -> Result<ParsedString, SQLFluffUserError> {
        let f_name = f_name.unwrap_or_else(|| "<string>".to_string());
        let encoding = encoding.unwrap_or_else(|| "utf-8".to_string());
        let parse_statistics = parse_statistics.unwrap_or(false);

        let mut violations: Vec<Box<dyn SqlError>> = vec![];

        // Scan the raw file for config commands.
        self.config.process_raw_file_for_config(&in_str);
        let rendered = self.render_string(in_str, f_name.clone(), &self.config, Some(encoding))?;

        for violation in &rendered.templater_violations {
            violations.push(Box::new(violation.clone()));
        }

        // Dispatch the output for the parse header
        if let Some(formatter) = &self.formatter {
            formatter.dispatch_parse_header(/*f_name.clone()*/);
        }

        Ok(Self::parse_rendered(rendered, parse_statistics))
    }

    /// Lint a string.
    #[allow(clippy::too_many_arguments)]
    pub fn lint_string(
        &mut self,
        in_str: Option<String>,
        f_name: Option<String>,
        _fix: Option<bool>,
        config: Option<&FluffConfig>,
        _encoding: Option<String>,
        rules: Vec<ErasedRule>,
        fix: bool,
    ) -> LintedFile {
        // Sort out config, defaulting to the built in config if no override
        let _defaulted_config = config.unwrap_or(&self.config);
        // Parse the string.
        let parsed =
            self.parse_string(in_str.unwrap_or("".to_string()), f_name, None, None).unwrap();

        // Lint the file and return the LintedFile
        self.lint_parsed(parsed, rules, fix)
    }

    pub fn lint_paths(&mut self, mut paths: Vec<PathBuf>, fix: bool) -> LintingResult {
        let mut result = LintingResult::new();

        if paths.is_empty() {
            paths.push(std::env::current_dir().unwrap());
        }

        let mut expanded_paths = Vec::new();
        let mut expanded_path_to_linted_dir = AHashMap::default();

        for path in paths {
            let linted_dir = LintedDir::new(path.display().to_string());
            let key = result.add(linted_dir);

            let paths = self.paths_from_path(path, None, None, None, None);

            expanded_paths.reserve(paths.len());
            expanded_path_to_linted_dir.reserve(paths.len());

            for path in paths {
                expanded_paths.push(path.clone());
                expanded_path_to_linted_dir.insert(path, key);
            }
        }

        let mut runner = RunnerContext::sequential(self);
        for linted_file in runner.run(expanded_paths, fix) {
            let path = expanded_path_to_linted_dir[&linted_file.path];
            result.paths[path].add(linted_file);
        }

        result
    }

    pub fn get_rulepack(&self) -> RulePack {
        let rs = get_ruleset();
        rs.get_rulepack(&self.config)
    }

    pub fn render_file(&mut self, fname: String) -> RenderedFile {
        let in_str = std::fs::read_to_string(&fname).unwrap();
        self.render_string(in_str, fname, &self.config, None).unwrap()
    }

    pub fn lint_rendered(
        &mut self,
        rendered: RenderedFile,
        rule_pack: &RulePack,
        fix: bool,
    ) -> LintedFile {
        let parsed = Self::parse_rendered(rendered, false);
        self.lint_parsed(parsed, rule_pack.rules.clone(), fix)
    }

    pub fn lint_parsed(
        &mut self,
        parsed_string: ParsedString,
        rules: Vec<ErasedRule>,
        fix: bool,
    ) -> LintedFile {
        let violations = parsed_string.violations;
        assert!(violations.is_empty());

        let (tree, initial_linting_errors) = if let Some(tree) = parsed_string.tree {
            self.lint_fix_parsed(tree, rules, fix)
        } else {
            unimplemented!()
        };

        let linted_file = LintedFile {
            path: parsed_string.f_name,
            tree,
            templated_file: parsed_string.templated_file,
            violations: initial_linting_errors,
        };

        if let Some(formatter) = &mut self.formatter {
            formatter.dispatch_file_violations(&linted_file, false, false);
        }

        linted_file
    }

    #[allow(unused_variables)]
    pub fn lint_fix_parsed(
        &self,
        mut tree: ErasedSegment,
        rules: Vec<ErasedRule>,
        fix: bool,
    ) -> (ErasedSegment, Vec<SQLLintError>) {
        let mut tmp;

        let mut initial_linting_errors = Vec::new();
        let phases: &[_] = if fix { &["main", "post"] } else { &["main"] };

        // If we are fixing then we want to loop up to the runaway_limit, otherwise just
        // once for linting.
        let loop_limit = if fix { 10 } else { 1 };

        for &phase in phases {
            let rules_this_phase = if phases.len() > 1 {
                tmp = rules
                    .clone()
                    .into_iter()
                    .filter(|rule| rule.lint_phase() == phase)
                    .collect_vec();

                &tmp
            } else {
                &rules
            };

            for loop_ in 0..(if phase == "main" { loop_limit } else { 2 }) {
                let is_first_linter_pass = phase == phases[0] && loop_ == 0;
                let mut changed = false;

                if is_first_linter_pass {
                    // TODO:
                    // rules_this_phase = rule_pack.rules
                }

                for rule in rules_this_phase {
                    // Performance: After first loop pass, skip rules that don't do fixes. Any
                    // results returned won't be seen by the user anyway (linting errors ADDED by
                    // rules changing SQL, are not reported back to the user - only initial linting
                    // errors), so there's absolutely no reason to run them.
                    if fix && !is_first_linter_pass && !rule.is_fix_compatible() {
                        continue;
                    }

                    let (linting_errors, fixes) =
                        rule.crawl(&self.config.dialect, fix, tree.clone(), &self.config);
                    let anchor_info = compute_anchor_edit_info(fixes.clone());

                    if is_first_linter_pass {
                        initial_linting_errors.extend(linting_errors);
                    }

                    if fix && !fixes.is_empty() {
                        // Do some sanity checks on the fixes before applying.
                        // let anchor_info = BaseSegment.compute_anchor_edit_info(fixes);

                        // This is the happy path. We have fixes, now we want to apply them.
                        let _last_fixes = fixes;

                        let (new_tree, _, _, valid) =
                            tree.apply_fixes(&self.config.dialect, anchor_info);

                        if false {
                            println!(
                                "Fixes for {rule:?} not applied, as it would result in an \
                                 unparsable file. Please report this as a bug with a minimal \
                                 query which demonstrates this warning.",
                            );
                        }

                        tree = new_tree;
                        changed = true;
                    }
                }

                if fix && !changed {
                    break;
                }
            }
        }

        (tree, initial_linting_errors)
    }

    /// Template the file.
    pub fn render_string(
        &self,
        in_str: String,
        f_name: String,
        config: &FluffConfig,
        encoding: Option<String>,
    ) -> Result<RenderedFile, SQLFluffUserError> {
        // TODO Implement loggers eventually
        // let linter_logger = log::logger();
        // linter_logger.info!("TEMPLATING RAW [{}] ({})", self.templater.name, f_name);

        // Start the templating timer
        let _t0 = Instant::now();

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
        //         "Attempt to set templater to {} failed. Using {} templater. Templater
        // cannot be set in a .sqlfluff file in a subdirectory of the current working
        // directory. It can be set in a .sqlfluff in the current working directory. See
        // Nesting section of the docs for more details.",         config.get("
        // templater_obj").name,         self.templater.name,
        //     ));
        // }

        #[allow(unused_assignments)]
        let mut templated_file = None;

        let templater_violations = vec![];
        match self.templater.process(
            in_str.as_str(),
            f_name.as_str(),
            Some(config),
            self.formatter.as_ref(),
        ) {
            Ok(file) => {
                templated_file = Some(file);
            }
            Err(_s) => {
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
            config: config.clone(),
            time_dict: AHashMap::new(),
            f_name: f_name.to_owned(),
            encoding: encoding.to_owned().unwrap_or_else(|| "UTF-8".into()),
            source_str: f_name.to_owned(),
        })
    }

    /// Parse a rendered file.

    pub fn parse_rendered(rendered: RenderedFile, parse_statistics: bool) -> ParsedString {
        // panic!("Not implemented");

        let t0 = Instant::now();
        let violations = rendered.templater_violations.clone();
        if !violations.is_empty() {
            unimplemented!()
        }

        let mut violations = Vec::new();
        #[allow(unused_assignments)]
        let mut tokens: Option<Vec<ErasedSegment>> = None;

        if rendered.templated_file.is_templated() {
            let (t, lvs) =
                Self::lex_templated_file(rendered.templated_file.clone(), &rendered.config);
            tokens = t;
            if !lvs.is_empty() {
                unimplemented!("violations.extend(lvs);")
            }
        } else {
            tokens = None;
        };

        let t1 = Instant::now();
        // TODO Add the logging
        // let linter_logger = log::logger();
        // linter_logger.info("PARSING ({})", rendered.fname);

        let parsed: Option<ErasedSegment>;
        if let Some(token_list) = tokens {
            let (p, pvs) = Self::parse_tokens(
                &token_list,
                &rendered.config,
                Some(rendered.f_name.to_string()),
                parse_statistics,
            );
            parsed = p;
            violations.extend(pvs.into_iter().map(Into::into));
        } else {
            parsed = None;
        };

        // TODO Time_Dict should be a structure, it should also probably replace f64
        // with Duration type
        let mut time_dict = rendered.time_dict.clone();
        time_dict.insert("lexing".to_string(), (t1 - t0).as_secs_f64());
        time_dict.insert("parsing".to_string(), (Instant::now() - t1).as_secs_f64());

        ParsedString {
            tree: parsed,
            violations,
            time_dict,
            templated_file: rendered.templated_file,
            config: rendered.config,
            f_name: rendered.f_name,
            source_str: rendered.source_str,
        }
    }

    fn parse_tokens(
        tokens: &[ErasedSegment],
        config: &FluffConfig,
        f_name: Option<String>,
        parse_statistics: bool,
    ) -> (Option<ErasedSegment>, Vec<SQLParseError>) {
        let mut parser = Parser::new(config, None);
        let mut violations: Vec<SQLParseError> = Vec::new();

        let parsed = match parser.parse(tokens, f_name, parse_statistics) {
            Ok(parsed) => parsed,
            Err(error) => {
                violations.push(error);
                None
            }
        };

        (parsed, violations)
    }

    /// Lex a templated file.
    ///
    /// NOTE: This potentially mutates the config, so make sure to
    /// use the returned one.
    fn lex_templated_file(
        templated_file: TemplatedFile,
        config: &FluffConfig,
    ) -> (Option<Vec<ErasedSegment>>, Vec<SQLLexError>) {
        let mut violations: Vec<SQLLexError> = vec![];
        // linter_logger.info("LEXING RAW ({})", templated_file.fname);
        // Get the lexer
        let lexer = Lexer::new(config, None);
        // Lex the file and log any problems
        let result = lexer.lex(StringOrTemplate::Template(templated_file));
        match result {
            Err(_err) => {
                unimplemented!("violations.push(_err)");
                // return (None, violations, config.clone());
            }
            Ok((tokens, lex_vs)) => {
                violations.extend(lex_vs);

                if tokens.is_empty() {
                    return (None, violations);
                }

                (tokens.into(), violations)
            }
        }
    }

    /// Normalise newlines to unix-style line endings.
    fn normalise_newlines(string: &str) -> String {
        let re = Regex::new(r"\r\n|\r").unwrap();
        re.replace_all(string, "\n").to_string()
    }

    // Return a set of sql file paths from a potentially more ambiguous path string.
    // Here we also deal with the .sqlfluffignore file if present.
    // When a path to a file to be linted is explicitly passed
    // we look for ignore files in all directories that are parents of the file,
    // up to the current directory.
    // If the current directory is not a parent of the file we only
    // look for an ignore file in the direct parent of the file.
    fn paths_from_path(
        &self,
        path: PathBuf,
        ignore_file_name: Option<String>,
        ignore_non_existent_files: Option<bool>,
        ignore_files: Option<bool>,
        working_path: Option<String>,
    ) -> Vec<String> {
        let ignore_file_name = ignore_file_name.unwrap_or_else(|| String::from(".sqlfluffignore"));
        let ignore_non_existent_files = ignore_non_existent_files.unwrap_or(false);
        let ignore_files = ignore_files.unwrap_or(true);
        let _working_path =
            working_path.unwrap_or_else(|| std::env::current_dir().unwrap().display().to_string());

        let Ok(metadata) = std::fs::metadata(&path) else {
            if ignore_non_existent_files {
                return Vec::new();
            } else {
                panic!("Specified path does not exist. Check it/they exist(s): {:?}", path);
            }
        };

        // Files referred to exactly are also ignored if
        // matched, but we warn the users when that happens
        let is_exact_file = metadata.is_file();

        let mut path_walk = if is_exact_file {
            let path = Path::new(&path);
            let dirpath = path.parent().unwrap().to_str().unwrap().to_string();
            let files = vec![path.file_name().unwrap().to_str().unwrap().to_string()];
            vec![(dirpath, None, files)]
        } else {
            WalkDir::new(&path)
                .into_iter()
                .filter_map(Result::ok) // Filter out the Result and get DirEntry
                .map(|entry| {
                    let dirpath = entry.path().parent().unwrap().to_str().unwrap().to_string();
                    let files = vec![entry.file_name().to_str().unwrap().to_string()];
                    (dirpath, None, files)
                })
                .collect_vec()
        };

        // TODO:
        // let ignore_file_paths = ConfigLoader.find_ignore_config_files(
        //     path=path, working_path=working_path, ignore_file_name=ignore_file_name
        // );
        let ignore_file_paths: Vec<String> = Vec::new();

        // Add paths that could contain "ignore files"
        // to the path_walk list
        let path_walk_ignore_file: Vec<(String, Option<()>, Vec<String>)> = ignore_file_paths
            .iter()
            .map(|ignore_file_path| {
                let ignore_file_path = Path::new(ignore_file_path);

                // Extracting the directory name from the ignore file path
                let dir_name = ignore_file_path.parent().unwrap().to_str().unwrap().to_string();

                // Only one possible file, since we only
                // have one "ignore file name"
                let file_name =
                    vec![ignore_file_path.file_name().unwrap().to_str().unwrap().to_string()];

                (dir_name, None, file_name)
            })
            .collect();

        path_walk.extend(path_walk_ignore_file);

        let mut buffer = Vec::new();
        let mut ignores = AHashMap::new();
        let sql_file_exts = self.config.sql_file_exts(); // Replace with actual extensions

        for (dirpath, _, filenames) in path_walk {
            for fname in filenames {
                let fpath = Path::new(&dirpath).join(&fname);

                // Handle potential .sqlfluffignore files
                if ignore_files && fname == ignore_file_name {
                    let file = File::open(&fpath).unwrap();
                    let lines = BufReader::new(file).lines();
                    let spec = lines.map_while(Result::ok); // Simple placeholder for pathspec logic
                    ignores.insert(dirpath.clone(), spec.collect::<Vec<String>>());

                    // We don't need to process the ignore file any further
                    continue;
                }

                // We won't purge files *here* because there's an edge case
                // that the ignore file is processed after the sql file.

                // Scan for remaining files
                for ext in sql_file_exts {
                    // is it a sql file?
                    if fname.to_lowercase().ends_with(ext) {
                        buffer.push(fpath.clone());
                    }
                }
            }
        }

        let mut filtered_buffer = AHashSet::new();

        for fpath in buffer {
            let npath = crate::helpers::normalize(&fpath).to_str().unwrap().to_string();
            filtered_buffer.insert(npath);
        }

        let mut files = filtered_buffer.into_iter().collect_vec();
        files.sort();
        files
    }
}

fn compute_anchor_edit_info(fixes: Vec<LintFix>) -> AHashMap<Uuid, AnchorEditInfo> {
    let mut anchor_info = AHashMap::new();

    for fix in fixes {
        let anchor_id = fix.anchor.get_uuid().unwrap();
        anchor_info.entry(anchor_id).or_insert_with(AnchorEditInfo::default).add(fix);
    }

    anchor_info
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::linter::Linter;

    fn normalise_paths(paths: Vec<String>) -> Vec<String> {
        paths.into_iter().map(|path| path.replace(['/', '\\'], ".")).collect()
    }

    #[test]
    fn test_linter_path_from_paths_dir() {
        // Test extracting paths from directories.
        let lntr = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None); // Assuming Linter has a new() method for initialization
        let paths = lntr.paths_from_path("test/fixtures/lexer".into(), None, None, None, None);
        let expected = vec![
            "test.fixtures.lexer.basic.sql",
            "test.fixtures.lexer.block_comment.sql",
            "test.fixtures.lexer.inline_comment.sql",
        ];
        assert_eq!(normalise_paths(paths), expected);
    }

    #[test]
    fn test_linter_path_from_paths_default() {
        // Test .sql files are found by default.
        let lntr = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None); // Assuming Linter has a new() method for initialization
        let paths = normalise_paths(lntr.paths_from_path(
            "test/fixtures/linter".into(),
            None,
            None,
            None,
            None,
        ));
        assert!(paths.contains(&"test.fixtures.linter.passing.sql".to_string()));
        assert!(paths.contains(&"test.fixtures.linter.passing_cap_extension.SQL".to_string()));
        assert!(!paths.contains(&"test.fixtures.linter.discovery_file.txt".to_string()));
    }

    #[test]
    fn test_linter_path_from_paths_exts() {
        // Assuming Linter is initialized with a configuration similar to Python's
        // FluffConfig
        let config =
            FluffConfig::new(<_>::default(), None, None).with_sql_file_exts(vec![".txt".into()]);
        let lntr = Linter::new(config, None, None); // Assuming Linter has a new() method for initialization

        let paths = lntr.paths_from_path("test/fixtures/linter".into(), None, None, None, None);

        // Normalizing paths as in the Python version
        let normalized_paths = normalise_paths(paths);

        // Assertions as per the Python test
        assert!(!normalized_paths.contains(&"test.fixtures.linter.passing.sql".into()));
        assert!(
            !normalized_paths.contains(&"test.fixtures.linter.passing_cap_extension.SQL".into())
        );
        assert!(normalized_paths.contains(&"test.fixtures.linter.discovery_file.txt".into()));
    }

    #[test]
    fn test__linter__path_from_paths__file() {
        let lntr = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None); // Assuming Linter has a new() method for initialization
        let paths = lntr.paths_from_path(
            "test/fixtures/linter/indentation_errors.sql".into(),
            None,
            None,
            None,
            None,
        );

        assert_eq!(normalise_paths(paths), &["test.fixtures.linter.indentation_errors.sql"]);
    }

    // test__linter__skip_large_bytes
    // test__linter__path_from_paths__not_exist
    // test__linter__path_from_paths__not_exist_ignore
    // test__linter__path_from_paths__explicit_ignore
    // test__linter__path_from_paths__sqlfluffignore_current_directory
    // test__linter__path_from_paths__dot
    // test__linter__path_from_paths__ignore
    // test__linter__lint_string_vs_file
    // test__linter__get_violations_filter_rules
    // test__linter__linting_result__sum_dicts
    // test__linter__linting_result__combine_dicts
    // test__linter__linting_result_check_tuples_by_path
    // test__linter__linting_result_get_violations
    // test__linter__linting_parallel_thread
    // test_lint_path_parallel_wrapper_exception
    // test__linter__get_runner_processes
    // test__linter__linting_unexpected_error_handled_gracefully
    #[test]
    fn test__linter__empty_file() {
        let linter = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None);
        let parsed = linter.parse_string("".into(), None, None, None).unwrap();

        assert!(parsed.violations.is_empty());
    }

    // test__linter__mask_templated_violations
    // test__linter__encoding
    // test_delayed_exception
    // test__attempt_to_change_templater_warning

    #[test]
    #[ignore = "The implementation of Lexer::lex_templated_file is required"]
    fn test_advanced_api_methods() {
        let sql = "
        WITH cte AS (
            SELECT * FROM tab_a
        )
        SELECT
            cte.col_a,
            tab_b.col_b
        FROM cte
        INNER JOIN tab_b;
        "
        .to_string();

        let linter = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None);
        let _parsed = linter.parse_string(sql, None, None, None).unwrap();
    }

    #[test]
    fn test_normalise_newlines() {
        let in_str = "SELECT\r\n foo\n FROM \r \n\r bar;";
        let out_str = "SELECT\n foo\n FROM \n \n\n bar;";

        assert_eq!(Linter::normalise_newlines(in_str), out_str);
    }
}
