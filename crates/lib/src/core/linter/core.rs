use std::borrow::Cow;
use std::sync::OnceLock;

use crate::api::{Mode, ParseErrors, SourceId};
use crate::core::config::FluffConfig;
use crate::core::linter::common::{ParsedString, RenderedFile, RenderedSource};
use crate::core::linter::linted_file::LintedFile;
use crate::core::rules::noqa::IgnoreMask;
use crate::core::rules::{ErasedRule, Exception, LintPhase, RulePack};
use crate::rules::get_ruleset;
use crate::templaters::{
    Templater, TemplaterError, TemplaterInput, TemplaterKind, TemplaterOutput, source_id_name,
};
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use smol_str::{SmolStr, ToSmolStr};
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::errors::{
    SQLBaseError, SQLFluffUserError, SQLLexError, SQLLintError, SQLParseError,
};
use sqruff_lib_core::linter::compute_anchor_edit_info;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};
use sqruff_lib_core::templaters::TemplatedFile;

pub struct Linter {
    config: FluffConfig,
    templater: &'static dyn Templater,
    rules: OnceLock<Vec<ErasedRule>>,

    parse_errors: ParseErrors,
}

impl Linter {
    pub fn new(
        config: FluffConfig,
        templater: Option<&'static dyn Templater>,
        parse_errors: ParseErrors,
    ) -> Result<Linter, String> {
        let templater: &'static dyn Templater = match templater {
            Some(templater) => templater,
            None => Linter::get_templater(&config)?,
        };
        Ok(Linter {
            config,
            templater,
            rules: OnceLock::new(),
            parse_errors,
        })
    }

    pub fn get_templater(config: &FluffConfig) -> Result<&'static dyn Templater, String> {
        config.templater_kind().map(TemplaterKind::templater)
    }

    /// Lint strings directly.
    pub fn lint_string_wrapped(
        &mut self,
        sql: &str,
        mode: Mode,
    ) -> Result<LintedFile, SQLFluffUserError> {
        let filename = "<string input>".to_owned();
        self.lint_string(sql, Some(filename), mode)
    }

    /// Parse a string.
    pub fn parse_string(
        &self,
        tables: &Tables,
        sql: &str,
        filename: Option<String>,
    ) -> Result<ParsedString, SQLFluffUserError> {
        let f_name = filename.unwrap_or_else(|| "<string>".to_string());

        // Scan the raw file for config commands.
        self.config.process_raw_file_for_config(sql);
        let rendered = self.render_string(sql, f_name, &self.config)?;

        Ok(self.parse_rendered(tables, rendered))
    }

    /// Lint a string.
    pub fn lint_string(
        &self,
        sql: &str,
        filename: Option<String>,
        mode: Mode,
    ) -> Result<LintedFile, SQLFluffUserError> {
        let tables = Tables::default();
        let parsed = self.parse_string(&tables, sql, filename)?;

        // Lint the file and return the LintedFile
        self.lint_parsed(&tables, parsed, mode)
    }

    pub fn get_rulepack(&self) -> Result<RulePack, SQLFluffUserError> {
        let rs = get_ruleset();
        rs.get_rulepack(&self.config)
    }

    pub fn lint_rendered(
        &self,
        rendered: RenderedFile,
        mode: Mode,
    ) -> Result<LintedFile, SQLFluffUserError> {
        let tables = Tables::default();
        let parsed = self.parse_rendered(&tables, rendered);
        self.lint_parsed(&tables, parsed, mode)
    }

    pub fn lint_parsed(
        &self,
        tables: &Tables,
        parsed_string: ParsedString,
        mode: Mode,
    ) -> Result<LintedFile, SQLFluffUserError> {
        let mut violations = parsed_string.violations;

        let (patches, ignore_mask, initial_linting_errors) = match parsed_string.tree {
            Some(erased_segment) => {
                let (tree, ignore_mask, initial_linting_errors) = self.lint_fix_parsed(
                    tables,
                    erased_segment,
                    &parsed_string.templated_file,
                    mode,
                )?;
                let patches = tree.iter_patches(&parsed_string.templated_file);
                (patches, ignore_mask, initial_linting_errors)
            }
            None => (Vec::new(), None, Vec::new()),
        };
        violations.extend(initial_linting_errors.into_iter().map_into());

        // Filter violations with ignore mask
        if let Some(ignore_mask) = &ignore_mask {
            violations.retain(|violation| !ignore_mask.is_masked(violation, None));
        }

        // TODO Need to error out unused noqas
        let linted_file = LintedFile::new(
            parsed_string.filename,
            patches,
            parsed_string.templated_file,
            violations,
            ignore_mask,
        );

        Ok(linted_file)
    }

    pub fn lint_fix_parsed(
        &self,
        tables: &Tables,
        mut tree: ErasedSegment,
        templated_file: &TemplatedFile,
        mode: Mode,
    ) -> Result<(ErasedSegment, Option<IgnoreMask>, Vec<SQLLintError>), SQLFluffUserError> {
        let mut initial_violations = Vec::new();
        let phases: &[_] = match mode {
            Mode::Check => &[LintPhase::Main],
            Mode::Fix => &[LintPhase::Main, LintPhase::Post],
        };
        let mut previous_versions: HashSet<(SmolStr, bool)> =
            [(tree.raw().to_smolstr(), false)].into_iter().collect();

        // If we are fixing then we want to loop up to the runaway_limit, otherwise just
        // once for linting.
        let loop_limit = match mode {
            Mode::Check => 1,
            Mode::Fix => 10,
        };
        // Look for comment segments which might indicate lines to ignore.
        let (ignore_mask, violations): (Option<IgnoreMask>, Vec<SQLBaseError>) = {
            let disable_noqa = self
                .config
                .get("disable_noqa", "core")
                .as_bool()
                .unwrap_or(false);
            if disable_noqa {
                (None, Vec::new())
            } else {
                let (ignore_mask, errors) = IgnoreMask::from_tree(&tree);
                (Some(ignore_mask), errors)
            }
        };

        initial_violations.extend(violations.into_iter().map_into());

        let mut anchor_info = HashMap::default();

        for phase in phases {
            let loop_limit = if *phase == LintPhase::Main {
                loop_limit
            } else {
                2
            };
            let rules = self.rules()?;
            let filtered_rules;
            let mut rules_this_phase: &[ErasedRule] = if phases.len() > 1 {
                filtered_rules = rules
                    .iter()
                    .filter(|rule| rule.lint_phase() == *phase)
                    .cloned()
                    .collect_vec();
                &filtered_rules
            } else {
                rules
            };

            for loop_ in 0..loop_limit {
                let is_first_linter_pass = *phase == phases[0] && loop_ == 0;
                let mut changed = false;

                if is_first_linter_pass {
                    rules_this_phase = self.rules()?;
                }

                for rule in rules_this_phase {
                    anchor_info.clear();

                    // Performance: After first loop pass, skip rules that don't do fixes. Any
                    // results returned won't be seen by the user anyway (linting errors ADDED by
                    // rules changing SQL, are not reported back to the user - only initial linting
                    // errors), so there's absolutely no reason to run them.
                    if matches!(mode, Mode::Fix)
                        && !is_first_linter_pass
                        && !rule.is_fix_compatible()
                    {
                        continue;
                    }

                    let result = crate::core::rules::crawl(
                        rule,
                        tables,
                        &self.config.dialect,
                        templated_file,
                        tree.clone(),
                        &self.config,
                        &mut |mut result| {
                            if ignore_mask.as_ref().is_none_or(|ignore_mask| {
                                !ignore_mask.is_masked(&result, rule.into())
                            }) {
                                compute_anchor_edit_info(
                                    &mut anchor_info,
                                    std::mem::take(&mut result.fixes),
                                );

                                if is_first_linter_pass {
                                    initial_violations.extend(result.to_linting_error(rule));
                                }
                            }
                        },
                    );

                    if let Err(Exception) = result {
                        if is_first_linter_pass {
                            initial_violations.push(
                                SQLLintError::new(
                                    "Unexpected exception. Could you open an issue at https://github.com/quarylabs/sqruff",
                                    tree.clone(),
                                    false,
                                ),
                            );
                        }

                        continue;
                    }

                    if matches!(mode, Mode::Fix) && !anchor_info.is_empty() {
                        let (new_tree, _, _) = tree.apply_fixes(&mut anchor_info);
                        let has_source_fixes = !new_tree.get_all_source_fixes().is_empty();

                        // For loop detection, we check raw and whether we have source_fixes.
                        // Source fixes don't change the tree raw, so once we have source_fixes
                        // and raw is unchanged, we're done.
                        let loop_check_tuple = (new_tree.raw().to_smolstr(), has_source_fixes);

                        if previous_versions.insert(loop_check_tuple) {
                            tree = new_tree;
                            changed = true;
                            continue;
                        }
                    }
                }

                if matches!(mode, Mode::Fix) && !changed {
                    break;
                }
            }
        }

        Ok((tree, ignore_mask, initial_violations))
    }

    /// Template the file.
    pub fn render_string(
        &self,
        sql: &str,
        filename: String,
        config: &FluffConfig,
    ) -> Result<RenderedFile, SQLFluffUserError> {
        let source_id = SourceId::Virtual(filename);
        self.render_source(sql, &source_id, config)
            .map_err(TemplaterError::into_user_error)?
            .into_rendered()
            .ok_or_else(|| SQLFluffUserError::new("Templater skipped string input".to_string()))
    }

    pub(crate) fn render_source(
        &self,
        sql: &str,
        source_id: &SourceId,
        config: &FluffConfig,
    ) -> Result<RenderedSource, TemplaterError> {
        let sql = Self::normalise_newlines(sql);

        if let Some(error) = config.verify_dialect_specified() {
            return Err(TemplaterError::Failed(error));
        }

        let templater_violations = vec![];
        let input = TemplaterInput {
            source: sql.as_ref(),
            source_id,
        };
        let mut results = self.templater.process(std::slice::from_ref(&input), config);

        match results.pop() {
            Some(Ok(TemplaterOutput::Rendered(templated_file))) => {
                Ok(RenderedSource::Rendered(RenderedFile {
                    templated_file,
                    templater_violations,
                    filename: source_id_name(source_id),
                    source_str: sql.to_string(),
                }))
            }
            Some(Ok(TemplaterOutput::Skipped(reason))) => Ok(RenderedSource::Skipped(reason)),
            Some(Err(err)) => Err(err),
            None => Err(TemplaterError::Failed(SQLFluffUserError::new(format!(
                "Templater returned no results for file {}",
                source_id_name(source_id)
            )))),
        }
    }

    /// Parse a rendered file.
    pub fn parse_rendered(&self, tables: &Tables, rendered: RenderedFile) -> ParsedString {
        let templater_violations = rendered.templater_violations.clone();
        if !templater_violations.is_empty() {
            // If the templater reported violations (e.g., dbt/jinja templater
            // failed), skip parsing. This prevents false positive lint errors
            // (like LT01 spacing violations on `{{ }}` template syntax) that
            // would occur if we tried to parse the raw source as SQL.
            let violations: Vec<SQLBaseError> = templater_violations
                .into_iter()
                .map(SQLBaseError::from)
                .collect();
            return ParsedString {
                tree: None,
                violations,
                templated_file: rendered.templated_file,
                filename: rendered.filename,
                source_str: rendered.source_str,
            };
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
            let (p, pvs) = Self::parse_tokens(tables, &token_list, &self.config, self.parse_errors);
            parsed = p;
            violations.extend(pvs.into_iter().map_into());
        } else {
            parsed = None;
        };

        ParsedString {
            tree: parsed,
            violations,
            templated_file: rendered.templated_file,
            filename: rendered.filename,
            source_str: rendered.source_str,
        }
    }

    fn parse_tokens(
        tables: &Tables,
        tokens: &[ErasedSegment],
        config: &FluffConfig,
        parse_errors: ParseErrors,
    ) -> (Option<ErasedSegment>, Vec<SQLParseError>) {
        let parser: Parser = config.into();
        let mut violations: Vec<SQLParseError> = Vec::new();

        let parsed = match parser.parse(tables, tokens) {
            Ok(parsed) => parsed,
            Err(error) => {
                violations.push(error);
                None
            }
        };

        if matches!(parse_errors, ParseErrors::Include)
            && let Some(parsed) = &parsed
        {
            let unparsables = parsed.recursive_crawl(
                &SyntaxSet::single(SyntaxKind::Unparsable),
                true,
                &SyntaxSet::EMPTY,
                true,
            );

            violations.extend(unparsables.into_iter().map(|segment| SQLParseError {
                description: "Unparsable section".into(),
                segment: segment.into(),
            }));
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
        log::debug!("LEXING RAW ({})", templated_file.name());
        // Get the lexer
        let lexer = dialect.lexer();
        // Lex the file and log any problems
        let (tokens, lex_vs) = lexer.lex(tables, templated_file);

        violations.extend(lex_vs);

        if tokens.is_empty() {
            return (None, violations);
        }

        (tokens.into(), violations)
    }

    /// Normalise newlines to unix-style line endings.
    fn normalise_newlines(string: &str) -> Cow<'_, str> {
        lazy_regex::regex!("\r\n|\r").replace_all(string, "\n")
    }

    pub fn config(&self) -> &FluffConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut FluffConfig {
        self.rules = OnceLock::new();
        &mut self.config
    }

    pub fn rules(&self) -> Result<&[ErasedRule], SQLFluffUserError> {
        if let Some(rules) = self.rules.get() {
            return Ok(rules);
        }
        let rulepack = self.get_rulepack()?;
        let _ = self.rules.set(rulepack.rules);
        Ok(self.rules.get().unwrap())
    }

    pub(crate) fn parse_errors(&self) -> ParseErrors {
        self.parse_errors
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use sqruff_lib_core::parser::segments::Tables;

    use crate::api::{Mode, ParseErrors, PathDiscoveryOptions, discover_paths};
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    fn postgres_all_rules_linter() -> Linter {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = postgres
rules = all
"#,
            None,
        );

        Linter::new(config, None, ParseErrors::Include).unwrap()
    }

    fn normalise_paths(paths: Vec<PathBuf>) -> Vec<String> {
        paths
            .into_iter()
            .map(|path| {
                let path = path.to_string_lossy().replace(['/', '\\'], ".");
                if let Some(index) = path.find("test.") {
                    path[index..].to_string()
                } else {
                    path
                }
            })
            .collect()
    }

    fn path_options() -> PathDiscoveryOptions<'static> {
        PathDiscoveryOptions {
            ignore_file_name: ".sqruffignore",
            ignore_non_existent_files: false,
            ignore_files: false,
            working_dir: std::env::current_dir().unwrap(),
            ignorer: None,
        }
    }

    fn temp_project(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("sqruff-{name}-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_linter_path_from_paths_dir() {
        // Test extracting paths from directories.
        let options = path_options();
        let paths = discover_paths(Path::new("test/fixtures/lexer"), &options).unwrap();
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
        let options = path_options();
        let paths =
            normalise_paths(discover_paths(Path::new("test/fixtures/linter"), &options).unwrap());
        assert!(paths.contains(&"test.fixtures.linter.passing.sql".to_string()));
        assert!(paths.contains(&"test.fixtures.linter.passing_cap_extension.SQL".to_string()));
        assert!(!paths.contains(&"test.fixtures.linter.discovery_file.txt".to_string()));
    }

    #[test]
    fn test_linter_path_from_paths_file() {
        let options = path_options();
        let paths = discover_paths(
            Path::new("test/fixtures/linter/indentation_errors.sql"),
            &options,
        )
        .unwrap();

        assert_eq!(
            normalise_paths(paths),
            &["test.fixtures.linter.indentation_errors.sql"]
        );
    }

    #[test]
    fn test_linter_path_from_paths_missing_returns_error() {
        let options = path_options();
        let err = discover_paths(
            Path::new("test/fixtures/linter/does_not_exist.sql"),
            &options,
        )
        .unwrap_err();

        assert!(err.value.contains("Specified path does not exist"));
    }

    #[test]
    fn test_linter_path_from_paths_prunes_ignored_directories() {
        let project = temp_project("ignored-dir");
        let ignored_dir = project.join("ignored").join("nested");
        fs::create_dir_all(&ignored_dir).unwrap();
        fs::write(project.join("regular.sql"), "SELECT 1;\n").unwrap();
        fs::write(ignored_dir.join("hidden.sql"), "SELECT bad FROM hidden;\n").unwrap();

        let ignorer = |path: &Path| path.file_name().is_some_and(|name| name == "ignored");
        let options = PathDiscoveryOptions {
            ignore_file_name: ".sqruffignore",
            ignore_non_existent_files: false,
            ignore_files: false,
            working_dir: std::env::current_dir().unwrap(),
            ignorer: Some(&ignorer),
        };
        let paths = discover_paths(&project, &options).unwrap();

        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("regular.sql"));

        fs::remove_dir_all(project).unwrap();
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
        let linter = Linter::new(
            FluffConfig::new(<_>::default(), None, None),
            None,
            ParseErrors::Suppress,
        )
        .unwrap();
        let tables = Tables::default();
        let parsed = linter.parse_string(&tables, "", None).unwrap();

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

        let linter = Linter::new(
            FluffConfig::new(<_>::default(), None, None),
            None,
            ParseErrors::Suppress,
        )
        .unwrap();
        let tables = Tables::default();
        let _parsed = linter.parse_string(&tables, &sql, None).unwrap();
    }

    #[test]
    fn test_normalise_newlines() {
        let in_str = "SELECT\r\n foo\n FROM \r \n\r bar;";
        let out_str = "SELECT\n foo\n FROM \n \n\n bar;";

        assert_eq!(Linter::normalise_newlines(in_str), out_str);
    }

    /// Regression test for https://github.com/quarylabs/sqruff/issues/2354
    /// When a templater fails (e.g., dbt/jinja can't find a project), the
    /// fallback should not produce false positive LT01 violations on template
    /// syntax like `{{ ref('stg_users') }}`.
    #[test]
    fn test_templater_error_skips_linting() {
        use crate::core::linter::common::RenderedFile;
        use sqruff_lib_core::errors::SQLTemplaterError;
        use sqruff_lib_core::templaters::TemplatedFile;

        let source =
            "SELECT *\nFROM {{ ref('stg_users') }}\nWHERE created_at > '{{ var(\"start_date\") }}'";
        let linter = Linter::new(
            FluffConfig::new(<_>::default(), None, None),
            None,
            ParseErrors::Suppress,
        )
        .unwrap();

        // Simulate a failed templater by creating a RenderedFile with
        // templater_violations.
        let rendered = RenderedFile {
            templated_file: TemplatedFile::new(
                source.to_string(),
                "test.sql".to_string(),
                None,
                None,
                None,
            )
            .unwrap(),
            templater_violations: vec![SQLTemplaterError::new(
                "Failed to template file: dbt project not found".to_string(),
            )],
            filename: "test.sql".to_string(),
            source_str: source.to_string(),
        };

        let result = linter.lint_rendered(rendered, Mode::Check).unwrap();
        let violations = result.violations();

        // Should have exactly 1 violation: the templater error.
        // Should NOT have any LT01 spacing violations.
        assert_eq!(violations.len(), 1);
        assert!(violations[0].desc().contains("Failed to template file"));
        assert!(
            !violations.iter().any(|v| v.rule_code() == "LT01"),
            "Should not have LT01 false positives on template syntax"
        );
    }

    #[test]
    fn test_postgres_case_else_concat_does_not_raise_lt01_and_fixes_cleanly() {
        let sql = r#"select case
      when a = 1 then 'one'
      when a = 2 then 'two'
  else 'other' || 's'
    end as b
from test;
"#;
        let expected = r#"select
    case
        when a = 1 then 'one'
        when a = 2 then 'two'
        else 'other' || 's'
    end as b
from test;
"#;

        let mut linter = postgres_all_rules_linter();
        let linted = linter.lint_string_wrapped(sql, Mode::Check).unwrap();
        let violations = linted.violations();

        assert!(
            !violations.iter().any(|v| v.rule_code() == "LT01"),
            "Expected no LT01 violations, got: {:?}",
            violations
                .iter()
                .map(|v| (v.rule_code(), v.desc().to_string()))
                .collect::<Vec<_>>()
        );
        assert!(
            violations.iter().all(|v| v.rule_code() == "LT02"),
            "Expected only LT02 violations, got: {:?}",
            violations
                .iter()
                .map(|v| (v.rule_code(), v.desc().to_string()))
                .collect::<Vec<_>>()
        );

        let fixed = postgres_all_rules_linter()
            .lint_string_wrapped(sql, Mode::Fix)
            .unwrap()
            .fix_string();

        assert_eq!(fixed, expected);
    }

    #[test]
    fn test_postgres_case_else_binary_operator_spacing_still_triggers_lt01() {
        let sql = r#"select case
      when a = 1 then 'one'
  else 1+2
    end as b
from test;
"#;
        let expected = r#"select
    case
        when a = 1 then 'one'
        else 1 + 2
    end as b
from test;
"#;

        let mut linter = postgres_all_rules_linter();
        let linted = linter.lint_string_wrapped(sql, Mode::Check).unwrap();
        let violations = linted.violations();

        assert!(
            violations.iter().any(|v| v.rule_code() == "LT01"),
            "Expected LT01 violations, got: {:?}",
            violations
                .iter()
                .map(|v| (v.rule_code(), v.desc().to_string()))
                .collect::<Vec<_>>()
        );

        let fixed = postgres_all_rules_linter()
            .lint_string_wrapped(sql, Mode::Fix)
            .unwrap()
            .fix_string();

        assert_eq!(fixed, expected);
    }
}
