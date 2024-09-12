use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ahash::{AHashMap, AHashSet};
use itertools::Itertools;
use regex::Regex;
use smol_str::{SmolStr, ToSmolStr};
use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::errors::{
    SQLFluffUserError, SQLLexError, SQLLintError, SQLParseError, SqlError,
};
use sqruff_lib_core::helpers;
use sqruff_lib_core::linter::compute_anchor_edit_info;
use sqruff_lib_core::parser::lexer::{Lexer, StringOrTemplate};
use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, Tables};
use sqruff_lib_core::parser::segments::fix::SourceFix;
use sqruff_lib_core::templaters::base::TemplatedFile;
use walkdir::WalkDir;

use super::linted_dir::LintedDir;
use super::runner::RunnerContext;
use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;
use crate::core::linter::common::{ParsedString, RenderedFile};
use crate::core::linter::linted_file::LintedFile;
use crate::core::linter::linting_result::LintingResult;
use crate::core::rules::base::{ErasedRule, LintPhase, RulePack};
use crate::rules::get_ruleset;
use crate::templaters::placeholder::PlaceholderTemplater;
use crate::templaters::raw::RawTemplater;
use crate::templaters::Templater;

pub struct Linter {
    config: FluffConfig,
    pub formatter: Option<OutputStreamFormatter>,
    templater: Arc<dyn Templater>,
    pub _rules: Vec<ErasedRule>,
}

impl Linter {
    pub fn new(
        config: FluffConfig,
        formatter: Option<OutputStreamFormatter>,
        templater: Option<Arc<dyn Templater>>,
    ) -> Linter {
        let rules = crate::rules::layout::rules();
        let templater: Arc<dyn Templater> = match templater {
            Some(templater) => templater,
            None => {
                let templater = config.get("templater", "core").as_string();
                match templater {
                    Some("placeholder") => Arc::<PlaceholderTemplater>::default(),
                    Some("raw") => Arc::<RawTemplater>::default(),
                    None => Arc::<RawTemplater>::default(),
                    _ => panic!("Unknown templater: {}", templater.unwrap()),
                }
            }
        };
        Linter { config, formatter, templater, _rules: rules }
    }

    /// Lint strings directly.
    pub fn lint_string_wrapped(
        &mut self,
        sql: &str,
        f_name: Option<String>,
        fix: Option<bool>,
        rules: Vec<ErasedRule>,
    ) -> LintingResult {
        let f_name = f_name.unwrap_or_else(|| "<string input>".into());

        let mut linted_path = LintedDir::new(f_name.clone());
        linted_path.add(self.lint_string(
            sql,
            Some(f_name),
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
    pub fn parse_string(
        &self,
        tables: &Tables,
        in_str: &str,
        f_name: Option<String>,
        encoding: Option<String>,
        parse_statistics: Option<bool>,
    ) -> Result<ParsedString, SQLFluffUserError> {
        let f_name = f_name.unwrap_or_else(|| "<string>".to_string());
        let encoding = encoding.unwrap_or_else(|| "utf-8".to_string());
        let parse_statistics = parse_statistics.unwrap_or(false);

        let mut violations: Vec<Box<dyn SqlError>> = vec![];

        // Scan the raw file for config commands.
        self.config.process_raw_file_for_config(in_str);
        let rendered = self.render_string(in_str, f_name, &self.config, Some(encoding))?;

        for violation in &rendered.templater_violations {
            violations.push(Box::new(violation.clone()));
        }

        // Dispatch the output for the parse header
        if let Some(formatter) = &self.formatter {
            formatter.dispatch_parse_header(/*f_name.clone()*/);
        }

        Ok(self.parse_rendered(tables, rendered, parse_statistics))
    }

    /// Lint a string.
    #[allow(clippy::too_many_arguments)]
    pub fn lint_string(
        &self,
        in_str: &str,
        f_name: Option<String>,
        config: Option<&FluffConfig>,
        _encoding: Option<String>,
        rules: Vec<ErasedRule>,
        fix: bool,
    ) -> LintedFile {
        // Sort out config, defaulting to the built in config if no override
        let _defaulted_config = config.unwrap_or(&self.config);
        // Parse the string.
        let tables = Tables::default();
        let parsed = self.parse_string(&tables, in_str, f_name, None, None).unwrap();

        // Lint the file and return the LintedFile
        self.lint_parsed(&tables, parsed, rules, fix)
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

            let paths = if path.is_file() {
                vec![path.to_string_lossy().to_string()]
            } else {
                self.paths_from_path(path, None, None, None, None)
            };

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

    pub fn render_file(&self, fname: String) -> RenderedFile {
        let in_str = std::fs::read_to_string(&fname).unwrap();
        self.render_string(&in_str, fname, &self.config, None).unwrap()
    }

    pub fn lint_rendered(
        &self,
        rendered: RenderedFile,
        rule_pack: &RulePack,
        fix: bool,
    ) -> LintedFile {
        let tables = Tables::default();
        let parsed = self.parse_rendered(&tables, rendered, false);
        self.lint_parsed(&tables, parsed, rule_pack.rules.clone(), fix)
    }

    pub fn lint_parsed(
        &self,
        tables: &Tables,
        parsed_string: ParsedString,
        rules: Vec<ErasedRule>,
        fix: bool,
    ) -> LintedFile {
        let mut violations = parsed_string.violations;

        let (tree, initial_linting_errors) = parsed_string
            .tree
            .map(|tree| {
                self.lint_fix_parsed(tables, tree, &parsed_string.templated_file, rules, fix)
            })
            .unzip();

        violations.extend(initial_linting_errors.unwrap_or_default().into_iter().map(Into::into));

        let linted_file = LintedFile {
            path: parsed_string.f_name,
            patches: tree
                .map_or(Vec::new(), |tree| tree.iter_patches(&parsed_string.templated_file)),
            templated_file: parsed_string.templated_file,
            violations,
        };

        if let Some(formatter) = &self.formatter {
            formatter.dispatch_file_violations(&linted_file, false);
        }

        linted_file
    }

    pub fn lint_fix_parsed(
        &self,
        tables: &Tables,
        mut tree: ErasedSegment,
        templated_file: &TemplatedFile,
        rules: Vec<ErasedRule>,
        fix: bool,
    ) -> (ErasedSegment, Vec<SQLLintError>) {
        let mut tmp;
        let mut initial_linting_errors = Vec::new();
        let phases: &[_] =
            if fix { &[LintPhase::Main, LintPhase::Post] } else { &[LintPhase::Main] };
        let mut previous_versions: AHashSet<(SmolStr, Vec<SourceFix>)> =
            [(tree.raw().to_smolstr(), vec![])].into_iter().collect();

        // If we are fixing then we want to loop up to the runaway_limit, otherwise just
        // once for linting.
        let loop_limit = if fix { 10 } else { 1 };

        for phase in phases {
            let mut rules_this_phase = if phases.len() > 1 {
                tmp = rules
                    .clone()
                    .into_iter()
                    .filter(|rule| rule.lint_phase() == *phase)
                    .collect_vec();

                &tmp
            } else {
                &rules
            };

            for loop_ in 0..(if *phase == LintPhase::Main { loop_limit } else { 2 }) {
                let is_first_linter_pass = *phase == phases[0] && loop_ == 0;
                let mut changed = false;

                if is_first_linter_pass {
                    rules_this_phase = &rules;
                }

                let last_fixes = Vec::new();
                for rule in rules_this_phase {
                    // Performance: After first loop pass, skip rules that don't do fixes. Any
                    // results returned won't be seen by the user anyway (linting errors ADDED by
                    // rules changing SQL, are not reported back to the user - only initial linting
                    // errors), so there's absolutely no reason to run them.
                    if fix && !is_first_linter_pass && !rule.is_fix_compatible() {
                        continue;
                    }

                    let (linting_errors, fixes) = rule.crawl(
                        tables,
                        &self.config.dialect,
                        fix,
                        templated_file,
                        tree.clone(),
                        &self.config,
                    );

                    if is_first_linter_pass {
                        initial_linting_errors.extend(linting_errors);
                    }

                    if fix && !fixes.is_empty() {
                        // Do some sanity checks on the fixes before applying.
                        // let anchor_info = BaseSegment.compute_anchor_edit_info(fixes);

                        // This is the happy path. We have fixes, now we want to apply them.

                        if fixes == last_fixes {
                            eprintln!(
                                "One fix for {} not applied, it would re-cause the same error.",
                                rule.code()
                            );
                            continue;
                        }

                        let mut anchor_info = compute_anchor_edit_info(fixes);
                        let (new_tree, _, _, _valid) = tree.apply_fixes(&mut anchor_info);

                        if false {
                            println!(
                                "Fixes for {rule:?} not applied, as it would result in an \
                                 unparsable file. Please report this as a bug with a minimal \
                                 query which demonstrates this warning.",
                            );
                        }

                        let loop_check_tuple =
                            (new_tree.raw().to_smolstr(), new_tree.get_source_fixes());

                        if previous_versions.insert(loop_check_tuple) {
                            tree = new_tree;
                            changed = true;
                            continue;
                        }
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
        in_str: &str,
        f_name: String,
        config: &FluffConfig,
        encoding: Option<String>,
    ) -> Result<RenderedFile, SQLFluffUserError> {
        let in_str = Self::normalise_newlines(in_str);

        if let Some(error) = config.verify_dialect_specified() {
            return Err(error);
        }

        let templater_violations = vec![];
        let templated_file = match self.templater.process(
            in_str.as_str(),
            f_name.as_str(),
            Some(config),
            self.formatter.as_ref(),
        ) {
            Ok(file) => file,
            Err(_s) => {
                // TODO Implement linter warning
                panic!("not implemented")
                // linter_logger::warning(s.to_string());
            }
        };

        Ok(RenderedFile {
            templated_file,
            templater_violations,

            f_name: f_name.to_owned(),
            encoding: encoding.to_owned().unwrap_or_else(|| "UTF-8".into()),
            source_str: f_name.to_owned(),
        })
    }

    /// Parse a rendered file.
    pub fn parse_rendered(
        &self,
        tables: &Tables,
        rendered: RenderedFile,
        parse_statistics: bool,
    ) -> ParsedString {
        let violations = rendered.templater_violations.clone();
        if !violations.is_empty() {
            unimplemented!()
        }

        let mut violations = Vec::new();
        let tokens = if rendered.templated_file.is_templated() {
            let (t, lvs) = Self::lex_templated_file(
                tables,
                rendered.templated_file.clone(),
                &self.config.dialect,
            );
            if !lvs.is_empty() {
                unimplemented!("violations.extend(lvs);")
            }
            t
        } else {
            None
        };

        let parsed: Option<ErasedSegment>;
        if let Some(token_list) = tokens {
            let (p, pvs) = Self::parse_tokens(
                tables,
                &token_list,
                &self.config,
                Some(rendered.f_name.to_string()),
                parse_statistics,
            );
            parsed = p;
            violations.extend(pvs.into_iter().map(Into::into));
        } else {
            parsed = None;
        };

        ParsedString {
            tree: parsed,
            violations,
            templated_file: rendered.templated_file,
            f_name: rendered.f_name,
            source_str: rendered.source_str,
        }
    }

    fn parse_tokens(
        tables: &Tables,
        tokens: &[ErasedSegment],
        config: &FluffConfig,
        f_name: Option<String>,
        parse_statistics: bool,
    ) -> (Option<ErasedSegment>, Vec<SQLParseError>) {
        let parser: Parser = config.into();
        let mut violations: Vec<SQLParseError> = Vec::new();

        let parsed = match parser.parse(tables, tokens, f_name, parse_statistics) {
            Ok(parsed) => parsed,
            Err(error) => {
                violations.push(error);
                None
            }
        };

        (parsed, violations)
    }

    /// Lex a templated file.
    pub fn lex_templated_file(
        tables: &Tables,
        templated_file: TemplatedFile,
        dialect: &Dialect,
    ) -> (Option<Vec<ErasedSegment>>, Vec<SQLLexError>) {
        let mut violations: Vec<SQLLexError> = vec![];
        // linter_logger.info("LEXING RAW ({})", templated_file.fname);
        // Get the lexer
        let lexer = Lexer::new(dialect);
        // Lex the file and log any problems
        let result = lexer.lex(tables, StringOrTemplate::Template(templated_file));
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
            let npath = helpers::normalize(&fpath).to_str().unwrap().to_string();
            filtered_buffer.insert(npath);
        }

        let mut files = filtered_buffer.into_iter().collect_vec();
        files.sort();
        files
    }

    pub fn config_mut(&mut self) -> &mut FluffConfig {
        &mut self.config
    }
}

#[cfg(test)]
mod tests {
    use sqruff_lib_core::parser::segments::base::Tables;

    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

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
    fn test_linter_path_from_paths_file() {
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
    fn test_linter_empty_file() {
        let linter = Linter::new(FluffConfig::new(<_>::default(), None, None), None, None);
        let tables = Tables::default();
        let parsed = linter.parse_string(&tables, "", None, None, None).unwrap();

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
        let tables = Tables::default();
        let _parsed = linter.parse_string(&tables, &sql, None, None, None).unwrap();
    }

    #[test]
    fn test_normalise_newlines() {
        let in_str = "SELECT\r\n foo\n FROM \r \n\r bar;";
        let out_str = "SELECT\n foo\n FROM \n \n\n bar;";

        assert_eq!(Linter::normalise_newlines(in_str), out_str);
    }
}
