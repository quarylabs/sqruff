use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::Formatter;
use crate::core::config::FluffConfig;
use crate::core::linter::common::{BatchRenderedResult, ParsedString, ParsedVariant, RenderedFile};
use crate::core::linter::discovery::paths_from_path;
use crate::core::linter::linted_file::LintedFile;
use crate::core::linter::linting_result::LintingResult;
use crate::core::rules::noqa::IgnoreMask;
use crate::core::rules::{ErasedRule, Exception, LintPhase, RulePack};
use crate::rules::get_ruleset;
use crate::templaters::{ProcessingMode, Templater, TemplaterKind};
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator as _};
use smol_str::{SmolStr, ToSmolStr};
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::errors::{
    SQLBaseError, SQLFluffUserError, SQLLexError, SQLLintError, SQLParseError, SQLTemplaterError,
};
use sqruff_lib_core::linter::compute_anchor_edit_info;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};
use sqruff_lib_core::templaters::TemplatedFile;

pub struct Linter {
    config: FluffConfig,
    formatter: Option<Arc<dyn Formatter>>,
    templater: &'static dyn Templater,
    rulepack: OnceLock<RulePack>,

    /// include_parse_errors is a flag to indicate whether to include parse errors in the output
    include_parse_errors: bool,
}

impl Linter {
    pub fn new(
        config: FluffConfig,
        formatter: Option<Arc<dyn Formatter>>,
        templater: Option<&'static dyn Templater>,
        include_parse_errors: bool,
    ) -> Result<Linter, String> {
        let templater: &'static dyn Templater = match templater {
            Some(templater) => templater,
            None => Linter::get_templater(&config)?,
        };
        Ok(Linter {
            config,
            formatter,
            templater,
            rulepack: OnceLock::new(),
            include_parse_errors,
        })
    }

    pub fn get_templater(config: &FluffConfig) -> Result<&'static dyn Templater, String> {
        config.templater_kind().map(TemplaterKind::templater)
    }

    /// Lint strings directly.
    pub fn lint_string_wrapped(
        &mut self,
        sql: &str,
        fix: bool,
    ) -> Result<LintedFile, SQLFluffUserError> {
        let filename = "<string input>".to_owned();
        self.lint_string(sql, Some(filename), fix)
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
        let rendered = self.render_string(sql, f_name.clone(), &self.config)?;

        Ok(self.parse_rendered(tables, rendered))
    }

    /// Lint a string.
    pub fn lint_string(
        &self,
        sql: &str,
        filename: Option<String>,
        fix: bool,
    ) -> Result<LintedFile, SQLFluffUserError> {
        let tables = Tables::default();
        let parsed = self.parse_string(&tables, sql, filename)?;

        // Lint the file and return the LintedFile
        self.lint_parsed(&tables, parsed, fix)
    }

    /// ignorer is an optional argument that takes in a function that returns a bool based on the
    /// path passed to it. If the function returns true, the path is ignored.
    pub fn lint_paths(
        &mut self,
        mut paths: Vec<PathBuf>,
        fix: bool,
        ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
    ) -> Result<LintingResult, SQLFluffUserError> {
        if paths.is_empty() {
            paths.push(std::env::current_dir().unwrap());
        }

        let mut expanded_paths = Vec::new();

        for path in paths {
            expanded_paths.extend(paths_from_path(
                path,
                None,
                None,
                None,
                None,
                self.config.sql_file_exts(),
                Some(ignorer),
            ));
        }

        let paths: Vec<String> = expanded_paths
            .into_iter()
            .filter(|path| {
                let should_ignore = ignorer(Path::new(path));
                if should_ignore {
                    log::debug!(
                        "Filtering out ignored file '{}' from final processing list",
                        path
                    );
                }
                !should_ignore
            })
            .collect_vec();

        let mut files = Vec::with_capacity(paths.len());

        match self.templater.processing_mode() {
            ProcessingMode::Parallel => {
                let results: Vec<_> = paths
                    .par_iter()
                    .map(|path| {
                        let rendered = self.render_file(path.clone());
                        self.lint_rendered(rendered, fix)
                    })
                    .collect();
                for result in results {
                    files.push(result?);
                }
            }
            ProcessingMode::Batch => {
                // Use batch processing for templaters that support it (e.g., dbt).
                // This allows sharing expensive initialization (manifest loading) across files.
                let batch_results = self.render_files_batch(&paths);
                for result in batch_results {
                    match result {
                        BatchRenderedResult::Rendered(rendered) => {
                            files.push(self.lint_rendered(rendered, fix)?);
                        }
                        BatchRenderedResult::Skipped { filename, reason } => {
                            if let Some(formatter) = &self.formatter {
                                formatter.dispatch_file_skip(&filename, &reason);
                            }
                        }
                    }
                }
            }
            ProcessingMode::Sequential => {
                for path in &paths {
                    let rendered = self.render_file(path.clone());
                    files.push(self.lint_rendered(rendered, fix)?);
                }
            }
        }

        Ok(LintingResult::new(files))
    }

    pub fn get_rulepack(&self) -> Result<RulePack, SQLFluffUserError> {
        let rs = get_ruleset();
        rs.get_rulepack(&self.config)
    }

    pub fn render_file(&self, fname: String) -> RenderedFile {
        let in_str = std::fs::read_to_string(&fname).unwrap();
        match self.render_string(&in_str, fname.clone(), &self.config) {
            Ok(rendered) => rendered,
            Err(err) => {
                log::error!("Failed to template file {}: {:?}", fname, err);
                let source_str = Self::normalise_newlines(&in_str).to_string();
                RenderedFile {
                    templated_file: TemplatedFile::new(
                        source_str.clone(),
                        fname.clone(),
                        None,
                        None,
                        None,
                    )
                    .expect("Creating raw TemplatedFile should not fail"),
                    alternate_templated_files: Vec::new(),
                    templater_violations: vec![SQLTemplaterError::new(format!(
                        "Failed to template file {fname}: {err}"
                    ))],
                    filename: fname,
                    source_str,
                }
            }
        }
    }

    /// Render multiple files in a batch using the templater's batch processing.
    ///
    /// This is more efficient for templaters like dbt that have expensive
    /// initialization (manifest loading) that can be shared across files.
    pub fn render_files_batch(&self, fnames: &[String]) -> Vec<BatchRenderedResult> {
        if fnames.is_empty() {
            return Vec::new();
        }

        // Check dialect before processing
        if let Some(_error) = self.config.verify_dialect_specified() {
            // Return error rendered files for all files
            return fnames
                .iter()
                .map(|fname| {
                    let source_str = std::fs::read_to_string(fname).unwrap_or_default();
                    BatchRenderedResult::Rendered(RenderedFile {
                        templated_file: TemplatedFile::new(
                            source_str.clone(),
                            fname.clone(),
                            None,
                            None,
                            None,
                        )
                        .expect("Creating raw TemplatedFile should not fail"),
                        alternate_templated_files: Vec::new(),
                        templater_violations: vec![],
                        filename: fname.clone(),
                        source_str,
                    })
                })
                .collect();
        }

        // Read all files and prepare for batch processing
        let files: Vec<(String, String)> = fnames
            .iter()
            .map(|fname| {
                let content = std::fs::read_to_string(fname).unwrap_or_default();
                let normalized = Self::normalise_newlines(&content).to_string();
                (normalized, fname.clone())
            })
            .collect();

        // Convert to slice of references for the process method
        let file_refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(content, fname)| (content.as_str(), fname.as_str()))
            .collect();

        // Process all files in batch
        let results =
            self.templater
                .process_with_variants(&file_refs, &self.config, &self.formatter);

        // Convert results to BatchRenderedResults, preserving order
        results
            .into_iter()
            .zip(files.iter())
            .map(|(result, (source_str, fname))| match result {
                Ok(mut templated_files) if !templated_files.is_empty() => {
                    BatchRenderedResult::Rendered(RenderedFile {
                        templated_file: templated_files.remove(0),
                        alternate_templated_files: templated_files,
                        templater_violations: vec![],
                        filename: fname.clone(),
                        source_str: source_str.clone(),
                    })
                }
                Ok(_) => BatchRenderedResult::Rendered(RenderedFile {
                    templated_file: TemplatedFile::new(
                        source_str.clone(),
                        fname.clone(),
                        None,
                        None,
                        None,
                    )
                    .expect("Creating raw TemplatedFile should not fail"),
                    alternate_templated_files: Vec::new(),
                    templater_violations: vec![SQLTemplaterError::new(format!(
                        "Templater returned no variants for file {fname}"
                    ))],
                    filename: fname.clone(),
                    source_str: source_str.clone(),
                }),
                Err(err) => {
                    let err_str = err.to_string();
                    if let Some(reason) = err_str.strip_prefix("SKIP:") {
                        return BatchRenderedResult::Skipped {
                            filename: fname.clone(),
                            reason: reason.to_string(),
                        };
                    }
                    log::error!("Failed to template file {}: {:?}", fname, err);
                    // Return a minimal RenderedFile with the templater error as a
                    // violation. This prevents linting the raw source (which contains
                    // template syntax like {{ }}) and producing false positive LT01
                    // spacing errors.
                    BatchRenderedResult::Rendered(RenderedFile {
                        templated_file: TemplatedFile::new(
                            source_str.clone(),
                            fname.clone(),
                            None,
                            None,
                            None,
                        )
                        .expect("Creating raw TemplatedFile should not fail"),
                        alternate_templated_files: Vec::new(),
                        templater_violations: vec![SQLTemplaterError::new(format!(
                            "Failed to template file {fname}: {err}"
                        ))],
                        filename: fname.clone(),
                        source_str: source_str.clone(),
                    })
                }
            })
            .collect()
    }

    pub fn lint_rendered(
        &self,
        rendered: RenderedFile,
        fix: bool,
    ) -> Result<LintedFile, SQLFluffUserError> {
        let tables = Tables::default();
        let parsed = self.parse_rendered(&tables, rendered);
        self.lint_parsed(&tables, parsed, fix)
    }

    pub fn lint_parsed(
        &self,
        tables: &Tables,
        parsed_string: ParsedString,
        fix: bool,
    ) -> Result<LintedFile, SQLFluffUserError> {
        let mut violations = parsed_string.violations;

        let (patches, ignore_mask, initial_linting_errors) = match parsed_string.tree {
            Some(erased_segment) => {
                let (tree, ignore_mask, initial_linting_errors) = self.lint_fix_parsed(
                    tables,
                    erased_segment,
                    &parsed_string.templated_file,
                    fix,
                )?;
                let patches = tree.iter_patches(&parsed_string.templated_file);
                (patches, ignore_mask, initial_linting_errors)
            }
            None => (Vec::new(), None, Vec::new()),
        };
        violations.extend(initial_linting_errors.into_iter().map_into());

        for alternate_variant in parsed_string.alternate_variants {
            violations.extend(alternate_variant.violations);
            if let Some(tree) = alternate_variant.tree {
                let (_, _, alternate_linting_errors) =
                    self.lint_fix_parsed(tables, tree, &alternate_variant.templated_file, fix)?;
                violations.extend(alternate_linting_errors.into_iter().map_into());
            }
        }

        // Filter violations with ignore mask
        if let Some(ignore_mask) = &ignore_mask {
            violations.retain(|violation| !ignore_mask.is_masked(violation, None, true));
        }

        // Surface warnings for `-- noqa:` directives that never masked anything,
        // when `warn_unused_ignores` is enabled.
        let warn_unused_ignores = self
            .config
            .get("warn_unused_ignores", "core")
            .as_bool()
            .unwrap_or(false);
        if warn_unused_ignores && let Some(ignore_mask) = &ignore_mask {
            violations.extend(ignore_mask.generate_warnings_for_unused());
        }

        let linted_file = LintedFile::new(
            parsed_string.filename,
            patches,
            parsed_string.templated_file,
            violations,
            ignore_mask,
        );

        if let Some(formatter) = &self.formatter {
            formatter.dispatch_file_violations(&linted_file);
        }

        Ok(linted_file)
    }

    pub fn lint_fix_parsed(
        &self,
        tables: &Tables,
        mut tree: ErasedSegment,
        templated_file: &TemplatedFile,
        fix: bool,
    ) -> Result<(ErasedSegment, Option<IgnoreMask>, Vec<SQLLintError>), SQLFluffUserError> {
        let mut initial_violations = Vec::new();
        let phases: &[_] = if fix {
            &[LintPhase::Main, LintPhase::Post]
        } else {
            &[LintPhase::Main]
        };
        let mut previous_versions: HashSet<(SmolStr, bool)> =
            [(tree.raw().to_smolstr(), false)].into_iter().collect();

        // If we are fixing then we want to loop up to the runaway_limit, otherwise just
        // once for linting.
        let loop_limit = if fix { 10 } else { 1 };
        // Look for comment segments which might indicate lines to ignore.
        let (ignore_mask, violations): (Option<IgnoreMask>, Vec<SQLBaseError>) = {
            let disable_noqa = self
                .config
                .get("disable_noqa", "core")
                .as_bool()
                .unwrap_or(false);
            let disable_noqa_except = self
                .config
                .get("disable_noqa_except", "core")
                .as_string()
                .filter(|value| !value.is_empty());
            if disable_noqa && disable_noqa_except.is_none() {
                (None, Vec::new())
            } else {
                let reference_map = Self::allowed_rule_ref_map(
                    self.rulepack()?.reference_map(),
                    disable_noqa_except,
                );
                let (ignore_mask, errors) = IgnoreMask::from_tree(&tree, &reference_map);
                (Some(ignore_mask), errors)
            }
        };

        initial_violations.extend(violations.into_iter().map_into());

        // Whether to suppress lint results whose anchor falls in a
        // template-generated (non-literal) region. Mirrors SQLFluff's
        // `remove_templated_errors`. Default true.
        let ignore_templated_areas = self
            .config
            .get("ignore_templated_areas", "core")
            .as_bool()
            .unwrap_or(true);

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
                    if fix && !is_first_linter_pass && !rule.is_fix_compatible() {
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
                            // Suppress results anchored in template-generated
                            // regions unless the rule targets templated areas,
                            // matching SQLFluff's `remove_templated_errors`.
                            let suppress_templated_violation = ignore_templated_areas
                                && !rule.targets_templated()
                                && result.anchor_in_templated_section();

                            if ignore_mask.as_ref().is_none_or(|ignore_mask| {
                                !ignore_mask.is_masked(&result, rule.into(), is_first_linter_pass)
                            }) {
                                if !suppress_templated_violation
                                    || (fix && !result.fixes.is_empty())
                                {
                                    compute_anchor_edit_info(
                                        &mut anchor_info,
                                        std::mem::take(&mut result.fixes),
                                    );
                                }

                                if is_first_linter_pass && !suppress_templated_violation {
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

                    if fix && !anchor_info.is_empty() {
                        let parser: Parser = (&self.config).into();
                        let mut parse_context = (&parser).into();
                        let (new_tree, _, _, valid) =
                            tree.apply_fixes(&mut anchor_info, &mut parse_context);
                        if !valid {
                            log::warn!(
                                "Fixes for {} not applied, as they would result in an unparsable \
                                 file. Please report this as a bug with a minimal query which \
                                 demonstrates this warning.",
                                rule.code(),
                            );
                            continue;
                        }
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

                if fix && !changed {
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
        let sql = Self::normalise_newlines(sql);

        if let Some(error) = config.verify_dialect_specified() {
            return Err(error);
        }

        let templater_violations = vec![];
        let mut results = self.templater.process_with_variants(
            &[(sql.as_ref(), filename.as_str())],
            config,
            &self.formatter,
        );

        match results.pop() {
            Some(Ok(mut templated_files)) if !templated_files.is_empty() => Ok(RenderedFile {
                templated_file: templated_files.remove(0),
                alternate_templated_files: templated_files,
                templater_violations,
                filename,
                source_str: sql.to_string(),
            }),
            Some(Err(err)) => Err(SQLFluffUserError::new(format!(
                "Failed to template file {filename} with error {err:?}"
            ))),
            Some(Ok(_)) => Err(SQLFluffUserError::new(format!(
                "Templater returned no variants for file {filename}"
            ))),
            None => Err(SQLFluffUserError::new(format!(
                "Templater returned no results for file {filename}"
            ))),
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
                alternate_variants: Vec::new(),
            };
        }

        let alternate_variants = rendered
            .alternate_templated_files
            .into_iter()
            .map(|templated_file| self.parse_templated_variant(tables, templated_file))
            .collect();
        let primary = self.parse_templated_variant(tables, rendered.templated_file);

        ParsedString {
            tree: primary.tree,
            violations: primary.violations,
            templated_file: primary.templated_file,
            filename: rendered.filename,
            source_str: rendered.source_str,
            alternate_variants,
        }
    }

    fn parse_templated_variant(
        &self,
        tables: &Tables,
        templated_file: TemplatedFile,
    ) -> ParsedVariant {
        let mut violations = Vec::new();
        let tokens = if templated_file.is_templated() {
            let (t, lvs) =
                Self::lex_templated_file(tables, templated_file.clone(), &self.config.dialect);
            if !lvs.is_empty() {
                unimplemented!("violations.extend(lvs);")
            }
            t
        } else {
            None
        };

        let parsed: Option<ErasedSegment>;
        if let Some(token_list) = tokens {
            let (p, pvs) =
                Self::parse_tokens(tables, &token_list, &self.config, self.include_parse_errors);
            parsed = p;
            violations.extend(pvs.into_iter().map_into());
        } else {
            parsed = None;
        };

        ParsedVariant {
            tree: parsed,
            violations,
            templated_file,
        }
    }

    fn parse_tokens(
        tables: &Tables,
        tokens: &[ErasedSegment],
        config: &FluffConfig,
        include_parse_errors: bool,
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

        if include_parse_errors && let Some(parsed) = &parsed {
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
        self.rulepack = OnceLock::new();
        &mut self.config
    }

    fn rulepack(&self) -> Result<&RulePack, SQLFluffUserError> {
        if let Some(rulepack) = self.rulepack.get() {
            return Ok(rulepack);
        }
        let _ = self.rulepack.set(self.get_rulepack()?);
        Ok(self.rulepack.get().unwrap())
    }

    pub fn rules(&self) -> Result<&[ErasedRule], SQLFluffUserError> {
        Ok(&self.rulepack()?.rules)
    }

    fn allowed_rule_ref_map(
        reference_map: &HashMap<&'static str, HashSet<&'static str>>,
        disable_noqa_except: Option<&str>,
    ) -> HashMap<&'static str, HashSet<&'static str>> {
        let Some(disable_noqa_except) = disable_noqa_except else {
            return reference_map.clone();
        };

        let mut output_map = reference_map.clone();
        for special_rule in ["PRS", "LXR", "TMP"] {
            output_map.insert(special_rule, HashSet::from([special_rule]));
        }

        let mut allowed_rules = HashSet::new();
        for rule_ref in disable_noqa_except.split(',').map(str::trim) {
            let pattern = glob::Pattern::new(rule_ref).ok();
            for (reference, codes) in &output_map {
                if pattern
                    .as_ref()
                    .is_some_and(|pattern| pattern.matches(reference))
                {
                    allowed_rules.extend(codes.iter().copied());
                }
            }
        }

        output_map
            .into_iter()
            .map(|(reference, codes)| {
                let codes = codes.intersection(&allowed_rules).copied().collect();
                (reference, codes)
            })
            .collect()
    }

    pub fn formatter(&self) -> Option<&Arc<dyn Formatter>> {
        self.formatter.as_ref()
    }

    pub fn formatter_mut(&mut self) -> Option<&mut Arc<dyn Formatter>> {
        self.formatter.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use hashbrown::HashMap;
    use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
    use sqruff_lib_core::lint_fix::LintFix;
    use sqruff_lib_core::linter::compute_anchor_edit_info;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::segments::{SegmentBuilder, Tables};

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

        Linter::new(config, None, None, true).unwrap()
    }

    // test__linter__skip_large_bytes
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
            None,
            false,
        )
        .unwrap();
        let tables = Tables::default();
        let parsed = linter.parse_string(&tables, "", None).unwrap();

        assert!(parsed.violations.is_empty());
    }

    #[test]
    fn test_structural_fix_that_breaks_parsing_is_invalid() {
        let config = FluffConfig::new(<_>::default(), None, None);
        let linter = Linter::new(config, None, None, false).unwrap();
        let tables = Tables::default();
        let tree = linter
            .parse_string(&tables, "SELECT 1 FROM a", None)
            .unwrap()
            .tree
            .unwrap();
        let numeric = tree
            .recursive_crawl(
                &SyntaxSet::single(SyntaxKind::NumericLiteral),
                true,
                &SyntaxSet::EMPTY,
                true,
            )
            .into_iter()
            .next()
            .unwrap();
        let replacement = vec![
            SegmentBuilder::token(tables.next_id(), "1", SyntaxKind::NumericLiteral).finish(),
            SegmentBuilder::whitespace(tables.next_id(), " "),
            SegmentBuilder::token(tables.next_id(), "1", SyntaxKind::NumericLiteral).finish(),
            SegmentBuilder::whitespace(tables.next_id(), " "),
            SegmentBuilder::token(tables.next_id(), "1", SyntaxKind::NumericLiteral).finish(),
        ];
        let mut fixes = HashMap::new();
        compute_anchor_edit_info(
            &mut fixes,
            vec![LintFix::replace(numeric, replacement, None)],
        );
        let parser: Parser = linter.config().into();
        let mut parse_context = (&parser).into();

        let (_, _, _, valid) = tree.apply_fixes(&mut fixes, &mut parse_context);

        assert!(!valid);
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
            None,
            false,
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
            None,
            false,
        )
        .unwrap();

        // Simulate a failed templater by creating a RenderedFile with
        // templater_violations (this is what render_files_batch does when
        // the dbt/jinja templater fails).
        let rendered = RenderedFile {
            templated_file: TemplatedFile::new(
                source.to_string(),
                "test.sql".to_string(),
                None,
                None,
                None,
            )
            .unwrap(),
            alternate_templated_files: Vec::new(),
            templater_violations: vec![SQLTemplaterError::new(
                "Failed to template file: dbt project not found".to_string(),
            )],
            filename: "test.sql".to_string(),
            source_str: source.to_string(),
        };

        let result = linter.lint_rendered(rendered, false).unwrap();
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
        let linted = linter.lint_string_wrapped(sql, false).unwrap();
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
            .lint_string_wrapped(sql, true)
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
        let linted = linter.lint_string_wrapped(sql, false).unwrap();
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
            .lint_string_wrapped(sql, true)
            .unwrap()
            .fix_string();

        assert_eq!(fixed, expected);
    }
}
