use crate::utils::reflow::config::ReflowConfig;
use ahash::AHashMap;
use itertools::Itertools;
use regex::Regex;
use serde::Deserialize;
use serde::de::{self, Deserializer};
use serde_json::Value as JsonValue;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_dialects::kind_to_dialect;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Default)]
struct ConfigLayer {
    core: Option<CoreConfig>,
    indentation: Option<IndentationConfig>,
    layout: Option<LayoutConfig>,
    templater: Option<TemplaterConfig>,
    rules: Option<RulesConfig>,
}

impl ConfigLayer {
    fn is_empty(&self) -> bool {
        self.core.is_none()
            && self.indentation.is_none()
            && self.layout.is_none()
            && self.templater.is_none()
            && self.rules.is_none()
    }
}

struct ConfigLayerBuilder {
    layer: ConfigLayer,
}

impl ConfigLayerBuilder {
    fn new() -> Self {
        Self {
            layer: ConfigLayer::default(),
        }
    }

    fn into_layer(self) -> ConfigLayer {
        self.layer
    }

    fn apply_section(
        &mut self,
        section: &str,
        values: Vec<(String, String)>,
        config_path: Option<&Path>,
    ) -> Result<(), String> {
        let section = canonical_section_name(section);
        let Some(path) = section_path(&section) else {
            return Ok(());
        };
        let normalized = normalize_values(values, config_path)?;

        let (root, rest) = path.split_first().unwrap();
        match root.as_str() {
            "core" => {
                let core = self.layer.core.get_or_insert_with(CoreConfig::default);
                if rest.is_empty() {
                    apply_core_config(core, normalized)?;
                }
            }
            "indentation" => {
                let indentation = self
                    .layer
                    .indentation
                    .get_or_insert_with(IndentationConfig::default);
                if rest.is_empty() {
                    apply_indentation_config(indentation, normalized)?;
                }
            }
            "layout" => {
                let layout = self.layer.layout.get_or_insert_with(LayoutConfig::default);
                apply_layout_section(layout, rest, normalized)?;
            }
            "templater" => {
                let templater = self
                    .layer
                    .templater
                    .get_or_insert_with(TemplaterConfig::default);
                apply_templater_section(templater, rest, normalized)?;
            }
            "rules" => {
                let rules = self.layer.rules.get_or_insert_with(RulesConfig::default);
                apply_rules_section(rules, rest, normalized)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn apply_pair(
        &mut self,
        section: &str,
        key: &str,
        value: &str,
        config_path: Option<&Path>,
    ) -> Result<(), String> {
        self.apply_section(
            section,
            vec![(key.to_string(), value.to_string())],
            config_path,
        )
    }
}

fn normalize_values(
    values: Vec<(String, String)>,
    config_path: Option<&Path>,
) -> Result<Vec<(String, String)>, String> {
    let mut normalized = Vec::with_capacity(values.len());
    for (key, value) in values {
        let value = normalize_raw_value(config_path, &key, &value)?;
        normalized.push((key, value));
    }
    Ok(normalized)
}

fn apply_string_map(target: &mut AHashMap<String, String>, values: Vec<(String, String)>) {
    for (key, value) in values {
        target.insert(key, value);
    }
}

fn canonical_section_name(section: &str) -> String {
    if section == "sqruff" {
        "sqlfluff".to_string()
    } else if let Some(rest) = section.strip_prefix("sqruff:") {
        format!("sqlfluff:{rest}")
    } else {
        section.to_string()
    }
}

fn section_path(section: &str) -> Option<Vec<String>> {
    if section == "sqlfluff" || section == "sqruff" {
        return Some(vec!["core".to_string()]);
    }
    let rest = section
        .strip_prefix("sqlfluff:")
        .or_else(|| section.strip_prefix("sqruff:"))?;
    if rest.is_empty() {
        return Some(vec!["core".to_string()]);
    }
    Some(rest.split(':').map(ToOwned::to_owned).collect())
}

fn normalize_raw_value(
    config_path: Option<&Path>,
    key: &str,
    value: &str,
) -> Result<String, String> {
    let name_lowercase = key.to_lowercase();
    if name_lowercase == "load_macros_from_path" {
        return Err("load_macros_from_path is not supported".to_string());
    }
    if name_lowercase.ends_with("_path") || name_lowercase.ends_with("_dir") {
        return normalize_path_value(config_path, value);
    }

    Ok(value.to_string())
}

fn normalize_path_value(config_path: Option<&Path>, value: &str) -> Result<String, String> {
    let parts: Vec<&str> = value
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect();

    if parts.is_empty() {
        return Ok(value.to_string());
    }

    let mut normalized = Vec::with_capacity(parts.len());
    for part in parts {
        if is_non_string_scalar(part) {
            return Err("Path values must be strings".to_string());
        }
        normalized.push(normalize_single_path_value(config_path, part)?);
    }

    if normalized.len() == 1 {
        Ok(normalized.pop().unwrap_or_default())
    } else {
        Ok(normalized.join(","))
    }
}

fn normalize_single_path_value(config_path: Option<&Path>, value: &str) -> Result<String, String> {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return Ok(value.to_string());
    }
    let config_path = config_path
        .and_then(|path| path.parent())
        .ok_or_else(|| "Relative paths require a config file path".to_string())?;
    let current_dir = std::env::current_dir().map_err(|err| err.to_string())?;
    let config_path =
        std::path::absolute(current_dir.join(config_path)).map_err(|err| err.to_string())?;
    let path = config_path.join(path);
    Ok(path.to_string_lossy().into_owned())
}

fn is_non_string_scalar(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true")
        || trimmed.eq_ignore_ascii_case("false")
        || trimmed.eq_ignore_ascii_case("none")
    {
        return true;
    }
    trimmed.parse::<i32>().is_ok() || trimmed.parse::<f64>().is_ok()
}

trait ConfigFormat {
    fn apply(
        &self,
        content: &str,
        config_path: Option<&Path>,
        builder: &mut ConfigLayerBuilder,
    ) -> Result<(), String>;
}

struct IniFormat;

impl ConfigFormat for IniFormat {
    fn apply(
        &self,
        content: &str,
        config_path: Option<&Path>,
        builder: &mut ConfigLayerBuilder,
    ) -> Result<(), String> {
        apply_ini_content(builder, content, config_path)
    }
}

fn normalize_ini_content(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn apply_ini_content(
    builder: &mut ConfigLayerBuilder,
    content: &str,
    config_path: Option<&Path>,
) -> Result<(), String> {
    let normalized = normalize_ini_content(content);
    let mut current_section: Option<String> = None;

    for line in normalized.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            let end = trimmed
                .find(']')
                .ok_or_else(|| "INI syntax error: section missing ']'".to_string())?;
            let section = trimmed[1..end].trim();
            if section.is_empty() {
                return Err("INI syntax error: empty section name".to_string());
            }
            current_section = Some(section.to_string());
            continue;
        }

        let (key, value) = trimmed
            .split_once('=')
            .ok_or_else(|| "INI syntax error: variable assignment missing '='".to_string())?;
        let key = key.trim();
        if key.is_empty() {
            return Err("INI syntax error: missing key".to_string());
        }
        let value = value.trim();

        let section = current_section.as_deref().unwrap_or("");
        builder.apply_pair(section, key, value, config_path)?;
    }

    Ok(())
}

fn apply_core_config(core: &mut CoreConfig, values: Vec<(String, String)>) -> Result<(), String> {
    let mut rules_alias = None;
    let mut exclude_rules_alias = None;

    for (key, value) in values {
        match key.as_str() {
            "dialect" => core.dialect = parse_option_string_none_value(&value),
            "templater" => core.templater = parse_option_string_none_value(&value),
            "nocolor" => core.nocolor = parse_boolish(&value),
            "verbose" => core.verbose = parse_option_i32_value(&value),
            "output_line_length" => core.output_line_length = parse_option_i32_value(&value),
            "runaway_limit" => core.runaway_limit = parse_option_i32_value(&value),
            "ignore" => core.ignore = parse_optional_comma_list_value(&value),
            "warnings" => core.warnings = parse_optional_comma_list_value(&value),
            "warn_unused_ignores" => core.warn_unused_ignores = parse_boolish(&value),
            "ignore_templated_areas" => core.ignore_templated_areas = parse_boolish(&value),
            "encoding" => core.encoding = parse_option_string_none_value(&value),
            "disable_noqa" => core.disable_noqa = parse_boolish(&value),
            "sql_file_exts" => core.sql_file_exts = split_comma_list(&value),
            "fix_even_unparsable" => core.fix_even_unparsable = parse_boolish(&value),
            "large_file_skip_char_limit" => {
                core.large_file_skip_char_limit = parse_option_i32_value(&value)
            }
            "large_file_skip_byte_limit" => {
                core.large_file_skip_byte_limit = parse_option_i32_value(&value)
            }
            "processes" => core.processes = parse_option_i32_value(&value),
            "max_line_length" => core.max_line_length = parse_option_i32_value(&value),
            "rule_allowlist" => core.rule_allowlist = parse_optional_comma_list_value(&value),
            "rule_denylist" => core.rule_denylist = split_comma_list(&value),
            "rules" => rules_alias = Some(value),
            "exclude_rules" => exclude_rules_alias = Some(value),
            _ => {}
        }
    }

    if let Some(value) = rules_alias {
        core.rule_allowlist = parse_optional_comma_list_value(&value);
    }

    if let Some(value) = exclude_rules_alias {
        core.rule_denylist = split_comma_list(&value);
    }

    Ok(())
}

fn apply_indentation_config(
    indentation: &mut IndentationConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "indent_unit" => indentation.indent_unit = parse_option_string_none_value(&value),
            "tab_space_size" => indentation.tab_space_size = parse_option_i32_value(&value),
            "hanging_indents" => indentation.hanging_indents = parse_boolish(&value),
            "indented_joins" => indentation.indented_joins = parse_boolish(&value),
            "indented_ctes" => indentation.indented_ctes = parse_boolish(&value),
            "indented_using_on" => indentation.indented_using_on = parse_boolish(&value),
            "indented_on_contents" => indentation.indented_on_contents = parse_boolish(&value),
            "indented_then" => indentation.indented_then = parse_boolish(&value),
            "indented_then_contents" => indentation.indented_then_contents = parse_boolish(&value),
            "indented_joins_on" => indentation.indented_joins_on = parse_boolish(&value),
            "allow_implicit_indents" => indentation.allow_implicit_indents = parse_boolish(&value),
            "template_blocks_indent" => indentation.template_blocks_indent = parse_boolish(&value),
            "skip_indentation_in" => {
                indentation.skip_indentation_in = parse_optional_comma_list_value(&value)
            }
            "trailing_comments" => {
                indentation.trailing_comments = parse_option_string_none_value(&value)
            }
            _ => {}
        }
    }

    Ok(())
}

fn apply_layout_section(
    layout: &mut LayoutConfig,
    rest: &[String],
    values: Vec<(String, String)>,
) -> Result<(), String> {
    if rest.len() == 2 && rest[0] == "type" {
        let entry = layout.types.entry(rest[1].clone()).or_default();
        apply_layout_type_config(entry, values)?;
    }
    Ok(())
}

fn apply_layout_type_config(
    config: &mut LayoutTypeConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "spacing_before" => config.spacing_before = parse_option_string_none_value(&value),
            "spacing_after" => config.spacing_after = parse_option_string_none_value(&value),
            "spacing_within" => config.spacing_within = parse_option_string_none_value(&value),
            "line_position" => config.line_position = parse_option_string_none_value(&value),
            "align_within" => config.align_within = parse_option_string_none_value(&value),
            "align_scope" => config.align_scope = parse_option_string_none_value(&value),
            _ => {}
        }
    }

    Ok(())
}

fn apply_templater_section(
    templater: &mut TemplaterConfig,
    rest: &[String],
    values: Vec<(String, String)>,
) -> Result<(), String> {
    if rest.is_empty() {
        return apply_templater_root_config(templater, values);
    }

    if rest.len() == 1 {
        return match rest[0].as_str() {
            "jinja" => apply_jinja_templater_config(&mut templater.jinja, values),
            "dbt" => apply_dbt_templater_config(&mut templater.dbt, values),
            "python" => Ok(()),
            "placeholder" => apply_placeholder_templater_config(&mut templater.placeholder, values),
            _ => Ok(()),
        };
    }

    if rest.len() == 2 && rest[1] == "context" {
        match rest[0].as_str() {
            "jinja" => apply_string_map(&mut templater.jinja.context, values),
            "dbt" => apply_string_map(&mut templater.dbt.context, values),
            "python" => apply_string_map(&mut templater.python.context, values),
            _ => {}
        }
    }

    Ok(())
}

fn apply_templater_root_config(
    templater: &mut TemplaterConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key.as_str() == "unwrap_wrapped_queries" {
            templater.unwrap_wrapped_queries = parse_boolish(&value);
        }
    }

    Ok(())
}

fn apply_jinja_templater_config(
    config: &mut JinjaTemplaterConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "templater_paths" => config.templater_paths = split_comma_list(&value),
            "loader_search_path" => config.loader_search_path = split_comma_list(&value),
            "apply_dbt_builtins" => config.apply_dbt_builtins = parse_boolish(&value),
            "ignore_templating" => config.ignore_templating = parse_boolish(&value),
            "library_paths" => config.library_paths = split_comma_list(&value),
            _ => {}
        }
    }

    Ok(())
}

fn apply_dbt_templater_config(
    config: &mut DbtTemplaterConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "profiles_dir" => config.profiles_dir = parse_option_string_none_value(&value),
            "project_dir" => config.project_dir = parse_option_string_none_value(&value),
            _ => {}
        }
    }

    Ok(())
}

fn apply_placeholder_templater_config(
    config: &mut PlaceholderTemplaterConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "param_regex" => config.param_regex = parse_option_string_none_value(&value),
            "param_style" => config.param_style = parse_option_string_none_value(&value),
            _ => {
                config
                    .replacements
                    .insert(key, PlaceholderValue::String(value));
            }
        }
    }

    Ok(())
}

fn apply_rules_section(
    rules: &mut RulesConfig,
    rest: &[String],
    values: Vec<(String, String)>,
) -> Result<(), String> {
    if rest.is_empty() {
        return apply_rules_root_config(rules, values);
    }
    if rest.len() != 1 {
        return Ok(());
    }

    match rest[0].as_str() {
        "aliasing.table" => apply_aliasing_rule_config(&mut rules.aliasing_table, values),
        "aliasing.column" => apply_aliasing_rule_config(&mut rules.aliasing_column, values),
        "aliasing.length" => apply_aliasing_length_rule_config(&mut rules.aliasing_length, values),
        "aliasing.forbid" => apply_aliasing_forbid_rule_config(&mut rules.aliasing_forbid, values),
        "ambiguous.join" => apply_ambiguous_join_rule_config(&mut rules.ambiguous_join, values),
        "ambiguous.column_references" => apply_ambiguous_column_references_rule_config(
            &mut rules.ambiguous_column_references,
            values,
        ),
        "capitalisation.keywords" => {
            apply_capitalisation_keywords_rule_config(&mut rules.capitalisation_keywords, values)
        }
        "capitalisation.identifiers" => apply_capitalisation_identifiers_rule_config(
            &mut rules.capitalisation_identifiers,
            values,
        ),
        "capitalisation.functions" => {
            apply_capitalisation_functions_rule_config(&mut rules.capitalisation_functions, values)
        }
        "capitalisation.literals" => {
            apply_capitalisation_literals_rule_config(&mut rules.capitalisation_literals, values)
        }
        "capitalisation.types" => {
            apply_capitalisation_types_rule_config(&mut rules.capitalisation_types, values)
        }
        "convention.select_trailing_comma" => apply_convention_select_trailing_comma_rule_config(
            &mut rules.convention_select_trailing_comma,
            values,
        ),
        "convention.count_rows" => {
            apply_convention_count_rows_rule_config(&mut rules.convention_count_rows, values)
        }
        "convention.terminator" => {
            apply_convention_terminator_rule_config(&mut rules.convention_terminator, values)
        }
        "convention.blocked_words" => {
            apply_convention_blocked_words_rule_config(&mut rules.convention_blocked_words, values)
        }
        "convention.quoted_literals" => apply_convention_quoted_literals_rule_config(
            &mut rules.convention_quoted_literals,
            values,
        ),
        "convention.casting_style" => {
            apply_convention_casting_style_rule_config(&mut rules.convention_casting_style, values)
        }
        "convention.not_equal" => {
            apply_convention_not_equal_rule_config(&mut rules.convention_not_equal, values)
        }
        "references.from" => apply_references_from_rule_config(&mut rules.references_from, values),
        "references.qualification" => {
            apply_references_qualification_rule_config(&mut rules.references_qualification, values)
        }
        "references.consistent" => {
            apply_references_consistent_rule_config(&mut rules.references_consistent, values)
        }
        "references.keywords" => {
            apply_references_keywords_rule_config(&mut rules.references_keywords, values)
        }
        "references.special_chars" => {
            apply_references_special_chars_rule_config(&mut rules.references_special_chars, values)
        }
        "references.quoting" => {
            apply_references_quoting_rule_config(&mut rules.references_quoting, values)
        }
        "layout.long_lines" => {
            apply_layout_long_lines_rule_config(&mut rules.layout_long_lines, values)
        }
        "layout.select_targets" => {
            apply_layout_select_targets_rule_config(&mut rules.layout_select_targets, values)
        }
        "layout.newlines" => apply_layout_newlines_rule_config(&mut rules.layout_newlines, values),
        "structure.subquery" => {
            apply_structure_subquery_rule_config(&mut rules.structure_subquery, values)
        }
        "structure.join_condition_order" => apply_structure_join_condition_order_rule_config(
            &mut rules.structure_join_condition_order,
            values,
        ),
        _ => Ok(()),
    }
}

fn apply_rules_root_config(
    rules: &mut RulesConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "allow_scalar" => rules.allow_scalar = parse_boolish_value(&value)?,
            "single_table_references" => rules.single_table_references = value,
            "unquoted_identifiers_policy" => rules.unquoted_identifiers_policy = value,
            _ => {}
        }
    }

    Ok(())
}

fn apply_aliasing_rule_config(
    config: &mut AliasingRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "aliasing" {
            config.aliasing = value;
        }
    }
    Ok(())
}

fn apply_aliasing_length_rule_config(
    config: &mut AliasingLengthRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "min_alias_length" => config.min_alias_length = parse_option_usize_value(&value),
            "max_alias_length" => config.max_alias_length = parse_option_usize_value(&value),
            _ => {}
        }
    }
    Ok(())
}

fn apply_aliasing_forbid_rule_config(
    config: &mut AliasingForbidRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "force_enable" {
            config.force_enable = parse_boolish_value(&value)?;
        }
    }
    Ok(())
}

fn apply_ambiguous_join_rule_config(
    config: &mut AmbiguousJoinRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "fully_qualify_join_types" {
            config.fully_qualify_join_types = value;
        }
    }
    Ok(())
}

fn apply_ambiguous_column_references_rule_config(
    config: &mut AmbiguousColumnReferencesRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "group_by_and_order_by_style" {
            config.group_by_and_order_by_style = value;
        }
    }
    Ok(())
}

fn apply_capitalisation_keywords_rule_config(
    config: &mut CapitalisationKeywordsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "capitalisation_policy" => config.capitalisation_policy = value,
            "ignore_words" => config.ignore_words = split_comma_list(&value),
            "ignore_words_regex" => config.ignore_words_regex = parse_regex_list_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_capitalisation_identifiers_rule_config(
    config: &mut CapitalisationIdentifiersRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "extended_capitalisation_policy" => config.extended_capitalisation_policy = value,
            "ignore_words" => config.ignore_words = split_comma_list(&value),
            "ignore_words_regex" => config.ignore_words_regex = parse_regex_list_value(&value)?,
            "unquoted_identifiers_policy" => {
                config.unquoted_identifiers_policy = parse_option_string_none_value(&value)
            }
            _ => {}
        }
    }
    Ok(())
}

fn apply_capitalisation_functions_rule_config(
    config: &mut CapitalisationFunctionsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "extended_capitalisation_policy" => config.extended_capitalisation_policy = value,
            "ignore_words" => config.ignore_words = split_comma_list(&value),
            "ignore_words_regex" => config.ignore_words_regex = parse_regex_list_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_capitalisation_literals_rule_config(
    config: &mut CapitalisationLiteralsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "capitalisation_policy" => config.capitalisation_policy = value,
            "ignore_words" => config.ignore_words = split_comma_list(&value),
            "ignore_words_regex" => config.ignore_words_regex = parse_regex_list_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_capitalisation_types_rule_config(
    config: &mut CapitalisationTypesRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "extended_capitalisation_policy" {
            config.extended_capitalisation_policy = value;
        }
    }
    Ok(())
}

fn apply_convention_select_trailing_comma_rule_config(
    config: &mut ConventionSelectTrailingCommaRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "select_clause_trailing_comma" {
            config.select_clause_trailing_comma = value;
        }
    }
    Ok(())
}

fn apply_convention_count_rows_rule_config(
    config: &mut ConventionCountRowsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "prefer_count_1" => config.prefer_count_1 = parse_boolish_value(&value)?,
            "prefer_count_0" => config.prefer_count_0 = parse_boolish_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_convention_terminator_rule_config(
    config: &mut ConventionTerminatorRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "multiline_newline" => config.multiline_newline = parse_boolish_value(&value)?,
            "require_final_semicolon" => {
                config.require_final_semicolon = parse_boolish_value(&value)?
            }
            _ => {}
        }
    }
    Ok(())
}

fn apply_convention_blocked_words_rule_config(
    config: &mut ConventionBlockedWordsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "blocked_words" => config.blocked_words = split_comma_list(&value),
            "blocked_regex" => config.blocked_regex = parse_regex_list_value(&value)?,
            "match_source" => config.match_source = parse_boolish_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_convention_quoted_literals_rule_config(
    config: &mut ConventionQuotedLiteralsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "preferred_quoted_literal_style" => config.preferred_quoted_literal_style = value,
            "force_enable" => config.force_enable = parse_boolish_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_convention_casting_style_rule_config(
    config: &mut ConventionCastingStyleRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "preferred_type_casting_style" {
            config.preferred_type_casting_style = value;
        }
    }
    Ok(())
}

fn apply_convention_not_equal_rule_config(
    config: &mut ConventionNotEqualRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "preferred_not_equal_style" {
            config.preferred_not_equal_style = value;
        }
    }
    Ok(())
}

fn apply_references_from_rule_config(
    config: &mut ReferencesFromRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "force_enable" {
            config.force_enable = parse_boolish_value(&value)?;
        }
    }
    Ok(())
}

fn apply_references_qualification_rule_config(
    config: &mut ReferencesQualificationRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "ignore_words" => config.ignore_words = split_comma_list(&value),
            "ignore_words_regex" => config.ignore_words_regex = parse_regex_list_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_references_consistent_rule_config(
    config: &mut ReferencesConsistentRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "single_table_references" => {
                config.single_table_references = parse_option_string_none_value(&value)
            }
            "force_enable" => config.force_enable = parse_boolish_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_references_keywords_rule_config(
    config: &mut ReferencesKeywordsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "unquoted_identifiers_policy" => config.unquoted_identifiers_policy = value,
            "quoted_identifiers_policy" => {
                config.quoted_identifiers_policy = parse_option_string_none_value(&value)
            }
            "ignore_words" => config.ignore_words = split_comma_list(&value),
            "ignore_words_regex" => config.ignore_words_regex = parse_regex_list_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_references_special_chars_rule_config(
    config: &mut ReferencesSpecialCharsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "quoted_identifiers_policy" => config.quoted_identifiers_policy = value,
            "unquoted_identifiers_policy" => config.unquoted_identifiers_policy = value,
            "allow_space_in_identifier" => {
                config.allow_space_in_identifier = parse_boolish_value(&value)?
            }
            "additional_allowed_characters" => {
                config.additional_allowed_characters = parse_option_string_none_value(&value)
            }
            "ignore_words" => config.ignore_words = split_comma_list(&value),
            "ignore_words_regex" => config.ignore_words_regex = parse_regex_list_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_references_quoting_rule_config(
    config: &mut ReferencesQuotingRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "prefer_quoted_identifiers" => {
                config.prefer_quoted_identifiers = parse_boolish_value(&value)?
            }
            "prefer_quoted_keywords" => {
                config.prefer_quoted_keywords = parse_boolish_value(&value)?
            }
            "ignore_words" => config.ignore_words = split_comma_list(&value),
            "ignore_words_regex" => config.ignore_words_regex = parse_regex_list_value(&value)?,
            "force_enable" => config.force_enable = parse_boolish_value(&value)?,
            _ => {}
        }
    }
    Ok(())
}

fn apply_layout_long_lines_rule_config(
    config: &mut LayoutLongLinesRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "ignore_comment_lines" => config.ignore_comment_lines = parse_boolish_value(&value)?,
            "ignore_comment_clauses" => {
                config.ignore_comment_clauses = parse_boolish_value(&value)?
            }
            _ => {}
        }
    }
    Ok(())
}

fn apply_layout_select_targets_rule_config(
    config: &mut LayoutSelectTargetsRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "wildcard_policy" {
            config.wildcard_policy = value;
        }
    }
    Ok(())
}

fn apply_layout_newlines_rule_config(
    config: &mut LayoutNewlinesRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        match key.as_str() {
            "maximum_empty_lines_between_statements" => {
                config.maximum_empty_lines_between_statements = parse_usize_value(&value)?
            }
            "maximum_empty_lines_inside_statements" => {
                config.maximum_empty_lines_inside_statements = parse_usize_value(&value)?
            }
            _ => {}
        }
    }
    Ok(())
}

fn apply_structure_subquery_rule_config(
    config: &mut StructureSubqueryRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "forbid_subquery_in" {
            config.forbid_subquery_in = value;
        }
    }
    Ok(())
}

fn apply_structure_join_condition_order_rule_config(
    config: &mut StructureJoinConditionOrderRuleConfig,
    values: Vec<(String, String)>,
) -> Result<(), String> {
    for (key, value) in values {
        if key == "preferred_first_table_in_join_clause" {
            config.preferred_first_table_in_join_clause = value;
        }
    }
    Ok(())
}

pub struct ConfigLoader;

impl ConfigLoader {
    #[allow(unused_variables)]
    fn iter_config_locations_up_to_path(
        path: &Path,
        working_path: Option<&Path>,
        ignore_local_config: bool,
    ) -> impl Iterator<Item = PathBuf> {
        let mut given_path = std::path::absolute(path).unwrap();
        let working_path = std::env::current_dir().unwrap();

        if !given_path.is_dir() {
            given_path = given_path.parent().unwrap().into();
        }

        let common_path = common_path::common_path(&given_path, working_path).unwrap();
        let mut path_to_visit = common_path;

        let head = Some(given_path.canonicalize().unwrap()).into_iter();
        let tail = std::iter::from_fn(move || {
            if path_to_visit != given_path {
                let path = path_to_visit.canonicalize().unwrap();

                let next_path_to_visit = {
                    // Convert `path_to_visit` & `given_path` to `Path`
                    let path_to_visit_as_path = path_to_visit.as_path();
                    let given_path_as_path = given_path.as_path();

                    // Attempt to create a relative path from `given_path` to `path_to_visit`
                    match given_path_as_path.strip_prefix(path_to_visit_as_path) {
                        Ok(relative_path) => {
                            // Get the first component of the relative path
                            if let Some(first_part) = relative_path.components().next() {
                                // Combine `path_to_visit` with the first part of the relative path
                                path_to_visit.join(first_part.as_os_str())
                            } else {
                                // If there are no components in the relative path, return
                                // `path_to_visit`
                                path_to_visit.clone()
                            }
                        }
                        Err(_) => {
                            // If `given_path` is not relative to `path_to_visit`, handle the error
                            // (e.g., return `path_to_visit`)
                            // This part depends on how you want to handle the error.
                            path_to_visit.clone()
                        }
                    }
                };

                if next_path_to_visit == path_to_visit {
                    return None;
                }

                path_to_visit = next_path_to_visit;

                Some(path)
            } else {
                None
            }
        });

        head.chain(tail)
    }

    fn load_config_from_source(source: &str, path: Option<&Path>) -> Result<FluffConfig, String> {
        let mut builder = ConfigLayerBuilder::new();
        Self::apply_source_to_builder(&mut builder, source, path)?;
        let mut config = merge_layers_replace_roots(vec![builder.into_layer()]);
        config.reload_reflow();
        Ok(config)
    }

    #[cfg(test)]
    fn load_config_at_path(&self, path: impl AsRef<Path>) -> Result<FluffConfig, String> {
        let layer = self.load_layer_at_path(path);
        let mut config = merge_layers_replace_roots(vec![layer]);
        config.reload_reflow();
        Ok(config)
    }

    fn load_config_up_to_path(
        &self,
        path: impl AsRef<Path>,
        extra_config_path: Option<String>,
        ignore_local_config: bool,
        overrides: Option<AHashMap<String, String>>,
    ) -> Result<FluffConfig, SQLFluffUserError> {
        let layers = self.load_layers_up_to_path(path, extra_config_path, ignore_local_config);
        let mut config = merge_layers_replace_roots(layers);
        if let Some(overrides) = overrides {
            apply_overrides_to_typed(&mut config, &overrides);
        }
        config.reload_reflow();
        Ok(config)
    }

    fn load_layers_up_to_path(
        &self,
        path: impl AsRef<Path>,
        extra_config_path: Option<String>,
        ignore_local_config: bool,
    ) -> Vec<ConfigLayer> {
        let path = path.as_ref();

        if ignore_local_config {
            return extra_config_path
                .map(|path| vec![self.load_layer_at_path(path)])
                .unwrap_or_default();
        }

        let configs = Self::iter_config_locations_up_to_path(path, None, ignore_local_config);
        configs
            .map(|path| self.load_layer_at_path(path))
            .collect_vec()
    }

    fn load_layer_at_path(&self, path: impl AsRef<Path>) -> ConfigLayer {
        let path = path.as_ref();

        let filename_options = [
            /* "setup.cfg", "tox.ini", "pep8.ini", */
            ".sqlfluff",
            ".sqruff",
            /* "pyproject.toml" */
        ];

        let mut builder = ConfigLayerBuilder::new();

        if path.is_dir() {
            for fname in filename_options {
                let path = path.join(fname);
                if path.exists() {
                    Self::apply_file_to_builder(&mut builder, &path);
                }
            }
        } else if path.is_file() {
            Self::apply_file_to_builder(&mut builder, path);
        };

        builder.into_layer()
    }

    fn apply_file_to_builder(builder: &mut ConfigLayerBuilder, path: &Path) {
        let content = std::fs::read_to_string(path).unwrap();
        Self::apply_source_to_builder(builder, &content, Some(path)).unwrap();
    }

    fn apply_source_to_builder(
        builder: &mut ConfigLayerBuilder,
        content: &str,
        config_path: Option<&Path>,
    ) -> Result<(), String> {
        match config_path.and_then(|path| path.extension().and_then(|ext| ext.to_str())) {
            Some("toml") => Err("TOML config is no longer supported".to_string()),
            _ => IniFormat.apply(content, config_path, builder),
        }
    }
}

fn is_none_string(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none")
}

fn split_comma_list(raw: &str) -> Vec<String> {
    if is_none_string(raw) {
        return Vec::new();
    }

    raw.split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect()
}

fn list_from_json_array(items: Vec<JsonValue>) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        match item {
            JsonValue::Null => {}
            JsonValue::String(value) => {
                if !is_none_string(&value) {
                    let value = value.trim();
                    if !value.is_empty() {
                        result.push(value.to_string());
                    }
                }
            }
            JsonValue::Bool(value) => result.push(value.to_string()),
            JsonValue::Number(value) => result.push(value.to_string()),
            JsonValue::Array(_) | JsonValue::Object(_) => {}
        }
    }
    result
}

fn parse_boolish(value: &str) -> Option<bool> {
    let trimmed = value.trim();
    if is_none_string(trimmed) {
        return None;
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "true" | "t" | "yes" | "y" => Some(true),
        "false" | "f" | "no" | "n" => Some(false),
        _ => trimmed.parse::<i64>().ok().map(|num| num != 0),
    }
}

fn parse_option_string_none_value(value: &str) -> Option<String> {
    if is_none_string(value) {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_optional_comma_list_value(value: &str) -> Option<Vec<String>> {
    if is_none_string(value) {
        None
    } else {
        Some(split_comma_list(value))
    }
}

fn parse_option_i32_value(value: &str) -> Option<i32> {
    if is_none_string(value) {
        None
    } else {
        value.trim().parse::<i32>().ok()
    }
}

fn parse_option_usize_value(value: &str) -> Option<usize> {
    if is_none_string(value) {
        None
    } else {
        value.trim().parse::<usize>().ok()
    }
}

fn parse_boolish_value(value: &str) -> Result<bool, String> {
    parse_boolish(value).ok_or_else(|| "Expected boolean value".to_string())
}

fn parse_usize_value(value: &str) -> Result<usize, String> {
    parse_option_usize_value(value).ok_or_else(|| "Expected integer value".to_string())
}

fn parse_regex_list_value(value: &str) -> Result<Vec<Regex>, String> {
    let patterns = split_comma_list(value);
    let mut regexes = Vec::with_capacity(patterns.len());
    for pattern in patterns {
        if is_none_string(&pattern) {
            continue;
        }
        let regex =
            Regex::new(&pattern).map_err(|err| format!("Invalid regex '{pattern}': {err}"))?;
        regexes.push(regex);
    }
    Ok(regexes)
}

pub(crate) fn deserialize_comma_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(match value {
        JsonValue::Null => Vec::new(),
        JsonValue::String(value) => split_comma_list(&value),
        JsonValue::Array(items) => list_from_json_array(items),
        JsonValue::Bool(value) => vec![value.to_string()],
        JsonValue::Number(value) => vec![value.to_string()],
        JsonValue::Object(_) => Vec::new(),
    })
}

pub(crate) fn deserialize_regex_list<'de, D>(deserializer: D) -> Result<Vec<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    let patterns = match value {
        JsonValue::Null => Vec::new(),
        JsonValue::String(value) => split_comma_list(&value),
        JsonValue::Array(items) => list_from_json_array(items),
        JsonValue::Bool(value) => vec![value.to_string()],
        JsonValue::Number(value) => vec![value.to_string()],
        JsonValue::Object(_) => Vec::new(),
    };

    let mut regexes = Vec::with_capacity(patterns.len());
    for pattern in patterns {
        if is_none_string(&pattern) {
            continue;
        }
        let regex = Regex::new(&pattern)
            .map_err(|err| de::Error::custom(format!("Invalid regex '{pattern}': {err}")))?;
        regexes.push(regex);
    }

    Ok(regexes)
}

pub(crate) fn deserialize_optional_comma_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(match value {
        JsonValue::Null => None,
        JsonValue::String(value) => {
            if value.trim().eq_ignore_ascii_case("none") {
                None
            } else {
                Some(split_comma_list(&value))
            }
        }
        JsonValue::Array(items) => Some(list_from_json_array(items)),
        JsonValue::Bool(value) => Some(vec![value.to_string()]),
        JsonValue::Number(value) => Some(vec![value.to_string()]),
        JsonValue::Object(_) => None,
    })
}

pub(crate) fn deserialize_option_string_none<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(match value {
        JsonValue::Null => None,
        JsonValue::String(value) => {
            if is_none_string(&value) {
                None
            } else {
                Some(value)
            }
        }
        JsonValue::Bool(value) => Some(value.to_string()),
        JsonValue::Number(value) => Some(value.to_string()),
        JsonValue::Array(_) | JsonValue::Object(_) => None,
    })
}

pub(crate) fn deserialize_option_boolish<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(match value {
        JsonValue::Null => None,
        JsonValue::Bool(value) => Some(value),
        JsonValue::Number(value) => value
            .as_i64()
            .map(|num| num != 0)
            .or_else(|| value.as_f64().map(|num| num != 0.0)),
        JsonValue::String(value) => parse_boolish(&value),
        JsonValue::Array(_) | JsonValue::Object(_) => None,
    })
}

pub(crate) fn deserialize_option_i32<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(match value {
        JsonValue::Null => None,
        JsonValue::Number(value) => value.as_i64().and_then(|num| i32::try_from(num).ok()),
        JsonValue::String(value) => {
            if is_none_string(&value) {
                None
            } else {
                value.trim().parse::<i32>().ok()
            }
        }
        JsonValue::Bool(_) | JsonValue::Array(_) | JsonValue::Object(_) => None,
    })
}

pub(crate) fn deserialize_option_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(match value {
        JsonValue::Null => None,
        JsonValue::Number(value) => value.as_i64().and_then(|num| usize::try_from(num).ok()),
        JsonValue::String(value) => {
            if is_none_string(&value) {
                None
            } else {
                value.trim().parse::<usize>().ok()
            }
        }
        JsonValue::Bool(_) | JsonValue::Array(_) | JsonValue::Object(_) => None,
    })
}

pub(crate) fn deserialize_usize<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_option_usize(deserializer)?
        .ok_or_else(|| de::Error::custom("Expected integer value"))
}

fn scalar_to_string(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(value) => Some(value.clone()),
        JsonValue::Bool(value) => Some(value.to_string()),
        JsonValue::Number(value) => Some(value.to_string()),
        JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_) => None,
    }
}

pub(crate) fn deserialize_string_map<'de, D>(
    deserializer: D,
) -> Result<AHashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    let mut result = AHashMap::new();

    if let JsonValue::Object(map) = value {
        for (key, value) in map {
            if let Some(value) = scalar_to_string(&value) {
                result.insert(key, value);
            }
        }
    }

    Ok(result)
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaceholderValue {
    String(String),
    Bool(bool),
    Int(i64),
    Float(f64),
}

fn json_to_placeholder_value(value: &JsonValue) -> Option<PlaceholderValue> {
    match value {
        JsonValue::String(value) => Some(PlaceholderValue::String(value.clone())),
        JsonValue::Bool(value) => Some(PlaceholderValue::Bool(*value)),
        JsonValue::Number(value) => value
            .as_i64()
            .map(PlaceholderValue::Int)
            .or_else(|| value.as_f64().map(PlaceholderValue::Float)),
        JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_) => None,
    }
}

pub(crate) fn deserialize_placeholder_replacements<'de, D>(
    deserializer: D,
) -> Result<AHashMap<String, PlaceholderValue>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    let mut result = AHashMap::new();

    if let JsonValue::Object(map) = value {
        for (key, value) in map {
            if key == "param_regex" || key == "param_style" {
                continue;
            }
            if let Some(value) = json_to_placeholder_value(&value) {
                result.insert(key, value);
            }
        }
    }

    Ok(result)
}

pub(crate) fn deserialize_boolish<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_option_boolish(deserializer)?
        .ok_or_else(|| de::Error::custom("Expected boolean value"))
}

fn default_core_templater() -> Option<String> {
    Some("raw".to_string())
}

fn default_core_nocolor() -> Option<bool> {
    Some(false)
}

fn default_core_verbose() -> Option<i32> {
    Some(0)
}

fn default_core_output_line_length() -> Option<i32> {
    Some(80)
}

fn default_core_runaway_limit() -> Option<i32> {
    Some(10)
}

fn default_core_warn_unused_ignores() -> Option<bool> {
    Some(false)
}

fn default_core_ignore_templated_areas() -> Option<bool> {
    Some(true)
}

fn default_core_encoding() -> Option<String> {
    Some("autodetect".to_string())
}

fn default_core_disable_noqa() -> Option<bool> {
    Some(false)
}

fn default_core_sql_file_exts() -> Vec<String> {
    vec![
        ".sql".to_string(),
        ".sql.j2".to_string(),
        ".dml".to_string(),
        ".ddl".to_string(),
    ]
}

fn default_core_fix_even_unparsable() -> Option<bool> {
    Some(false)
}

fn default_core_large_file_skip_char_limit() -> Option<i32> {
    Some(0)
}

fn default_core_large_file_skip_byte_limit() -> Option<i32> {
    Some(20000)
}

fn default_core_processes() -> Option<i32> {
    Some(1)
}

fn default_core_max_line_length() -> Option<i32> {
    Some(80)
}

fn default_core_rule_allowlist() -> Option<Vec<String>> {
    Some(vec!["core".to_string()])
}

fn default_indent_unit() -> Option<String> {
    Some("space".to_string())
}

fn default_tab_space_size() -> Option<i32> {
    Some(4)
}

fn default_indented_joins() -> Option<bool> {
    Some(false)
}

fn default_indented_ctes() -> Option<bool> {
    Some(false)
}

fn default_indented_using_on() -> Option<bool> {
    Some(true)
}

fn default_indented_on_contents() -> Option<bool> {
    Some(true)
}

fn default_indented_then() -> Option<bool> {
    Some(true)
}

fn default_indented_then_contents() -> Option<bool> {
    Some(true)
}

fn default_allow_implicit_indents() -> Option<bool> {
    Some(false)
}

fn default_template_blocks_indent() -> Option<bool> {
    Some(true)
}

fn default_skip_indentation_in() -> Option<Vec<String>> {
    Some(vec!["script_content".to_string()])
}

fn default_trailing_comments() -> Option<String> {
    Some("before".to_string())
}

fn default_templater_unwrap_wrapped_queries() -> Option<bool> {
    Some(true)
}

fn default_jinja_apply_dbt_builtins() -> Option<bool> {
    Some(true)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FluffConfig {
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub indentation: IndentationConfig,
    #[serde(default)]
    pub layout: LayoutConfig,
    #[serde(default)]
    pub templater: TemplaterConfig,
    #[serde(default)]
    pub rules: RulesConfig,
    #[serde(skip)]
    pub reflow: ReflowSettings,
    #[serde(skip)]
    pub reflow_config: ReflowConfig,
}

impl Default for FluffConfig {
    fn default() -> Self {
        let mut typed = Self {
            core: CoreConfig::default(),
            indentation: IndentationConfig::default(),
            layout: LayoutConfig::default(),
            templater: TemplaterConfig::default(),
            rules: RulesConfig::default(),
            reflow: ReflowSettings::default(),
            reflow_config: ReflowConfig::default(),
        };
        typed.reload_reflow();
        typed
    }
}

impl FluffConfig {
    pub fn from_source(
        source: &str,
        optional_path_specification: Option<&Path>,
    ) -> Result<Self, String> {
        ConfigLoader::load_config_from_source(source, optional_path_specification)
    }

    pub fn from_root(
        extra_config_path: Option<String>,
        ignore_local_config: bool,
        overrides: Option<AHashMap<String, String>>,
    ) -> Result<Self, SQLFluffUserError> {
        let loader = ConfigLoader {};
        loader.load_config_up_to_path(".", extra_config_path, ignore_local_config, overrides)
    }

    pub fn sql_file_exts(&self) -> &[String] {
        self.core.sql_file_exts.as_ref()
    }

    pub fn dialect(&self) -> Result<Dialect, String> {
        let dialect = match self.core.dialect.as_deref() {
            None => DialectKind::default(),
            Some(value) => {
                DialectKind::from_str(value).map_err(|_| format!("Invalid dialect: {}", value))?
            }
        };
        kind_to_dialect(&dialect).ok_or_else(|| format!("Invalid dialect: {}", dialect.as_ref()))
    }

    /// Check if the config specifies a dialect, raising an error if not.
    pub fn verify_dialect_specified(&self) -> Option<SQLFluffUserError> {
        // Legacy defaults include a dialect key even when set to None.
        None
    }

    /// Process a full raw file for inline config and update self.
    pub fn process_raw_file_for_config(&self, raw_str: &str) {
        for raw_line in raw_str.lines() {
            if raw_line.to_string().starts_with("-- sqlfluff") {
                self.process_inline_config(raw_line)
            }
        }
    }

    /// Process an inline config command and update self.
    pub fn process_inline_config(&self, _config_line: &str) {
        panic!("Not implemented")
    }

    pub fn reload_reflow(&mut self) {
        self.reflow = ReflowSettings::from_typed(&self.core, &self.indentation);
        self.reflow_config = ReflowConfig::from_typed(self);
    }

    pub fn override_dialect(&mut self, dialect: DialectKind) -> Result<(), String> {
        kind_to_dialect(&dialect)
            .ok_or_else(|| format!("Invalid dialect: {}", dialect.as_ref()))?;
        self.core.dialect = Some(dialect.as_ref().to_string());
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CoreConfig {
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub dialect: Option<String>,
    #[serde(
        default = "default_core_templater",
        deserialize_with = "deserialize_option_string_none"
    )]
    pub templater: Option<String>,
    #[serde(
        default = "default_core_nocolor",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub nocolor: Option<bool>,
    #[serde(
        default = "default_core_verbose",
        deserialize_with = "deserialize_option_i32"
    )]
    pub verbose: Option<i32>,
    #[serde(
        default = "default_core_output_line_length",
        deserialize_with = "deserialize_option_i32"
    )]
    pub output_line_length: Option<i32>,
    #[serde(
        default = "default_core_runaway_limit",
        deserialize_with = "deserialize_option_i32"
    )]
    pub runaway_limit: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_optional_comma_list")]
    pub ignore: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_comma_list")]
    pub warnings: Option<Vec<String>>,
    #[serde(
        default = "default_core_warn_unused_ignores",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub warn_unused_ignores: Option<bool>,
    #[serde(
        default = "default_core_ignore_templated_areas",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub ignore_templated_areas: Option<bool>,
    #[serde(
        default = "default_core_encoding",
        deserialize_with = "deserialize_option_string_none"
    )]
    pub encoding: Option<String>,
    #[serde(
        default = "default_core_disable_noqa",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub disable_noqa: Option<bool>,
    #[serde(
        default = "default_core_sql_file_exts",
        deserialize_with = "deserialize_comma_list"
    )]
    pub sql_file_exts: Vec<String>,
    #[serde(
        default = "default_core_fix_even_unparsable",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub fix_even_unparsable: Option<bool>,
    #[serde(
        default = "default_core_large_file_skip_char_limit",
        deserialize_with = "deserialize_option_i32"
    )]
    pub large_file_skip_char_limit: Option<i32>,
    #[serde(
        default = "default_core_large_file_skip_byte_limit",
        deserialize_with = "deserialize_option_i32"
    )]
    pub large_file_skip_byte_limit: Option<i32>,
    #[serde(
        default = "default_core_processes",
        deserialize_with = "deserialize_option_i32"
    )]
    pub processes: Option<i32>,
    #[serde(
        default = "default_core_max_line_length",
        deserialize_with = "deserialize_option_i32"
    )]
    pub max_line_length: Option<i32>,
    #[serde(
        default = "default_core_rule_allowlist",
        deserialize_with = "deserialize_optional_comma_list"
    )]
    pub rule_allowlist: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub rule_denylist: Vec<String>,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            dialect: None,
            templater: Some("raw".to_string()),
            nocolor: Some(false),
            verbose: Some(0),
            output_line_length: Some(80),
            runaway_limit: Some(10),
            ignore: None,
            warnings: None,
            warn_unused_ignores: Some(false),
            ignore_templated_areas: Some(true),
            encoding: Some("autodetect".to_string()),
            disable_noqa: Some(false),
            sql_file_exts: vec![
                ".sql".to_string(),
                ".sql.j2".to_string(),
                ".dml".to_string(),
                ".ddl".to_string(),
            ],
            fix_even_unparsable: Some(false),
            large_file_skip_char_limit: Some(0),
            large_file_skip_byte_limit: Some(20000),
            processes: Some(1),
            max_line_length: Some(80),
            rule_allowlist: Some(vec!["core".to_string()]),
            rule_denylist: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct IndentationConfig {
    #[serde(
        default = "default_indent_unit",
        deserialize_with = "deserialize_option_string_none"
    )]
    pub indent_unit: Option<String>,
    #[serde(
        default = "default_tab_space_size",
        deserialize_with = "deserialize_option_i32"
    )]
    pub tab_space_size: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_option_boolish")]
    pub hanging_indents: Option<bool>,
    #[serde(
        default = "default_indented_joins",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub indented_joins: Option<bool>,
    #[serde(
        default = "default_indented_ctes",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub indented_ctes: Option<bool>,
    #[serde(
        default = "default_indented_using_on",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub indented_using_on: Option<bool>,
    #[serde(
        default = "default_indented_on_contents",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub indented_on_contents: Option<bool>,
    #[serde(
        default = "default_indented_then",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub indented_then: Option<bool>,
    #[serde(
        default = "default_indented_then_contents",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub indented_then_contents: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_option_boolish")]
    pub indented_joins_on: Option<bool>,
    #[serde(
        default = "default_allow_implicit_indents",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub allow_implicit_indents: Option<bool>,
    #[serde(
        default = "default_template_blocks_indent",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub template_blocks_indent: Option<bool>,
    #[serde(
        default = "default_skip_indentation_in",
        deserialize_with = "deserialize_optional_comma_list"
    )]
    pub skip_indentation_in: Option<Vec<String>>,
    #[serde(
        default = "default_trailing_comments",
        deserialize_with = "deserialize_option_string_none"
    )]
    pub trailing_comments: Option<String>,
}

impl Default for IndentationConfig {
    fn default() -> Self {
        Self {
            indent_unit: Some("space".to_string()),
            tab_space_size: Some(4),
            hanging_indents: None,
            indented_joins: Some(false),
            indented_ctes: Some(false),
            indented_using_on: Some(true),
            indented_on_contents: Some(true),
            indented_then: Some(true),
            indented_then_contents: Some(true),
            indented_joins_on: None,
            allow_implicit_indents: Some(false),
            template_blocks_indent: Some(true),
            skip_indentation_in: Some(vec!["script_content".to_string()]),
            trailing_comments: Some("before".to_string()),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LayoutConfig {
    #[serde(
        default = "default_layout_types",
        rename = "type",
        deserialize_with = "deserialize_layout_types"
    )]
    pub types: AHashMap<String, LayoutTypeConfig>,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            types: default_layout_types(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LayoutTypeConfig {
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub spacing_before: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub spacing_after: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub spacing_within: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub line_position: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub align_within: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub align_scope: Option<String>,
}

fn default_layout_types() -> AHashMap<String, LayoutTypeConfig> {
    let mut types = AHashMap::new();
    types.insert(
        "comma".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: Some("trailing".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "binary_operator".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch".to_string()),
            line_position: Some("leading".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "statement_terminator".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: Some("trailing".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "end_of_file".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "set_operator".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("alone:strict".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "start_bracket".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: Some("touch".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "end_bracket".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "start_square_bracket".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: Some("touch".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "end_square_bracket".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "start_angle_bracket".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: Some("touch".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "end_angle_bracket".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "casting_operator".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: Some("touch:inline".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "slice".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: Some("touch".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "dot".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: Some("touch".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "comparison_operator".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch".to_string()),
            line_position: Some("leading".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "assignment_operator".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch".to_string()),
            line_position: Some("leading".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "object_reference".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch:inline".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "numeric_literal".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch:inline".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "sign_indicator".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: Some("touch:inline".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "tilde".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: Some("touch:inline".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "function_name".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch:inline".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "function_contents".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch:inline".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "function_parameter_list".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch:inline".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "array_type".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch:inline".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "typed_array_literal".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "sized_array_type".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "struct_type".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch:inline".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "bracketed_arguments".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch:inline".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "typed_struct_literal".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "semi_structured_expression".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch:inline".to_string()),
            spacing_after: None,
            spacing_within: Some("touch:inline".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "array_accessor".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch:inline".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "colon".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "colon_delimiter".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: Some("touch".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "path_segment".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "sql_conf_option".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("touch".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "sqlcmd_operator".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("touch".to_string()),
            spacing_after: None,
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "comment".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("any".to_string()),
            spacing_after: Some("any".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "inline_comment".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("any".to_string()),
            spacing_after: Some("any".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "block_comment".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("any".to_string()),
            spacing_after: Some("any".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "pattern_expression".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("any".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "placeholder".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("any".to_string()),
            spacing_after: Some("any".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "common_table_expression".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: Some("single:inline".to_string()),
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "select_clause".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("alone".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "where_clause".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("alone".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "from_clause".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("alone".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "join_clause".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("alone".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "groupby_clause".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("alone".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "orderby_clause".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("leading".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "having_clause".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("alone".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "limit_clause".to_string(),
        LayoutTypeConfig {
            spacing_before: None,
            spacing_after: None,
            spacing_within: None,
            line_position: Some("alone".to_string()),
            align_within: None,
            align_scope: None,
        },
    );
    types.insert(
        "template_loop".to_string(),
        LayoutTypeConfig {
            spacing_before: Some("any".to_string()),
            spacing_after: Some("any".to_string()),
            spacing_within: None,
            line_position: None,
            align_within: None,
            align_scope: None,
        },
    );
    types
}

fn deserialize_layout_types<'de, D>(
    deserializer: D,
) -> Result<AHashMap<String, LayoutTypeConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    let overrides = AHashMap::<String, LayoutTypeConfig>::deserialize(deserializer)?;
    let mut types = default_layout_types();
    for (key, value) in overrides {
        types.insert(key, value);
    }
    Ok(types)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TemplaterConfig {
    #[serde(
        default = "default_templater_unwrap_wrapped_queries",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub unwrap_wrapped_queries: Option<bool>,
    #[serde(default)]
    pub jinja: JinjaTemplaterConfig,
    #[serde(default)]
    pub dbt: DbtTemplaterConfig,
    #[serde(default)]
    pub python: PythonTemplaterConfig,
    #[serde(default)]
    pub placeholder: PlaceholderTemplaterConfig,
}

impl Default for TemplaterConfig {
    fn default() -> Self {
        Self {
            unwrap_wrapped_queries: Some(true),
            jinja: JinjaTemplaterConfig::default(),
            dbt: DbtTemplaterConfig::default(),
            python: PythonTemplaterConfig::default(),
            placeholder: PlaceholderTemplaterConfig::default(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct JinjaTemplaterConfig {
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub templater_paths: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub loader_search_path: Vec<String>,
    #[serde(
        default = "default_jinja_apply_dbt_builtins",
        deserialize_with = "deserialize_option_boolish"
    )]
    pub apply_dbt_builtins: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_option_boolish")]
    pub ignore_templating: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub library_paths: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub context: AHashMap<String, String>,
}

impl Default for JinjaTemplaterConfig {
    fn default() -> Self {
        Self {
            templater_paths: Vec::new(),
            loader_search_path: Vec::new(),
            apply_dbt_builtins: Some(true),
            ignore_templating: None,
            library_paths: Vec::new(),
            context: AHashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DbtTemplaterConfig {
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub profiles_dir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub project_dir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub context: AHashMap<String, String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PythonTemplaterConfig {
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub context: AHashMap<String, String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PlaceholderTemplaterConfig {
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub param_regex: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub param_style: Option<String>,
    #[serde(
        default,
        flatten,
        deserialize_with = "deserialize_placeholder_replacements"
    )]
    pub replacements: AHashMap<String, PlaceholderValue>,
}

fn default_rules_allow_scalar() -> bool {
    true
}

fn default_consistent() -> String {
    "consistent".to_string()
}

fn default_all() -> String {
    "all".to_string()
}

fn default_aliases() -> String {
    "aliases".to_string()
}

fn default_explicit() -> String {
    "explicit".to_string()
}

fn default_inner() -> String {
    "inner".to_string()
}

fn default_forbid() -> String {
    "forbid".to_string()
}

fn default_single() -> String {
    "single".to_string()
}

fn default_join() -> String {
    "join".to_string()
}

fn default_earlier() -> String {
    "earlier".to_string()
}

fn default_max_empty_lines_between_statements() -> usize {
    2
}

fn default_max_empty_lines_inside_statements() -> usize {
    1
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RulesConfig {
    #[serde(
        default = "default_rules_allow_scalar",
        deserialize_with = "deserialize_boolish"
    )]
    pub allow_scalar: bool,
    #[serde(default = "default_consistent")]
    pub single_table_references: String,
    #[serde(default = "default_all")]
    pub unquoted_identifiers_policy: String,
    #[serde(default, rename = "aliasing.table")]
    pub aliasing_table: AliasingRuleConfig,
    #[serde(default, rename = "aliasing.column")]
    pub aliasing_column: AliasingRuleConfig,
    #[serde(default, rename = "aliasing.length")]
    pub aliasing_length: AliasingLengthRuleConfig,
    #[serde(default, rename = "aliasing.forbid")]
    pub aliasing_forbid: AliasingForbidRuleConfig,
    #[serde(default, rename = "ambiguous.join")]
    pub ambiguous_join: AmbiguousJoinRuleConfig,
    #[serde(default, rename = "ambiguous.column_references")]
    pub ambiguous_column_references: AmbiguousColumnReferencesRuleConfig,
    #[serde(default, rename = "capitalisation.keywords")]
    pub capitalisation_keywords: CapitalisationKeywordsRuleConfig,
    #[serde(default, rename = "capitalisation.identifiers")]
    pub capitalisation_identifiers: CapitalisationIdentifiersRuleConfig,
    #[serde(default, rename = "capitalisation.functions")]
    pub capitalisation_functions: CapitalisationFunctionsRuleConfig,
    #[serde(default, rename = "capitalisation.literals")]
    pub capitalisation_literals: CapitalisationLiteralsRuleConfig,
    #[serde(default, rename = "capitalisation.types")]
    pub capitalisation_types: CapitalisationTypesRuleConfig,
    #[serde(default, rename = "convention.select_trailing_comma")]
    pub convention_select_trailing_comma: ConventionSelectTrailingCommaRuleConfig,
    #[serde(default, rename = "convention.count_rows")]
    pub convention_count_rows: ConventionCountRowsRuleConfig,
    #[serde(default, rename = "convention.terminator")]
    pub convention_terminator: ConventionTerminatorRuleConfig,
    #[serde(default, rename = "convention.blocked_words")]
    pub convention_blocked_words: ConventionBlockedWordsRuleConfig,
    #[serde(default, rename = "convention.quoted_literals")]
    pub convention_quoted_literals: ConventionQuotedLiteralsRuleConfig,
    #[serde(default, rename = "convention.casting_style")]
    pub convention_casting_style: ConventionCastingStyleRuleConfig,
    #[serde(default, rename = "convention.not_equal")]
    pub convention_not_equal: ConventionNotEqualRuleConfig,
    #[serde(default, rename = "references.from")]
    pub references_from: ReferencesFromRuleConfig,
    #[serde(default, rename = "references.qualification")]
    pub references_qualification: ReferencesQualificationRuleConfig,
    #[serde(default, rename = "references.consistent")]
    pub references_consistent: ReferencesConsistentRuleConfig,
    #[serde(default, rename = "references.keywords")]
    pub references_keywords: ReferencesKeywordsRuleConfig,
    #[serde(default, rename = "references.special_chars")]
    pub references_special_chars: ReferencesSpecialCharsRuleConfig,
    #[serde(default, rename = "references.quoting")]
    pub references_quoting: ReferencesQuotingRuleConfig,
    #[serde(default, rename = "layout.long_lines")]
    pub layout_long_lines: LayoutLongLinesRuleConfig,
    #[serde(default, rename = "layout.select_targets")]
    pub layout_select_targets: LayoutSelectTargetsRuleConfig,
    #[serde(default, rename = "layout.newlines")]
    pub layout_newlines: LayoutNewlinesRuleConfig,
    #[serde(default, rename = "structure.subquery")]
    pub structure_subquery: StructureSubqueryRuleConfig,
    #[serde(default, rename = "structure.join_condition_order")]
    pub structure_join_condition_order: StructureJoinConditionOrderRuleConfig,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            allow_scalar: default_rules_allow_scalar(),
            single_table_references: default_consistent(),
            unquoted_identifiers_policy: default_all(),
            aliasing_table: AliasingRuleConfig::default(),
            aliasing_column: AliasingRuleConfig::default(),
            aliasing_length: AliasingLengthRuleConfig::default(),
            aliasing_forbid: AliasingForbidRuleConfig::default(),
            ambiguous_join: AmbiguousJoinRuleConfig::default(),
            ambiguous_column_references: AmbiguousColumnReferencesRuleConfig::default(),
            capitalisation_keywords: CapitalisationKeywordsRuleConfig::default(),
            capitalisation_identifiers: CapitalisationIdentifiersRuleConfig::default(),
            capitalisation_functions: CapitalisationFunctionsRuleConfig::default(),
            capitalisation_literals: CapitalisationLiteralsRuleConfig::default(),
            capitalisation_types: CapitalisationTypesRuleConfig::default(),
            convention_select_trailing_comma: ConventionSelectTrailingCommaRuleConfig::default(),
            convention_count_rows: ConventionCountRowsRuleConfig::default(),
            convention_terminator: ConventionTerminatorRuleConfig::default(),
            convention_blocked_words: ConventionBlockedWordsRuleConfig::default(),
            convention_quoted_literals: ConventionQuotedLiteralsRuleConfig::default(),
            convention_casting_style: ConventionCastingStyleRuleConfig::default(),
            convention_not_equal: ConventionNotEqualRuleConfig::default(),
            references_from: ReferencesFromRuleConfig::default(),
            references_qualification: ReferencesQualificationRuleConfig::default(),
            references_consistent: ReferencesConsistentRuleConfig::default(),
            references_keywords: ReferencesKeywordsRuleConfig::default(),
            references_special_chars: ReferencesSpecialCharsRuleConfig::default(),
            references_quoting: ReferencesQuotingRuleConfig::default(),
            layout_long_lines: LayoutLongLinesRuleConfig::default(),
            layout_select_targets: LayoutSelectTargetsRuleConfig::default(),
            layout_newlines: LayoutNewlinesRuleConfig::default(),
            structure_subquery: StructureSubqueryRuleConfig::default(),
            structure_join_condition_order: StructureJoinConditionOrderRuleConfig::default(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AliasingRuleConfig {
    #[serde(default = "default_explicit")]
    pub aliasing: String,
}

impl Default for AliasingRuleConfig {
    fn default() -> Self {
        Self {
            aliasing: default_explicit(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct AliasingLengthRuleConfig {
    #[serde(default, deserialize_with = "deserialize_option_usize")]
    pub min_alias_length: Option<usize>,
    #[serde(default, deserialize_with = "deserialize_option_usize")]
    pub max_alias_length: Option<usize>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct AliasingForbidRuleConfig {
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub force_enable: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AmbiguousJoinRuleConfig {
    #[serde(default = "default_inner")]
    pub fully_qualify_join_types: String,
}

impl Default for AmbiguousJoinRuleConfig {
    fn default() -> Self {
        Self {
            fully_qualify_join_types: default_inner(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AmbiguousColumnReferencesRuleConfig {
    #[serde(default = "default_consistent")]
    pub group_by_and_order_by_style: String,
}

impl Default for AmbiguousColumnReferencesRuleConfig {
    fn default() -> Self {
        Self {
            group_by_and_order_by_style: default_consistent(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CapitalisationKeywordsRuleConfig {
    #[serde(default = "default_consistent")]
    pub capitalisation_policy: String,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub ignore_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub ignore_words_regex: Vec<Regex>,
}

impl Default for CapitalisationKeywordsRuleConfig {
    fn default() -> Self {
        Self {
            capitalisation_policy: default_consistent(),
            ignore_words: Vec::new(),
            ignore_words_regex: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CapitalisationIdentifiersRuleConfig {
    #[serde(default = "default_consistent")]
    pub extended_capitalisation_policy: String,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub ignore_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub ignore_words_regex: Vec<Regex>,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub unquoted_identifiers_policy: Option<String>,
}

impl Default for CapitalisationIdentifiersRuleConfig {
    fn default() -> Self {
        Self {
            extended_capitalisation_policy: default_consistent(),
            ignore_words: Vec::new(),
            ignore_words_regex: Vec::new(),
            unquoted_identifiers_policy: None,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CapitalisationFunctionsRuleConfig {
    #[serde(default = "default_consistent")]
    pub extended_capitalisation_policy: String,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub ignore_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub ignore_words_regex: Vec<Regex>,
}

impl Default for CapitalisationFunctionsRuleConfig {
    fn default() -> Self {
        Self {
            extended_capitalisation_policy: default_consistent(),
            ignore_words: Vec::new(),
            ignore_words_regex: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CapitalisationLiteralsRuleConfig {
    #[serde(default = "default_consistent")]
    pub capitalisation_policy: String,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub ignore_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub ignore_words_regex: Vec<Regex>,
}

impl Default for CapitalisationLiteralsRuleConfig {
    fn default() -> Self {
        Self {
            capitalisation_policy: default_consistent(),
            ignore_words: Vec::new(),
            ignore_words_regex: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CapitalisationTypesRuleConfig {
    #[serde(default = "default_consistent")]
    pub extended_capitalisation_policy: String,
}

impl Default for CapitalisationTypesRuleConfig {
    fn default() -> Self {
        Self {
            extended_capitalisation_policy: default_consistent(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConventionSelectTrailingCommaRuleConfig {
    #[serde(default = "default_forbid")]
    pub select_clause_trailing_comma: String,
}

impl Default for ConventionSelectTrailingCommaRuleConfig {
    fn default() -> Self {
        Self {
            select_clause_trailing_comma: default_forbid(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ConventionCountRowsRuleConfig {
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub prefer_count_1: bool,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub prefer_count_0: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ConventionTerminatorRuleConfig {
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub multiline_newline: bool,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub require_final_semicolon: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ConventionBlockedWordsRuleConfig {
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub blocked_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub blocked_regex: Vec<Regex>,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub match_source: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConventionQuotedLiteralsRuleConfig {
    #[serde(default = "default_consistent")]
    pub preferred_quoted_literal_style: String,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub force_enable: bool,
}

impl Default for ConventionQuotedLiteralsRuleConfig {
    fn default() -> Self {
        Self {
            preferred_quoted_literal_style: default_consistent(),
            force_enable: false,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConventionCastingStyleRuleConfig {
    #[serde(default = "default_consistent")]
    pub preferred_type_casting_style: String,
}

impl Default for ConventionCastingStyleRuleConfig {
    fn default() -> Self {
        Self {
            preferred_type_casting_style: default_consistent(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConventionNotEqualRuleConfig {
    #[serde(default = "default_consistent")]
    pub preferred_not_equal_style: String,
}

impl Default for ConventionNotEqualRuleConfig {
    fn default() -> Self {
        Self {
            preferred_not_equal_style: default_consistent(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ReferencesFromRuleConfig {
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub force_enable: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ReferencesQualificationRuleConfig {
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub ignore_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub ignore_words_regex: Vec<Regex>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ReferencesConsistentRuleConfig {
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub single_table_references: Option<String>,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub force_enable: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReferencesKeywordsRuleConfig {
    #[serde(default = "default_aliases")]
    pub unquoted_identifiers_policy: String,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub quoted_identifiers_policy: Option<String>,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub ignore_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub ignore_words_regex: Vec<Regex>,
}

impl Default for ReferencesKeywordsRuleConfig {
    fn default() -> Self {
        Self {
            unquoted_identifiers_policy: default_aliases(),
            quoted_identifiers_policy: Some("none".to_string()),
            ignore_words: Vec::new(),
            ignore_words_regex: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReferencesSpecialCharsRuleConfig {
    #[serde(default = "default_all")]
    pub quoted_identifiers_policy: String,
    #[serde(default = "default_all")]
    pub unquoted_identifiers_policy: String,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub allow_space_in_identifier: bool,
    #[serde(default, deserialize_with = "deserialize_option_string_none")]
    pub additional_allowed_characters: Option<String>,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub ignore_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub ignore_words_regex: Vec<Regex>,
}

impl Default for ReferencesSpecialCharsRuleConfig {
    fn default() -> Self {
        Self {
            quoted_identifiers_policy: default_all(),
            unquoted_identifiers_policy: default_all(),
            allow_space_in_identifier: false,
            additional_allowed_characters: None,
            ignore_words: Vec::new(),
            ignore_words_regex: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ReferencesQuotingRuleConfig {
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub prefer_quoted_identifiers: bool,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub prefer_quoted_keywords: bool,
    #[serde(default, deserialize_with = "deserialize_comma_list")]
    pub ignore_words: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_regex_list")]
    pub ignore_words_regex: Vec<Regex>,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub force_enable: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LayoutLongLinesRuleConfig {
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub ignore_comment_lines: bool,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub ignore_comment_clauses: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LayoutSelectTargetsRuleConfig {
    #[serde(default = "default_single")]
    pub wildcard_policy: String,
}

impl Default for LayoutSelectTargetsRuleConfig {
    fn default() -> Self {
        Self {
            wildcard_policy: default_single(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LayoutNewlinesRuleConfig {
    #[serde(
        default = "default_max_empty_lines_between_statements",
        deserialize_with = "deserialize_usize"
    )]
    pub maximum_empty_lines_between_statements: usize,
    #[serde(
        default = "default_max_empty_lines_inside_statements",
        deserialize_with = "deserialize_usize"
    )]
    pub maximum_empty_lines_inside_statements: usize,
}

impl Default for LayoutNewlinesRuleConfig {
    fn default() -> Self {
        Self {
            maximum_empty_lines_between_statements: default_max_empty_lines_between_statements(),
            maximum_empty_lines_inside_statements: default_max_empty_lines_inside_statements(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StructureSubqueryRuleConfig {
    #[serde(default = "default_join")]
    pub forbid_subquery_in: String,
}

impl Default for StructureSubqueryRuleConfig {
    fn default() -> Self {
        Self {
            forbid_subquery_in: default_join(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StructureJoinConditionOrderRuleConfig {
    #[serde(default = "default_earlier")]
    pub preferred_first_table_in_join_clause: String,
}

impl Default for StructureJoinConditionOrderRuleConfig {
    fn default() -> Self {
        Self {
            preferred_first_table_in_join_clause: default_earlier(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ReflowSettings {
    pub max_line_length: Option<usize>,
    pub hanging_indents: Option<bool>,
    pub allow_implicit_indents: Option<bool>,
    pub trailing_comments: Option<String>,
    pub indent_unit: Option<String>,
    pub tab_space_size: Option<usize>,
}

impl ReflowSettings {
    fn from_typed(core: &CoreConfig, indentation: &IndentationConfig) -> Self {
        Self {
            max_line_length: core.max_line_length.map(|value| value as usize),
            hanging_indents: indentation.hanging_indents,
            allow_implicit_indents: indentation.allow_implicit_indents,
            trailing_comments: indentation.trailing_comments.clone(),
            indent_unit: indentation.indent_unit.clone(),
            tab_space_size: indentation.tab_space_size.map(|value| value as usize),
        }
    }
}

fn merge_layers_replace_roots(config_stack: Vec<ConfigLayer>) -> FluffConfig {
    let mut result = FluffConfig::default();
    for layer in config_stack {
        if layer.is_empty() {
            continue;
        }
        if let Some(core) = layer.core {
            result.core = core;
        }
        if let Some(indentation) = layer.indentation {
            result.indentation = indentation;
        }
        if let Some(layout) = layer.layout {
            result.layout = layout;
        }
        if let Some(templater) = layer.templater {
            result.templater = templater;
        }
        if let Some(rules) = layer.rules {
            result.rules = rules;
        }
    }
    result
}

fn apply_overrides_to_typed(config: &mut FluffConfig, overrides: &AHashMap<String, String>) {
    if let Some(dialect) = overrides.get("dialect") {
        config.core.dialect = Some(dialect.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut dir = std::env::current_dir().unwrap();
        dir.push("target");
        dir.push("tmp_config_tests");
        dir.push(format!("{}_{}_{}", prefix, std::process::id(), nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(path: &Path, content: &str) {
        fs::write(path, content).unwrap();
    }

    #[test]
    fn config_section_mapping_and_normalization() {
        let source = "[sqlfluff]
dialect = ansi
max_line_length = 120
nocolor = true

[sqruff]
verbose = 2

[sqlfluff:rules:capitalisation.keywords]
ignore_words = foo,bar

[sqruff:templater:jinja]
apply_dbt_builtins = TRUE
";

        let typed = FluffConfig::from_source(source, None).unwrap();

        assert_eq!(typed.core.dialect.as_deref(), Some("ansi"));
        assert_eq!(typed.core.max_line_length, Some(120));
        assert_eq!(typed.core.nocolor, Some(true));
        assert_eq!(typed.core.verbose, Some(2));
        assert_eq!(
            typed.rules.capitalisation_keywords.ignore_words,
            vec!["foo".to_string(), "bar".to_string()]
        );
        assert_eq!(typed.templater.jinja.apply_dbt_builtins, Some(true));
    }

    #[test]
    fn get_config_from_file_absolutizes_path_and_dir() {
        let base_dir = unique_test_dir("paths");
        let config_dir = base_dir.join("config");
        fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join(".sqruff");
        let abs_path = base_dir.join("already_absolute");
        let config_contents = format!(
            "[sqlfluff:templater:dbt]
profiles_dir = relative_dir
project_dir = {}
",
            abs_path.to_string_lossy()
        );
        write_file(&config_path, &config_contents);

        let loader = ConfigLoader {};
        let config = loader.load_config_at_path(&config_dir).unwrap();
        let dbt = &config.templater.dbt;

        let config_dir_abs = std::path::absolute(&config_dir).unwrap();
        let expected_profiles = config_dir_abs
            .join("relative_dir")
            .to_string_lossy()
            .into_owned();
        let expected_project = abs_path.to_string_lossy().into_owned();

        assert_eq!(
            dbt.profiles_dir.as_deref(),
            Some(expected_profiles.as_str())
        );
        assert_eq!(dbt.project_dir.as_deref(), Some(expected_project.as_str()));
    }

    #[test]
    fn load_config_up_to_path_precedence_favors_parent_dir() {
        let base_dir = unique_test_dir("stack");
        let child_dir = base_dir.join("child");
        fs::create_dir_all(&child_dir).unwrap();

        let base_config = base_dir.join(".sqruff");
        let child_config = child_dir.join(".sqruff");
        write_file(&base_config, "[sqlfluff]\ndialect = base\n");
        write_file(
            &child_config,
            "[sqlfluff]\ndialect = child\ntemplater = child_templater\n",
        );

        let target_file = child_dir.join("input.sql");
        write_file(&target_file, "select 1;\n");

        let loader = ConfigLoader {};
        let config = loader
            .load_config_up_to_path(&target_file, None, false, None)
            .unwrap();

        assert_eq!(config.core.dialect.as_deref(), Some("base"));
        assert_eq!(config.core.templater.as_deref(), Some("raw"));
    }

    #[test]
    fn typed_config_parses_core_fields() {
        let source = "[sqlfluff]
nocolor = true
verbose = 2
rules = LT01, LT02
";
        let typed = FluffConfig::from_source(source, None).unwrap();

        assert_eq!(typed.core.nocolor, Some(true));
        assert_eq!(typed.core.verbose, Some(2));
        assert_eq!(
            typed.core.rule_allowlist,
            Some(vec!["LT01".to_string(), "LT02".to_string()])
        );
    }

    #[test]
    fn typed_config_rules_none_yields_none() {
        let typed = FluffConfig::from_source("[sqlfluff]\nrules = None\n", None).unwrap();

        assert_eq!(typed.core.rule_allowlist, None);
    }

    #[test]
    fn typed_config_sql_file_exts_from_defaults() {
        let typed = FluffConfig::default();

        assert!(typed.core.sql_file_exts.iter().any(|ext| ext == ".sql"));
    }

    #[test]
    fn typed_defaults_match_expected_values() {
        let typed = FluffConfig::default();

        assert_eq!(typed.core.verbose, Some(0));
        assert_eq!(typed.core.nocolor, Some(false));
        assert_eq!(typed.core.dialect, None);
        assert_eq!(typed.core.templater.as_deref(), Some("raw"));
        assert_eq!(typed.core.rule_allowlist, Some(vec!["core".to_string()]));
        assert!(typed.core.sql_file_exts.iter().any(|ext| ext == ".sql"));
        assert_eq!(typed.indentation.template_blocks_indent, Some(true));
        assert_eq!(typed.indentation.indented_using_on, Some(true));
        assert_eq!(typed.templater.unwrap_wrapped_queries, Some(true));
        assert_eq!(typed.templater.jinja.apply_dbt_builtins, Some(true));

        let comma = typed.layout.types.get("comma").unwrap();
        assert_eq!(comma.spacing_before.as_deref(), Some("touch"));
        assert_eq!(comma.line_position.as_deref(), Some("trailing"));
    }

    #[test]
    fn typed_parity_ini_matches_expected_values() {
        let source = "[sqlfluff]
nocolor = true
verbose = 2
output_line_length = 90
max_line_length = 120
rules = LT01,LT02
ignore = foo,bar
warnings = TMP,PRS
encoding = utf-8
sql_file_exts = .sql,.sql.j2

[sqlfluff:indentation]
template_blocks_indent = false

[sqlfluff:templater]
unwrap_wrapped_queries = false

[sqlfluff:templater:jinja]
apply_dbt_builtins = false
templater_paths = macro1,macro2
loader_search_path = path1,path2

[sqlfluff:layout:type:comma]
spacing_before = touch
line_position = trailing
";
        let base_dir = unique_test_dir("typed_ini_parity");
        let config_path = base_dir.join(".sqruff");
        write_file(&config_path, source);

        let loader = ConfigLoader {};
        let typed = loader.load_config_at_path(&base_dir).unwrap();
        let config_dir_abs = std::path::absolute(&base_dir).unwrap();
        let expected_loader_paths = vec![
            config_dir_abs.join("path1").to_string_lossy().into_owned(),
            config_dir_abs.join("path2").to_string_lossy().into_owned(),
        ];

        assert_eq!(typed.core.nocolor, Some(true));
        assert_eq!(typed.core.verbose, Some(2));
        assert_eq!(typed.core.output_line_length, Some(90));
        assert_eq!(typed.core.max_line_length, Some(120));
        assert_eq!(
            typed.core.rule_allowlist,
            Some(vec!["LT01".to_string(), "LT02".to_string()])
        );
        assert_eq!(
            typed.core.ignore,
            Some(vec!["foo".to_string(), "bar".to_string()])
        );
        assert_eq!(
            typed.core.warnings,
            Some(vec!["TMP".to_string(), "PRS".to_string()])
        );
        assert_eq!(typed.core.encoding.as_deref(), Some("utf-8"));
        assert_eq!(
            typed.core.sql_file_exts,
            vec![".sql".to_string(), ".sql.j2".to_string()]
        );
        assert_eq!(typed.indentation.template_blocks_indent, Some(false));
        assert_eq!(typed.templater.unwrap_wrapped_queries, Some(false));
        assert_eq!(typed.templater.jinja.apply_dbt_builtins, Some(false));
        assert_eq!(
            typed.templater.jinja.templater_paths,
            vec!["macro1".to_string(), "macro2".to_string()]
        );
        assert_eq!(
            typed.templater.jinja.loader_search_path,
            expected_loader_paths
        );

        let comma = typed.layout.types.get("comma").unwrap();
        assert_eq!(comma.spacing_before.as_deref(), Some("touch"));
        assert_eq!(comma.line_position.as_deref(), Some("trailing"));
    }
}
