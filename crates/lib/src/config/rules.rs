use std::str::FromStr;

use hashbrown::HashMap;
use regex::Regex;
use serde::Deserialize;

use super::de;
use super::error::ConfigError;
use super::raw::{RawConfig, Value};
use super::setting::Merge;

const KNOWN_RULE_OPTIONS: &[&str] = &[
    "additional_allowed_characters",
    "alias_case_check",
    "aliasing",
    "allow_scalar",
    "allow_space_in_identifier",
    "blocked_regex",
    "blocked_words",
    "capitalisation_policy",
    "extended_capitalisation_policy",
    "forbid_subquery_in",
    "force_enable",
    "fully_qualify_join_types",
    "group_by_and_order_by_style",
    "ignore_comment_clauses",
    "ignore_comment_lines",
    "ignore_words",
    "ignore_words_regex",
    "match_source",
    "max_alias_length",
    "maximum_empty_lines_between_statements",
    "maximum_empty_lines_inside_statements",
    "min_alias_length",
    "multiline_newline",
    "prefer_count_0",
    "prefer_count_1",
    "prefer_quoted_identifiers",
    "prefer_quoted_keywords",
    "preferred_first_table_in_join_clause",
    "preferred_not_equal_style",
    "preferred_quoted_literal_style",
    "preferred_type_casting_style",
    "quoted_identifiers_policy",
    "require_final_semicolon",
    "select_clause_trailing_comma",
    "single_table_references",
    "unquoted_identifiers_policy",
    "wildcard_policy",
];

macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident => $value:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    other => Err(format!("invalid {} '{other}'", stringify!($name))),
                }
            }
        }
    };
}

string_enum! {
    pub enum CapitalisationPolicy {
        Consistent => "consistent",
        Upper => "upper",
        Lower => "lower",
        Capitalise => "capitalise",
        Pascal => "pascal",
    }
}

string_enum! {
    pub enum IdentifierPolicy {
        All => "all",
        None => "none",
        Aliases => "aliases",
        ColumnAliases => "column_aliases",
        TableAliases => "table_aliases",
    }
}

string_enum! {
    pub enum SingleTableReferencesPolicy {
        Consistent => "consistent",
        Qualified => "qualified",
        Unqualified => "unqualified",
    }
}

string_enum! {
    pub enum AliasingStyle {
        Explicit => "explicit",
        Implicit => "implicit",
    }
}

string_enum! {
    pub enum AliasCaseCheckPolicy {
        Dialect => "dialect",
        CaseInsensitive => "case_insensitive",
        QuotedCaseSensitiveNakedUpper => "quoted_cs_naked_upper",
        QuotedCaseSensitiveNakedLower => "quoted_cs_naked_lower",
        CaseSensitive => "case_sensitive",
    }
}

string_enum! {
    pub enum JoinQualificationPolicy {
        Inner => "inner",
        Outer => "outer",
        Both => "both",
    }
}

string_enum! {
    pub enum GroupByAndOrderByStyle {
        Consistent => "consistent",
        Explicit => "explicit",
        Implicit => "implicit",
    }
}

string_enum! {
    pub enum SelectClauseTrailingComma {
        Require => "require",
        Forbid => "forbid",
    }
}

string_enum! {
    pub enum PreferredNotEqualStyle {
        Consistent => "consistent",
        CStyle => "c_style",
        Ansi => "ansi",
    }
}

string_enum! {
    pub enum QuotedLiteralStyle {
        Consistent => "consistent",
        SingleQuotes => "single_quotes",
        DoubleQuotes => "double_quotes",
    }
}

string_enum! {
    pub enum TypeCastingStyle {
        Consistent => "consistent",
        Cast => "cast",
        Convert => "convert",
        Shorthand => "shorthand",
        None => "none",
    }
}

string_enum! {
    pub enum WildcardPolicy {
        Single => "single",
        Multiple => "multiple",
    }
}

string_enum! {
    pub enum SubqueryPolicy {
        Join => "join",
        From => "from",
        Both => "both",
    }
}

string_enum! {
    pub enum JoinConditionOrderPolicy {
        Earlier => "earlier",
        Later => "later",
    }
}

/// Typed patch for the `[sqruff:rules]` section and per-rule subsections.
///
/// Unknown section names are allowed because rule sections are keyed by rule
/// name, but keys inside those sections must be known rule options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RuleConfigsPatch {
    #[serde(flatten)]
    pub configs: HashMap<String, Value>,
}

impl RuleConfigsPatch {
    pub(crate) fn merge_global(
        &mut self,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        validate_rule_options(section_name, values)?;
        self.configs
            .extend(de::deserialize_value_map(section_name, values)?);
        Ok(())
    }

    pub(crate) fn merge_rule_section(
        &mut self,
        rule_section: String,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        validate_rule_options(section_name, values)?;
        self.configs.insert(
            rule_section,
            Value::Map(de::deserialize_value_map(section_name, values)?),
        );
        Ok(())
    }

    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        if self.configs.is_empty() {
            return;
        }
        let rules = raw
            .entry("rules".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let rules_map = rules.as_map_mut().expect("rules must be a map");
        rules_map.extend(self.configs);
    }
}

fn validate_rule_options(
    section_name: &str,
    values: &std::collections::HashMap<String, Option<String>>,
) -> Result<(), ConfigError> {
    for key in values.keys() {
        if !KNOWN_RULE_OPTIONS.contains(&key.as_str()) {
            return Err(ConfigError::InvalidSection {
                section: section_name.to_string(),
                reason: format!("invalid rule option '{key}'"),
            });
        }
    }
    Ok(())
}

impl Merge for RuleConfigsPatch {
    fn merge(&mut self, other: Self) {
        self.configs.extend(other.configs);
    }
}

#[derive(Debug, Clone)]
pub struct RuleConfigs {
    pub global: GlobalRuleConfig,
    pub aliasing: AliasingRuleConfigs,
    pub ambiguous: AmbiguousRuleConfigs,
    pub capitalisation: CapitalisationRuleConfigs,
    pub convention: ConventionRuleConfigs,
    pub jinja: JinjaRuleConfigs,
    pub layout: LayoutRuleConfigs,
    pub references: ReferencesRuleConfigs,
    pub structure: StructureRuleConfigs,
}

#[derive(Debug, Clone)]
pub struct GlobalRuleConfig {
    pub allow_scalar: bool,
    pub single_table_references: SingleTableReferencesPolicy,
    pub unquoted_identifiers_policy: IdentifierPolicy,
}

#[derive(Debug, Clone)]
pub struct AliasingRuleConfigs {
    pub table: AliasingConfig,
    pub column: AliasingConfig,
    pub unused: AliasingUnusedConfig,
    pub length: AliasingLengthConfig,
    pub forbid: AliasingForbidConfig,
}

#[derive(Debug, Clone)]
pub struct AliasingConfig {
    pub aliasing: AliasingStyle,
}

#[derive(Debug, Clone)]
pub struct AliasingUnusedConfig {
    pub alias_case_check: AliasCaseCheckPolicy,
}

#[derive(Debug, Clone)]
pub struct AliasingLengthConfig {
    pub min_alias_length: Option<usize>,
    pub max_alias_length: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct AliasingForbidConfig {
    pub force_enable: bool,
}

#[derive(Debug, Clone)]
pub struct AmbiguousRuleConfigs {
    pub join: AmbiguousJoinConfig,
    pub column_references: AmbiguousColumnReferencesConfig,
}

#[derive(Debug, Clone)]
pub struct AmbiguousJoinConfig {
    pub fully_qualify_join_types: JoinQualificationPolicy,
}

#[derive(Debug, Clone)]
pub struct AmbiguousColumnReferencesConfig {
    pub group_by_and_order_by_style: GroupByAndOrderByStyle,
}

#[derive(Debug, Clone)]
pub struct CapitalisationRuleConfigs {
    pub keywords: CapitalisationKeywordsConfig,
    pub identifiers: CapitalisationIdentifiersConfig,
    pub functions: CapitalisationFunctionsConfig,
    pub literals: CapitalisationLiteralsConfig,
    pub types: CapitalisationTypesConfig,
}

#[derive(Debug, Clone)]
pub struct CapitalisationKeywordsConfig {
    pub capitalisation_policy: CapitalisationPolicy,
    pub ignore_words: Vec<String>,
    pub ignore_words_regex: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct CapitalisationIdentifiersConfig {
    pub extended_capitalisation_policy: CapitalisationPolicy,
    pub unquoted_identifiers_policy: IdentifierPolicy,
    pub ignore_words: Vec<String>,
    pub ignore_words_regex: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct CapitalisationFunctionsConfig {
    pub extended_capitalisation_policy: CapitalisationPolicy,
    pub ignore_words: Vec<String>,
    pub ignore_words_regex: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct CapitalisationLiteralsConfig {
    pub capitalisation_policy: CapitalisationPolicy,
    pub ignore_words: Vec<String>,
    pub ignore_words_regex: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct CapitalisationTypesConfig {
    pub extended_capitalisation_policy: CapitalisationPolicy,
}

#[derive(Debug, Clone)]
pub struct ConventionRuleConfigs {
    pub select_trailing_comma: ConventionSelectTrailingCommaConfig,
    pub count_rows: ConventionCountRowsConfig,
    pub terminator: ConventionTerminatorConfig,
    pub blocked_words: ConventionBlockedWordsConfig,
    pub quoted_literals: ConventionQuotedLiteralsConfig,
    pub casting_style: ConventionCastingStyleConfig,
    pub not_equal: ConventionNotEqualConfig,
}

#[derive(Debug, Clone)]
pub struct ConventionSelectTrailingCommaConfig {
    pub select_clause_trailing_comma: SelectClauseTrailingComma,
}

#[derive(Debug, Clone)]
pub struct ConventionCountRowsConfig {
    pub prefer_count_1: bool,
    pub prefer_count_0: bool,
}

#[derive(Debug, Clone)]
pub struct ConventionTerminatorConfig {
    pub multiline_newline: bool,
    pub require_final_semicolon: bool,
}

#[derive(Debug, Clone)]
pub struct ConventionBlockedWordsConfig {
    pub blocked_words: Vec<String>,
    pub blocked_regex: Vec<Regex>,
    pub match_source: bool,
}

#[derive(Debug, Clone)]
pub struct ConventionQuotedLiteralsConfig {
    pub preferred_quoted_literal_style: QuotedLiteralStyle,
    pub force_enable: bool,
}

#[derive(Debug, Clone)]
pub struct ConventionCastingStyleConfig {
    pub preferred_type_casting_style: TypeCastingStyle,
}

#[derive(Debug, Clone)]
pub struct ConventionNotEqualConfig {
    pub preferred_not_equal_style: PreferredNotEqualStyle,
}

#[derive(Debug, Clone, Default)]
pub struct JinjaRuleConfigs;

#[derive(Debug, Clone)]
pub struct LayoutRuleConfigs {
    pub long_lines: LayoutLongLinesConfig,
    pub select_targets: LayoutSelectTargetsConfig,
    pub newlines: LayoutNewlinesConfig,
}

#[derive(Debug, Clone)]
pub struct LayoutLongLinesConfig {
    pub ignore_comment_lines: bool,
    pub ignore_comment_clauses: bool,
}

#[derive(Debug, Clone)]
pub struct LayoutSelectTargetsConfig {
    pub wildcard_policy: WildcardPolicy,
}

#[derive(Debug, Clone)]
pub struct LayoutNewlinesConfig {
    pub maximum_empty_lines_between_statements: usize,
    pub maximum_empty_lines_inside_statements: usize,
}

#[derive(Debug, Clone)]
pub struct ReferencesRuleConfigs {
    pub from: ReferencesFromConfig,
    pub qualification: ReferencesQualificationConfig,
    pub consistent: ReferencesConsistentConfig,
    pub keywords: ReferencesKeywordsConfig,
    pub special_chars: ReferencesSpecialCharsConfig,
    pub quoting: ReferencesQuotingConfig,
}

#[derive(Debug, Clone)]
pub struct ReferencesFromConfig {
    pub force_enable: bool,
}

#[derive(Debug, Clone)]
pub struct ReferencesQualificationConfig {
    pub ignore_words: Vec<String>,
    pub ignore_words_regex: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct ReferencesConsistentConfig {
    pub force_enable: bool,
    pub single_table_references: SingleTableReferencesPolicy,
}

#[derive(Debug, Clone)]
pub struct ReferencesKeywordsConfig {
    pub unquoted_identifiers_policy: IdentifierPolicy,
    pub quoted_identifiers_policy: Option<IdentifierPolicy>,
    pub ignore_words: Vec<String>,
    pub ignore_words_regex: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct ReferencesSpecialCharsConfig {
    pub unquoted_identifiers_policy: IdentifierPolicy,
    pub quoted_identifiers_policy: IdentifierPolicy,
    pub allow_space_in_identifier: bool,
    pub additional_allowed_characters: String,
    pub ignore_words: Vec<String>,
    pub ignore_words_regex: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct ReferencesQuotingConfig {
    pub prefer_quoted_identifiers: bool,
    pub prefer_quoted_keywords: bool,
    pub ignore_words: Vec<String>,
    pub ignore_words_regex: Vec<Regex>,
    pub force_enable: bool,
}

#[derive(Debug, Clone)]
pub struct StructureRuleConfigs {
    pub subquery: StructureSubqueryConfig,
    pub join_condition_order: StructureJoinConditionOrderConfig,
}

#[derive(Debug, Clone)]
pub struct StructureSubqueryConfig {
    pub forbid_subquery_in: SubqueryPolicy,
}

#[derive(Debug, Clone)]
pub struct StructureJoinConditionOrderConfig {
    pub preferred_first_table_in_join_clause: JoinConditionOrderPolicy,
}

impl RuleConfigs {
    pub(super) fn from_raw(raw: &RawConfig) -> Result<Self, ConfigError> {
        let values = raw["rules"].as_map().unwrap();
        let global = GlobalRuleConfig::from_map(values)?;

        Ok(Self {
            global: global.clone(),
            aliasing: AliasingRuleConfigs::from_raw(values)?,
            ambiguous: AmbiguousRuleConfigs::from_raw(values)?,
            capitalisation: CapitalisationRuleConfigs::from_raw(values, &global)?,
            convention: ConventionRuleConfigs::from_raw(values)?,
            jinja: JinjaRuleConfigs,
            layout: LayoutRuleConfigs::from_raw(values)?,
            references: ReferencesRuleConfigs::from_raw(values, &global)?,
            structure: StructureRuleConfigs::from_raw(values)?,
        })
    }
}

impl GlobalRuleConfig {
    fn from_map(values: &HashMap<String, Value>) -> Result<Self, ConfigError> {
        Ok(Self {
            allow_scalar: required_bool(values, "allow_scalar", "rules")?,
            single_table_references: required_enum(values, "single_table_references", "rules")?,
            unquoted_identifiers_policy: required_enum(
                values,
                "unquoted_identifiers_policy",
                "rules",
            )?,
        })
    }
}

impl AliasingRuleConfigs {
    fn from_raw(values: &HashMap<String, Value>) -> Result<Self, ConfigError> {
        Ok(Self {
            table: AliasingConfig::from_map(
                &merged_rule_map(values, "aliasing.table"),
                "aliasing.table",
            )?,
            column: AliasingConfig::from_map(
                &merged_rule_map(values, "aliasing.column"),
                "aliasing.column",
            )?,
            unused: AliasingUnusedConfig::from_map(
                &merged_rule_map(values, "aliasing.unused"),
                "aliasing.unused",
            )?,
            length: AliasingLengthConfig::from_map(
                &merged_rule_map(values, "aliasing.length"),
                "aliasing.length",
            )?,
            forbid: AliasingForbidConfig::from_map(
                &merged_rule_map(values, "aliasing.forbid"),
                "aliasing.forbid",
            )?,
        })
    }
}

impl AliasingConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            aliasing: required_enum(values, "aliasing", section)?,
        })
    }
}

impl AliasingUnusedConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            alias_case_check: required_enum(values, "alias_case_check", section)?,
        })
    }
}

impl AliasingLengthConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        _section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            min_alias_length: optional_usize(values, "min_alias_length")?,
            max_alias_length: optional_usize(values, "max_alias_length")?,
        })
    }
}

impl AliasingForbidConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            force_enable: required_bool(values, "force_enable", section)?,
        })
    }
}

impl AmbiguousRuleConfigs {
    fn from_raw(values: &HashMap<String, Value>) -> Result<Self, ConfigError> {
        Ok(Self {
            join: AmbiguousJoinConfig::from_map(
                &merged_rule_map(values, "ambiguous.join"),
                "ambiguous.join",
            )?,
            column_references: AmbiguousColumnReferencesConfig::from_map(
                &merged_rule_map(values, "ambiguous.column_references"),
                "ambiguous.column_references",
            )?,
        })
    }
}

impl AmbiguousJoinConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            fully_qualify_join_types: required_enum(values, "fully_qualify_join_types", section)?,
        })
    }
}

impl AmbiguousColumnReferencesConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            group_by_and_order_by_style: required_enum(
                values,
                "group_by_and_order_by_style",
                section,
            )?,
        })
    }
}

impl CapitalisationRuleConfigs {
    fn from_raw(
        values: &HashMap<String, Value>,
        global: &GlobalRuleConfig,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            keywords: CapitalisationKeywordsConfig::from_map(
                &merged_rule_map(values, "capitalisation.keywords"),
                "capitalisation.keywords",
            )?,
            identifiers: CapitalisationIdentifiersConfig::from_map(
                &merged_rule_map(values, "capitalisation.identifiers"),
                "capitalisation.identifiers",
                global,
            )?,
            functions: CapitalisationFunctionsConfig::from_map(
                &merged_rule_map(values, "capitalisation.functions"),
                "capitalisation.functions",
            )?,
            literals: CapitalisationLiteralsConfig::from_map(
                &merged_rule_map(values, "capitalisation.literals"),
                "capitalisation.literals",
            )?,
            types: CapitalisationTypesConfig::from_map(
                &merged_rule_map(values, "capitalisation.types"),
                "capitalisation.types",
            )?,
        })
    }
}

impl CapitalisationKeywordsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            capitalisation_policy: required_enum(values, "capitalisation_policy", section)?,
            ignore_words: lower_string_list(values, "ignore_words")?,
            ignore_words_regex: regex_list(values, "ignore_words_regex")?,
        })
    }
}

impl CapitalisationIdentifiersConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
        global: &GlobalRuleConfig,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            extended_capitalisation_policy: required_enum(
                values,
                "extended_capitalisation_policy",
                section,
            )?,
            unquoted_identifiers_policy: optional_enum(values, "unquoted_identifiers_policy")?
                .unwrap_or(global.unquoted_identifiers_policy),
            ignore_words: lower_string_list(values, "ignore_words")?,
            ignore_words_regex: regex_list(values, "ignore_words_regex")?,
        })
    }
}

impl CapitalisationFunctionsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            extended_capitalisation_policy: required_enum(
                values,
                "extended_capitalisation_policy",
                section,
            )?,
            ignore_words: lower_string_list(values, "ignore_words")?,
            ignore_words_regex: regex_list(values, "ignore_words_regex")?,
        })
    }
}

impl CapitalisationLiteralsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            capitalisation_policy: required_enum(values, "capitalisation_policy", section)?,
            ignore_words: lower_string_list(values, "ignore_words")?,
            ignore_words_regex: regex_list(values, "ignore_words_regex")?,
        })
    }
}

impl CapitalisationTypesConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            extended_capitalisation_policy: required_enum(
                values,
                "extended_capitalisation_policy",
                section,
            )?,
        })
    }
}

impl ConventionRuleConfigs {
    fn from_raw(values: &HashMap<String, Value>) -> Result<Self, ConfigError> {
        Ok(Self {
            select_trailing_comma: ConventionSelectTrailingCommaConfig::from_map(
                &merged_rule_map(values, "convention.select_trailing_comma"),
                "convention.select_trailing_comma",
            )?,
            count_rows: ConventionCountRowsConfig::from_map(
                &merged_rule_map(values, "convention.count_rows"),
                "convention.count_rows",
            )?,
            terminator: ConventionTerminatorConfig::from_map(
                &merged_rule_map(values, "convention.terminator"),
                "convention.terminator",
            )?,
            blocked_words: ConventionBlockedWordsConfig::from_map(
                &merged_rule_map(values, "convention.blocked_words"),
                "convention.blocked_words",
            )?,
            quoted_literals: ConventionQuotedLiteralsConfig::from_map(
                &merged_rule_map(values, "convention.quoted_literals"),
                "convention.quoted_literals",
            )?,
            casting_style: ConventionCastingStyleConfig::from_map(
                &merged_rule_map(values, "convention.casting_style"),
                "convention.casting_style",
            )?,
            not_equal: ConventionNotEqualConfig::from_map(
                &merged_rule_map(values, "convention.not_equal"),
                "convention.not_equal",
            )?,
        })
    }
}

impl ConventionSelectTrailingCommaConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            select_clause_trailing_comma: required_enum(
                values,
                "select_clause_trailing_comma",
                section,
            )?,
        })
    }
}

impl ConventionCountRowsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            prefer_count_1: required_bool(values, "prefer_count_1", section)?,
            prefer_count_0: required_bool(values, "prefer_count_0", section)?,
        })
    }
}

impl ConventionTerminatorConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            multiline_newline: required_bool(values, "multiline_newline", section)?,
            require_final_semicolon: required_bool(values, "require_final_semicolon", section)?,
        })
    }
}

impl ConventionBlockedWordsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            blocked_words: upper_string_list(values, "blocked_words")?,
            blocked_regex: regex_list(values, "blocked_regex")?,
            match_source: required_bool(values, "match_source", section)?,
        })
    }
}

impl ConventionQuotedLiteralsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            preferred_quoted_literal_style: required_enum(
                values,
                "preferred_quoted_literal_style",
                section,
            )?,
            force_enable: required_bool(values, "force_enable", section)?,
        })
    }
}

impl ConventionCastingStyleConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            preferred_type_casting_style: required_enum(
                values,
                "preferred_type_casting_style",
                section,
            )?,
        })
    }
}

impl ConventionNotEqualConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            preferred_not_equal_style: required_enum(values, "preferred_not_equal_style", section)?,
        })
    }
}

impl LayoutRuleConfigs {
    fn from_raw(values: &HashMap<String, Value>) -> Result<Self, ConfigError> {
        Ok(Self {
            long_lines: LayoutLongLinesConfig::from_map(
                &merged_rule_map(values, "layout.long_lines"),
                "layout.long_lines",
            )?,
            select_targets: LayoutSelectTargetsConfig::from_map(
                &merged_rule_map(values, "layout.select_targets"),
                "layout.select_targets",
            )?,
            newlines: LayoutNewlinesConfig::from_map(
                &merged_rule_map(values, "layout.newlines"),
                "layout.newlines",
            )?,
        })
    }
}

impl LayoutLongLinesConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            ignore_comment_lines: required_bool(values, "ignore_comment_lines", section)?,
            ignore_comment_clauses: required_bool(values, "ignore_comment_clauses", section)?,
        })
    }
}

impl LayoutSelectTargetsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            wildcard_policy: required_enum(values, "wildcard_policy", section)?,
        })
    }
}

impl LayoutNewlinesConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            maximum_empty_lines_between_statements: required_usize(
                values,
                "maximum_empty_lines_between_statements",
                section,
            )?,
            maximum_empty_lines_inside_statements: required_usize(
                values,
                "maximum_empty_lines_inside_statements",
                section,
            )?,
        })
    }
}

impl ReferencesRuleConfigs {
    fn from_raw(
        values: &HashMap<String, Value>,
        global: &GlobalRuleConfig,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            from: ReferencesFromConfig::from_map(
                &merged_rule_map(values, "references.from"),
                "references.from",
            )?,
            qualification: ReferencesQualificationConfig::from_map(&merged_rule_map(
                values,
                "references.qualification",
            ))?,
            consistent: ReferencesConsistentConfig::from_map(
                &merged_rule_map(values, "references.consistent"),
                "references.consistent",
                global,
            )?,
            keywords: ReferencesKeywordsConfig::from_map(
                &merged_rule_map(values, "references.keywords"),
                "references.keywords",
                global,
            )?,
            special_chars: ReferencesSpecialCharsConfig::from_map(
                &merged_rule_map(values, "references.special_chars"),
                "references.special_chars",
                global,
            )?,
            quoting: ReferencesQuotingConfig::from_map(
                &merged_rule_map(values, "references.quoting"),
                "references.quoting",
            )?,
        })
    }
}

impl ReferencesFromConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            force_enable: required_bool(values, "force_enable", section)?,
        })
    }
}

impl ReferencesQualificationConfig {
    fn from_map(values: &HashMap<String, Value>) -> Result<Self, ConfigError> {
        Ok(Self {
            ignore_words: lower_string_list(values, "ignore_words")?,
            ignore_words_regex: regex_list(values, "ignore_words_regex")?,
        })
    }
}

impl ReferencesConsistentConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
        global: &GlobalRuleConfig,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            force_enable: required_bool(values, "force_enable", section)?,
            single_table_references: optional_enum(values, "single_table_references")?
                .unwrap_or(global.single_table_references),
        })
    }
}

impl ReferencesKeywordsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
        global: &GlobalRuleConfig,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            unquoted_identifiers_policy: optional_enum(values, "unquoted_identifiers_policy")?
                .unwrap_or(global.unquoted_identifiers_policy),
            quoted_identifiers_policy: optional_enum(values, "quoted_identifiers_policy")?,
            ignore_words: lower_string_list(values, "ignore_words")?,
            ignore_words_regex: regex_list(values, "ignore_words_regex")?,
        })
        .map_err(|err| match err {
            ConfigError::InvalidField { field, reason } => ConfigError::InvalidSection {
                section: section.to_string(),
                reason: format!("{field}: {reason}"),
            },
            err => err,
        })
    }
}

impl ReferencesSpecialCharsConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
        global: &GlobalRuleConfig,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            unquoted_identifiers_policy: optional_enum(values, "unquoted_identifiers_policy")?
                .unwrap_or(global.unquoted_identifiers_policy),
            quoted_identifiers_policy: optional_enum(values, "quoted_identifiers_policy")?
                .unwrap_or(IdentifierPolicy::None),
            allow_space_in_identifier: required_bool(values, "allow_space_in_identifier", section)?,
            additional_allowed_characters: optional_string(values, "additional_allowed_characters")
                .unwrap_or_default(),
            ignore_words: lower_string_list(values, "ignore_words")?,
            ignore_words_regex: regex_list(values, "ignore_words_regex")?,
        })
    }
}

impl ReferencesQuotingConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            prefer_quoted_identifiers: required_bool(values, "prefer_quoted_identifiers", section)?,
            prefer_quoted_keywords: required_bool(values, "prefer_quoted_keywords", section)?,
            ignore_words: lower_string_list(values, "ignore_words")?,
            ignore_words_regex: regex_list(values, "ignore_words_regex")?,
            force_enable: required_bool(values, "force_enable", section)?,
        })
    }
}

impl StructureRuleConfigs {
    fn from_raw(values: &HashMap<String, Value>) -> Result<Self, ConfigError> {
        Ok(Self {
            subquery: StructureSubqueryConfig::from_map(
                &merged_rule_map(values, "structure.subquery"),
                "structure.subquery",
            )?,
            join_condition_order: StructureJoinConditionOrderConfig::from_map(
                &merged_rule_map(values, "structure.join_condition_order"),
                "structure.join_condition_order",
            )?,
        })
    }
}

impl StructureSubqueryConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            forbid_subquery_in: required_enum(values, "forbid_subquery_in", section)?,
        })
    }
}

impl StructureJoinConditionOrderConfig {
    fn from_map(
        values: &HashMap<String, Value>,
        section: &'static str,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            preferred_first_table_in_join_clause: required_enum(
                values,
                "preferred_first_table_in_join_clause",
                section,
            )?,
        })
    }
}

fn merged_rule_map(
    values: &HashMap<String, Value>,
    rule_config_ref: &str,
) -> HashMap<String, Value> {
    let mut merged = values
        .iter()
        .filter(|(_, value)| !matches!(value, Value::Map(_)))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<HashMap<_, _>>();

    if let Some(specific) = values.get(rule_config_ref).and_then(Value::as_map) {
        merged.extend(specific.clone());
    }

    merged
}

fn required_value<'a>(
    values: &'a HashMap<String, Value>,
    key: &'static str,
    section: &'static str,
) -> Result<&'a Value, ConfigError> {
    values.get(key).ok_or_else(|| ConfigError::InvalidSection {
        section: section.to_string(),
        reason: format!("missing rule option '{key}'"),
    })
}

fn required_bool(
    values: &HashMap<String, Value>,
    key: &'static str,
    section: &'static str,
) -> Result<bool, ConfigError> {
    match required_value(values, key, section)? {
        Value::Bool(value) => Ok(*value),
        other => Err(ConfigError::InvalidSection {
            section: section.to_string(),
            reason: format!("invalid bool for '{key}': {other:?}"),
        }),
    }
}

fn required_usize(
    values: &HashMap<String, Value>,
    key: &'static str,
    section: &'static str,
) -> Result<usize, ConfigError> {
    match required_value(values, key, section)? {
        Value::Int(value) if *value >= 0 => Ok(*value as usize),
        other => Err(ConfigError::InvalidSection {
            section: section.to_string(),
            reason: format!("invalid integer for '{key}': {other:?}"),
        }),
    }
}

fn optional_usize(
    values: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Option<usize>, ConfigError> {
    match values.get(key) {
        None | Some(Value::None) => Ok(None),
        Some(Value::Int(value)) if *value >= 0 => Ok(Some(*value as usize)),
        Some(other) => Err(ConfigError::InvalidField {
            field: key,
            reason: format!("invalid integer: {other:?}"),
        }),
    }
}

fn optional_string(values: &HashMap<String, Value>, key: &'static str) -> Option<String> {
    match values.get(key) {
        Some(Value::String(value)) => Some(value.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        Some(Value::Int(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn required_enum<T>(
    values: &HashMap<String, Value>,
    key: &'static str,
    section: &'static str,
) -> Result<T, ConfigError>
where
    T: FromStr<Err = String>,
{
    let value = required_value(values, key, section)?;
    let raw = match value {
        Value::String(value) => value.as_ref(),
        Value::None => "none",
        other => {
            return Err(ConfigError::InvalidSection {
                section: section.to_string(),
                reason: format!("invalid value for '{key}': {other:?}"),
            });
        }
    };
    raw.parse().map_err(|reason| ConfigError::InvalidSection {
        section: section.to_string(),
        reason: format!("{key}: {reason}"),
    })
}

fn optional_enum<T>(
    values: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Option<T>, ConfigError>
where
    T: FromStr<Err = String>,
{
    let Some(value) = values.get(key) else {
        return Ok(None);
    };
    let raw = match value {
        Value::None => return Ok(None),
        Value::String(value) => value.as_ref(),
        other => {
            return Err(ConfigError::InvalidField {
                field: key,
                reason: format!("invalid value: {other:?}"),
            });
        }
    };
    raw.parse()
        .map(Some)
        .map_err(|reason| ConfigError::InvalidField { field: key, reason })
}

fn string_list(
    values: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Vec<String>, ConfigError> {
    let Some(value) = values.get(key) else {
        return Ok(Vec::new());
    };
    match value {
        Value::None => Ok(Vec::new()),
        Value::String(value) => Ok(value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect()),
        Value::Bool(value) => Ok(vec![value.to_string()]),
        Value::Int(value) => Ok(vec![value.to_string()]),
        Value::Array(values) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.to_string()),
                Value::Bool(value) => Ok(value.to_string()),
                Value::Int(value) => Ok(value.to_string()),
                other => Err(ConfigError::InvalidField {
                    field: key,
                    reason: format!("expected string, found {other:?}"),
                }),
            })
            .collect(),
        other => Err(ConfigError::InvalidField {
            field: key,
            reason: format!("expected string list, found {other:?}"),
        }),
    }
}

fn lower_string_list(
    values: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Vec<String>, ConfigError> {
    Ok(string_list(values, key)?
        .into_iter()
        .map(|value| value.to_lowercase())
        .collect())
}

fn upper_string_list(
    values: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Vec<String>, ConfigError> {
    Ok(string_list(values, key)?
        .into_iter()
        .map(|value| value.to_uppercase())
        .collect())
}

fn regex_list(
    values: &HashMap<String, Value>,
    key: &'static str,
) -> Result<Vec<Regex>, ConfigError> {
    string_list(values, key)?
        .into_iter()
        .map(|pattern| {
            Regex::new(&pattern).map_err(|err| ConfigError::InvalidField {
                field: key,
                reason: err.to_string(),
            })
        })
        .collect()
}
