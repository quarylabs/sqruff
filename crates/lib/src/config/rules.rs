use std::str::FromStr;

use regex::Regex;
use serde::Deserialize;

use super::de;
use super::error::ConfigError;
use super::setting::{Merge, NullableSetting, Setting};

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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleConfigsPatch {
    pub allow_scalar: Setting<bool>,
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub single_table_references: Setting<SingleTableReferencesPolicy>,
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub unquoted_identifiers_policy: Setting<IdentifierPolicy>,
    pub indent_unit: Setting<String>,
    pub tab_space_size: Setting<usize>,

    #[serde(rename = "aliasing.table")]
    pub aliasing_table: AliasingConfigPatch,
    #[serde(rename = "aliasing.column")]
    pub aliasing_column: AliasingConfigPatch,
    #[serde(rename = "aliasing.unused")]
    pub aliasing_unused: AliasingUnusedConfigPatch,
    #[serde(rename = "aliasing.length")]
    pub aliasing_length: AliasingLengthConfigPatch,
    #[serde(rename = "aliasing.forbid")]
    pub aliasing_forbid: ForceEnableConfigPatch,
    #[serde(rename = "ambiguous.join")]
    pub ambiguous_join: AmbiguousJoinConfigPatch,
    #[serde(rename = "ambiguous.column_references")]
    pub ambiguous_column_references: AmbiguousColumnReferencesConfigPatch,
    #[serde(rename = "capitalisation.keywords")]
    pub capitalisation_keywords: CapitalisationKeywordsConfigPatch,
    #[serde(rename = "capitalisation.identifiers")]
    pub capitalisation_identifiers: CapitalisationIdentifiersConfigPatch,
    #[serde(rename = "capitalisation.functions")]
    pub capitalisation_functions: CapitalisationFunctionsConfigPatch,
    #[serde(rename = "capitalisation.literals")]
    pub capitalisation_literals: CapitalisationLiteralsConfigPatch,
    #[serde(rename = "capitalisation.types")]
    pub capitalisation_types: CapitalisationTypesConfigPatch,
    #[serde(rename = "convention.select_trailing_comma")]
    pub convention_select_trailing_comma: ConventionSelectTrailingCommaConfigPatch,
    #[serde(rename = "convention.count_rows")]
    pub convention_count_rows: ConventionCountRowsConfigPatch,
    #[serde(rename = "convention.terminator")]
    pub convention_terminator: ConventionTerminatorConfigPatch,
    #[serde(rename = "convention.blocked_words")]
    pub convention_blocked_words: ConventionBlockedWordsConfigPatch,
    #[serde(rename = "convention.quoted_literals")]
    pub convention_quoted_literals: ConventionQuotedLiteralsConfigPatch,
    #[serde(rename = "convention.casting_style")]
    pub convention_casting_style: ConventionCastingStyleConfigPatch,
    #[serde(rename = "convention.not_equal")]
    pub convention_not_equal: ConventionNotEqualConfigPatch,
    #[serde(rename = "references.from")]
    pub references_from: ForceEnableConfigPatch,
    #[serde(rename = "references.qualification")]
    pub references_qualification: ReferencesQualificationConfigPatch,
    #[serde(rename = "references.consistent")]
    pub references_consistent: ReferencesConsistentConfigPatch,
    #[serde(rename = "references.keywords")]
    pub references_keywords: ReferencesKeywordsConfigPatch,
    #[serde(rename = "references.special_chars")]
    pub references_special_chars: ReferencesSpecialCharsConfigPatch,
    #[serde(rename = "references.quoting")]
    pub references_quoting: ReferencesQuotingConfigPatch,
    #[serde(rename = "layout.long_lines")]
    pub layout_long_lines: LayoutLongLinesConfigPatch,
    #[serde(rename = "layout.select_targets")]
    pub layout_select_targets: LayoutSelectTargetsConfigPatch,
    #[serde(rename = "layout.newlines")]
    pub layout_newlines: LayoutNewlinesConfigPatch,
    #[serde(rename = "structure.subquery")]
    pub structure_subquery: StructureSubqueryConfigPatch,
    #[serde(rename = "structure.join_condition_order")]
    pub structure_join_condition_order: StructureJoinConditionOrderConfigPatch,
}

impl RuleConfigsPatch {
    pub(crate) fn merge_global(
        &mut self,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        validate_rule_options(section_name, values)?;
        self.merge(de::deserialize_section(section_name, values)?);
        Ok(())
    }

    pub(crate) fn merge_rule_section(
        &mut self,
        rule_section: String,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        validate_rule_options(section_name, values)?;
        match rule_section.as_str() {
            "aliasing.table" => self
                .aliasing_table
                .merge(de::deserialize_section(section_name, values)?),
            "aliasing.column" => self
                .aliasing_column
                .merge(de::deserialize_section(section_name, values)?),
            "aliasing.unused" => self
                .aliasing_unused
                .merge(de::deserialize_section(section_name, values)?),
            "aliasing.length" => self
                .aliasing_length
                .merge(de::deserialize_section(section_name, values)?),
            "aliasing.forbid" => self
                .aliasing_forbid
                .merge(de::deserialize_section(section_name, values)?),
            "ambiguous.join" => self
                .ambiguous_join
                .merge(de::deserialize_section(section_name, values)?),
            "ambiguous.column_references" => self
                .ambiguous_column_references
                .merge(de::deserialize_section(section_name, values)?),
            "capitalisation.keywords" => self
                .capitalisation_keywords
                .merge(de::deserialize_section(section_name, values)?),
            "capitalisation.identifiers" => self
                .capitalisation_identifiers
                .merge(de::deserialize_section(section_name, values)?),
            "capitalisation.functions" => self
                .capitalisation_functions
                .merge(de::deserialize_section(section_name, values)?),
            "capitalisation.literals" => self
                .capitalisation_literals
                .merge(de::deserialize_section(section_name, values)?),
            "capitalisation.types" => self
                .capitalisation_types
                .merge(de::deserialize_section(section_name, values)?),
            "convention.select_trailing_comma" => self
                .convention_select_trailing_comma
                .merge(de::deserialize_section(section_name, values)?),
            "convention.count_rows" => self
                .convention_count_rows
                .merge(de::deserialize_section(section_name, values)?),
            "convention.terminator" => self
                .convention_terminator
                .merge(de::deserialize_section(section_name, values)?),
            "convention.blocked_words" => self
                .convention_blocked_words
                .merge(de::deserialize_section(section_name, values)?),
            "convention.quoted_literals" => self
                .convention_quoted_literals
                .merge(de::deserialize_section(section_name, values)?),
            "convention.casting_style" => self
                .convention_casting_style
                .merge(de::deserialize_section(section_name, values)?),
            "convention.not_equal" => self
                .convention_not_equal
                .merge(de::deserialize_section(section_name, values)?),
            "references.from" => self
                .references_from
                .merge(de::deserialize_section(section_name, values)?),
            "references.qualification" => self
                .references_qualification
                .merge(de::deserialize_section(section_name, values)?),
            "references.consistent" => self
                .references_consistent
                .merge(de::deserialize_section(section_name, values)?),
            "references.keywords" => self
                .references_keywords
                .merge(de::deserialize_section(section_name, values)?),
            "references.special_chars" => self
                .references_special_chars
                .merge(de::deserialize_section(section_name, values)?),
            "references.quoting" => self
                .references_quoting
                .merge(de::deserialize_section(section_name, values)?),
            "layout.long_lines" => self
                .layout_long_lines
                .merge(de::deserialize_section(section_name, values)?),
            "layout.select_targets" => self
                .layout_select_targets
                .merge(de::deserialize_section(section_name, values)?),
            "layout.newlines" => self
                .layout_newlines
                .merge(de::deserialize_section(section_name, values)?),
            "structure.subquery" => self
                .structure_subquery
                .merge(de::deserialize_section(section_name, values)?),
            "structure.join_condition_order" => self
                .structure_join_condition_order
                .merge(de::deserialize_section(section_name, values)?),
            _ => return Err(ConfigError::UnknownSection(section_name.to_string())),
        }
        Ok(())
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
        self.allow_scalar.merge(other.allow_scalar);
        self.single_table_references
            .merge(other.single_table_references);
        self.unquoted_identifiers_policy
            .merge(other.unquoted_identifiers_policy);
        self.indent_unit.merge(other.indent_unit);
        self.tab_space_size.merge(other.tab_space_size);
        self.aliasing_table.merge(other.aliasing_table);
        self.aliasing_column.merge(other.aliasing_column);
        self.aliasing_unused.merge(other.aliasing_unused);
        self.aliasing_length.merge(other.aliasing_length);
        self.aliasing_forbid.merge(other.aliasing_forbid);
        self.ambiguous_join.merge(other.ambiguous_join);
        self.ambiguous_column_references
            .merge(other.ambiguous_column_references);
        self.capitalisation_keywords
            .merge(other.capitalisation_keywords);
        self.capitalisation_identifiers
            .merge(other.capitalisation_identifiers);
        self.capitalisation_functions
            .merge(other.capitalisation_functions);
        self.capitalisation_literals
            .merge(other.capitalisation_literals);
        self.capitalisation_types.merge(other.capitalisation_types);
        self.convention_select_trailing_comma
            .merge(other.convention_select_trailing_comma);
        self.convention_count_rows
            .merge(other.convention_count_rows);
        self.convention_terminator
            .merge(other.convention_terminator);
        self.convention_blocked_words
            .merge(other.convention_blocked_words);
        self.convention_quoted_literals
            .merge(other.convention_quoted_literals);
        self.convention_casting_style
            .merge(other.convention_casting_style);
        self.convention_not_equal.merge(other.convention_not_equal);
        self.references_from.merge(other.references_from);
        self.references_qualification
            .merge(other.references_qualification);
        self.references_consistent
            .merge(other.references_consistent);
        self.references_keywords.merge(other.references_keywords);
        self.references_special_chars
            .merge(other.references_special_chars);
        self.references_quoting.merge(other.references_quoting);
        self.layout_long_lines.merge(other.layout_long_lines);
        self.layout_select_targets
            .merge(other.layout_select_targets);
        self.layout_newlines.merge(other.layout_newlines);
        self.structure_subquery.merge(other.structure_subquery);
        self.structure_join_condition_order
            .merge(other.structure_join_condition_order);
    }
}

macro_rules! impl_merge {
    ($ty:ty { $($field:ident),* $(,)? }) => {
        impl Merge for $ty {
            fn merge(&mut self, other: Self) {
                $(self.$field.merge(other.$field);)*
            }
        }
    };
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct AliasingConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub aliasing: Setting<AliasingStyle>,
}
impl_merge!(AliasingConfigPatch { aliasing });

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct AliasingUnusedConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub alias_case_check: Setting<AliasCaseCheckPolicy>,
}
impl_merge!(AliasingUnusedConfigPatch { alias_case_check });

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct AliasingLengthConfigPatch {
    pub min_alias_length: NullableSetting<usize>,
    pub max_alias_length: NullableSetting<usize>,
}
impl_merge!(AliasingLengthConfigPatch {
    min_alias_length,
    max_alias_length
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ForceEnableConfigPatch {
    pub force_enable: Setting<bool>,
}
impl_merge!(ForceEnableConfigPatch { force_enable });

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct AmbiguousJoinConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub fully_qualify_join_types: Setting<JoinQualificationPolicy>,
}
impl_merge!(AmbiguousJoinConfigPatch {
    fully_qualify_join_types
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct AmbiguousColumnReferencesConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub group_by_and_order_by_style: Setting<GroupByAndOrderByStyle>,
}
impl_merge!(AmbiguousColumnReferencesConfigPatch {
    group_by_and_order_by_style
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct CapitalisationKeywordsConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub capitalisation_policy: Setting<CapitalisationPolicy>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
}
impl_merge!(CapitalisationKeywordsConfigPatch {
    capitalisation_policy,
    ignore_words,
    ignore_words_regex
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct CapitalisationIdentifiersConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub extended_capitalisation_policy: Setting<CapitalisationPolicy>,
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub unquoted_identifiers_policy: Setting<IdentifierPolicy>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
}
impl_merge!(CapitalisationIdentifiersConfigPatch {
    extended_capitalisation_policy,
    unquoted_identifiers_policy,
    ignore_words,
    ignore_words_regex
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct CapitalisationFunctionsConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub extended_capitalisation_policy: Setting<CapitalisationPolicy>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
}
impl_merge!(CapitalisationFunctionsConfigPatch {
    extended_capitalisation_policy,
    ignore_words,
    ignore_words_regex
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct CapitalisationLiteralsConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub capitalisation_policy: Setting<CapitalisationPolicy>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
}
impl_merge!(CapitalisationLiteralsConfigPatch {
    capitalisation_policy,
    ignore_words,
    ignore_words_regex
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct CapitalisationTypesConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub extended_capitalisation_policy: Setting<CapitalisationPolicy>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
}
impl_merge!(CapitalisationTypesConfigPatch {
    extended_capitalisation_policy,
    ignore_words,
    ignore_words_regex
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ConventionSelectTrailingCommaConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub select_clause_trailing_comma: Setting<SelectClauseTrailingComma>,
}
impl_merge!(ConventionSelectTrailingCommaConfigPatch {
    select_clause_trailing_comma
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ConventionCountRowsConfigPatch {
    pub prefer_count_1: Setting<bool>,
    pub prefer_count_0: Setting<bool>,
}
impl_merge!(ConventionCountRowsConfigPatch {
    prefer_count_1,
    prefer_count_0
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ConventionTerminatorConfigPatch {
    pub multiline_newline: Setting<bool>,
    pub require_final_semicolon: Setting<bool>,
}
impl_merge!(ConventionTerminatorConfigPatch {
    multiline_newline,
    require_final_semicolon
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ConventionBlockedWordsConfigPatch {
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub blocked_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub blocked_regex: Setting<Vec<String>>,
    pub match_source: Setting<bool>,
}
impl_merge!(ConventionBlockedWordsConfigPatch {
    blocked_words,
    blocked_regex,
    match_source
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ConventionQuotedLiteralsConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub preferred_quoted_literal_style: Setting<QuotedLiteralStyle>,
    pub force_enable: Setting<bool>,
}
impl_merge!(ConventionQuotedLiteralsConfigPatch {
    preferred_quoted_literal_style,
    force_enable
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ConventionCastingStyleConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub preferred_type_casting_style: Setting<TypeCastingStyle>,
}
impl_merge!(ConventionCastingStyleConfigPatch {
    preferred_type_casting_style
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ConventionNotEqualConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub preferred_not_equal_style: Setting<PreferredNotEqualStyle>,
}
impl_merge!(ConventionNotEqualConfigPatch {
    preferred_not_equal_style
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ReferencesQualificationConfigPatch {
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
}
impl_merge!(ReferencesQualificationConfigPatch {
    ignore_words,
    ignore_words_regex
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ReferencesConsistentConfigPatch {
    pub force_enable: Setting<bool>,
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub single_table_references: Setting<SingleTableReferencesPolicy>,
}
impl_merge!(ReferencesConsistentConfigPatch {
    force_enable,
    single_table_references
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ReferencesKeywordsConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub unquoted_identifiers_policy: Setting<IdentifierPolicy>,
    #[serde(default, deserialize_with = "de::nullable_setting_from_str")]
    pub quoted_identifiers_policy: NullableSetting<IdentifierPolicy>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
}
impl_merge!(ReferencesKeywordsConfigPatch {
    unquoted_identifiers_policy,
    quoted_identifiers_policy,
    ignore_words,
    ignore_words_regex
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ReferencesSpecialCharsConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub unquoted_identifiers_policy: Setting<IdentifierPolicy>,
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub quoted_identifiers_policy: Setting<IdentifierPolicy>,
    pub allow_space_in_identifier: Setting<bool>,
    pub additional_allowed_characters: NullableSetting<String>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
}
impl_merge!(ReferencesSpecialCharsConfigPatch {
    unquoted_identifiers_policy,
    quoted_identifiers_policy,
    allow_space_in_identifier,
    additional_allowed_characters,
    ignore_words,
    ignore_words_regex
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ReferencesQuotingConfigPatch {
    pub prefer_quoted_identifiers: Setting<bool>,
    pub prefer_quoted_keywords: Setting<bool>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words: Setting<Vec<String>>,
    #[serde(default, deserialize_with = "de::setting_csv")]
    pub ignore_words_regex: Setting<Vec<String>>,
    pub force_enable: Setting<bool>,
}
impl_merge!(ReferencesQuotingConfigPatch {
    prefer_quoted_identifiers,
    prefer_quoted_keywords,
    ignore_words,
    ignore_words_regex,
    force_enable
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LayoutLongLinesConfigPatch {
    pub ignore_comment_lines: Setting<bool>,
    pub ignore_comment_clauses: Setting<bool>,
}
impl_merge!(LayoutLongLinesConfigPatch {
    ignore_comment_lines,
    ignore_comment_clauses
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LayoutSelectTargetsConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub wildcard_policy: Setting<WildcardPolicy>,
}
impl_merge!(LayoutSelectTargetsConfigPatch { wildcard_policy });

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LayoutNewlinesConfigPatch {
    pub maximum_empty_lines_between_statements: Setting<usize>,
    pub maximum_empty_lines_inside_statements: Setting<usize>,
}
impl_merge!(LayoutNewlinesConfigPatch {
    maximum_empty_lines_between_statements,
    maximum_empty_lines_inside_statements
});

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct StructureSubqueryConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub forbid_subquery_in: Setting<SubqueryPolicy>,
}
impl_merge!(StructureSubqueryConfigPatch { forbid_subquery_in });

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct StructureJoinConditionOrderConfigPatch {
    #[serde(default, deserialize_with = "de::setting_from_str")]
    pub preferred_first_table_in_join_clause: Setting<JoinConditionOrderPolicy>,
}
impl_merge!(StructureJoinConditionOrderConfigPatch {
    preferred_first_table_in_join_clause
});

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
    pub(super) fn from_patch(patch: &RuleConfigsPatch) -> Result<Self, ConfigError> {
        let global = GlobalRuleConfig {
            allow_scalar: required(&patch.allow_scalar, "allow_scalar", "rules")?,
            single_table_references: required(
                &patch.single_table_references,
                "single_table_references",
                "rules",
            )?,
            unquoted_identifiers_policy: required(
                &patch.unquoted_identifiers_policy,
                "unquoted_identifiers_policy",
                "rules",
            )?,
        };

        Ok(Self {
            global: global.clone(),
            aliasing: AliasingRuleConfigs {
                table: AliasingConfig {
                    aliasing: required(
                        &patch.aliasing_table.aliasing,
                        "aliasing",
                        "aliasing.table",
                    )?,
                },
                column: AliasingConfig {
                    aliasing: required(
                        &patch.aliasing_column.aliasing,
                        "aliasing",
                        "aliasing.column",
                    )?,
                },
                unused: AliasingUnusedConfig {
                    alias_case_check: required(
                        &patch.aliasing_unused.alias_case_check,
                        "alias_case_check",
                        "aliasing.unused",
                    )?,
                },
                length: AliasingLengthConfig {
                    min_alias_length: patch
                        .aliasing_length
                        .min_alias_length
                        .clone()
                        .into_option()
                        .flatten(),
                    max_alias_length: patch
                        .aliasing_length
                        .max_alias_length
                        .clone()
                        .into_option()
                        .flatten(),
                },
                forbid: AliasingForbidConfig {
                    force_enable: required(
                        &patch.aliasing_forbid.force_enable,
                        "force_enable",
                        "aliasing.forbid",
                    )?,
                },
            },
            ambiguous: AmbiguousRuleConfigs {
                join: AmbiguousJoinConfig {
                    fully_qualify_join_types: required(
                        &patch.ambiguous_join.fully_qualify_join_types,
                        "fully_qualify_join_types",
                        "ambiguous.join",
                    )?,
                },
                column_references: AmbiguousColumnReferencesConfig {
                    group_by_and_order_by_style: required(
                        &patch
                            .ambiguous_column_references
                            .group_by_and_order_by_style,
                        "group_by_and_order_by_style",
                        "ambiguous.column_references",
                    )?,
                },
            },
            capitalisation: CapitalisationRuleConfigs {
                keywords: CapitalisationKeywordsConfig {
                    capitalisation_policy: required(
                        &patch.capitalisation_keywords.capitalisation_policy,
                        "capitalisation_policy",
                        "capitalisation.keywords",
                    )?,
                    ignore_words: lower_list(&patch.capitalisation_keywords.ignore_words),
                    ignore_words_regex: regex_list_values(
                        list(&patch.capitalisation_keywords.ignore_words_regex),
                        "ignore_words_regex",
                    )?,
                },
                identifiers: CapitalisationIdentifiersConfig {
                    extended_capitalisation_policy: required(
                        &patch
                            .capitalisation_identifiers
                            .extended_capitalisation_policy,
                        "extended_capitalisation_policy",
                        "capitalisation.identifiers",
                    )?,
                    unquoted_identifiers_policy: setting_or(
                        &patch.capitalisation_identifiers.unquoted_identifiers_policy,
                        global.unquoted_identifiers_policy,
                    ),
                    ignore_words: lower_list(&patch.capitalisation_identifiers.ignore_words),
                    ignore_words_regex: regex_list_values(
                        list(&patch.capitalisation_identifiers.ignore_words_regex),
                        "ignore_words_regex",
                    )?,
                },
                functions: CapitalisationFunctionsConfig {
                    extended_capitalisation_policy: required(
                        &patch
                            .capitalisation_functions
                            .extended_capitalisation_policy,
                        "extended_capitalisation_policy",
                        "capitalisation.functions",
                    )?,
                    ignore_words: lower_list(&patch.capitalisation_functions.ignore_words),
                    ignore_words_regex: regex_list_values(
                        list(&patch.capitalisation_functions.ignore_words_regex),
                        "ignore_words_regex",
                    )?,
                },
                literals: CapitalisationLiteralsConfig {
                    capitalisation_policy: required(
                        &patch.capitalisation_literals.capitalisation_policy,
                        "capitalisation_policy",
                        "capitalisation.literals",
                    )?,
                    ignore_words: lower_list(&patch.capitalisation_literals.ignore_words),
                    ignore_words_regex: regex_list_values(
                        list(&patch.capitalisation_literals.ignore_words_regex),
                        "ignore_words_regex",
                    )?,
                },
                types: CapitalisationTypesConfig {
                    extended_capitalisation_policy: required(
                        &patch.capitalisation_types.extended_capitalisation_policy,
                        "extended_capitalisation_policy",
                        "capitalisation.types",
                    )?,
                },
            },
            convention: ConventionRuleConfigs {
                select_trailing_comma: ConventionSelectTrailingCommaConfig {
                    select_clause_trailing_comma: required(
                        &patch
                            .convention_select_trailing_comma
                            .select_clause_trailing_comma,
                        "select_clause_trailing_comma",
                        "convention.select_trailing_comma",
                    )?,
                },
                count_rows: ConventionCountRowsConfig {
                    prefer_count_1: required(
                        &patch.convention_count_rows.prefer_count_1,
                        "prefer_count_1",
                        "convention.count_rows",
                    )?,
                    prefer_count_0: required(
                        &patch.convention_count_rows.prefer_count_0,
                        "prefer_count_0",
                        "convention.count_rows",
                    )?,
                },
                terminator: ConventionTerminatorConfig {
                    multiline_newline: required(
                        &patch.convention_terminator.multiline_newline,
                        "multiline_newline",
                        "convention.terminator",
                    )?,
                    require_final_semicolon: required(
                        &patch.convention_terminator.require_final_semicolon,
                        "require_final_semicolon",
                        "convention.terminator",
                    )?,
                },
                blocked_words: ConventionBlockedWordsConfig {
                    blocked_words: upper_list(&patch.convention_blocked_words.blocked_words),
                    blocked_regex: regex_list_values(
                        list(&patch.convention_blocked_words.blocked_regex),
                        "blocked_regex",
                    )?,
                    match_source: required(
                        &patch.convention_blocked_words.match_source,
                        "match_source",
                        "convention.blocked_words",
                    )?,
                },
                quoted_literals: ConventionQuotedLiteralsConfig {
                    preferred_quoted_literal_style: required(
                        &patch
                            .convention_quoted_literals
                            .preferred_quoted_literal_style,
                        "preferred_quoted_literal_style",
                        "convention.quoted_literals",
                    )?,
                    force_enable: required(
                        &patch.convention_quoted_literals.force_enable,
                        "force_enable",
                        "convention.quoted_literals",
                    )?,
                },
                casting_style: ConventionCastingStyleConfig {
                    preferred_type_casting_style: required(
                        &patch.convention_casting_style.preferred_type_casting_style,
                        "preferred_type_casting_style",
                        "convention.casting_style",
                    )?,
                },
                not_equal: ConventionNotEqualConfig {
                    preferred_not_equal_style: required(
                        &patch.convention_not_equal.preferred_not_equal_style,
                        "preferred_not_equal_style",
                        "convention.not_equal",
                    )?,
                },
            },
            jinja: JinjaRuleConfigs,
            layout: LayoutRuleConfigs {
                long_lines: LayoutLongLinesConfig {
                    ignore_comment_lines: required(
                        &patch.layout_long_lines.ignore_comment_lines,
                        "ignore_comment_lines",
                        "layout.long_lines",
                    )?,
                    ignore_comment_clauses: required(
                        &patch.layout_long_lines.ignore_comment_clauses,
                        "ignore_comment_clauses",
                        "layout.long_lines",
                    )?,
                },
                select_targets: LayoutSelectTargetsConfig {
                    wildcard_policy: required(
                        &patch.layout_select_targets.wildcard_policy,
                        "wildcard_policy",
                        "layout.select_targets",
                    )?,
                },
                newlines: LayoutNewlinesConfig {
                    maximum_empty_lines_between_statements: required(
                        &patch.layout_newlines.maximum_empty_lines_between_statements,
                        "maximum_empty_lines_between_statements",
                        "layout.newlines",
                    )?,
                    maximum_empty_lines_inside_statements: required(
                        &patch.layout_newlines.maximum_empty_lines_inside_statements,
                        "maximum_empty_lines_inside_statements",
                        "layout.newlines",
                    )?,
                },
            },
            references: ReferencesRuleConfigs {
                from: ReferencesFromConfig {
                    force_enable: required(
                        &patch.references_from.force_enable,
                        "force_enable",
                        "references.from",
                    )?,
                },
                qualification: ReferencesQualificationConfig {
                    ignore_words: lower_list(&patch.references_qualification.ignore_words),
                    ignore_words_regex: regex_list_values(
                        list(&patch.references_qualification.ignore_words_regex),
                        "ignore_words_regex",
                    )?,
                },
                consistent: ReferencesConsistentConfig {
                    force_enable: required(
                        &patch.references_consistent.force_enable,
                        "force_enable",
                        "references.consistent",
                    )?,
                    single_table_references: setting_or(
                        &patch.references_consistent.single_table_references,
                        global.single_table_references,
                    ),
                },
                keywords: ReferencesKeywordsConfig {
                    unquoted_identifiers_policy: setting_or(
                        &patch.references_keywords.unquoted_identifiers_policy,
                        global.unquoted_identifiers_policy,
                    ),
                    quoted_identifiers_policy: patch
                        .references_keywords
                        .quoted_identifiers_policy
                        .clone()
                        .into_option()
                        .flatten(),
                    ignore_words: lower_list(&patch.references_keywords.ignore_words),
                    ignore_words_regex: regex_list_values(
                        list(&patch.references_keywords.ignore_words_regex),
                        "ignore_words_regex",
                    )?,
                },
                special_chars: ReferencesSpecialCharsConfig {
                    unquoted_identifiers_policy: setting_or(
                        &patch.references_special_chars.unquoted_identifiers_policy,
                        global.unquoted_identifiers_policy,
                    ),
                    quoted_identifiers_policy: patch
                        .references_special_chars
                        .quoted_identifiers_policy
                        .clone()
                        .into_option()
                        .unwrap_or(IdentifierPolicy::None),
                    allow_space_in_identifier: required(
                        &patch.references_special_chars.allow_space_in_identifier,
                        "allow_space_in_identifier",
                        "references.special_chars",
                    )?,
                    additional_allowed_characters: patch
                        .references_special_chars
                        .additional_allowed_characters
                        .clone()
                        .into_option()
                        .flatten()
                        .unwrap_or_default(),
                    ignore_words: lower_list(&patch.references_special_chars.ignore_words),
                    ignore_words_regex: regex_list_values(
                        list(&patch.references_special_chars.ignore_words_regex),
                        "ignore_words_regex",
                    )?,
                },
                quoting: ReferencesQuotingConfig {
                    prefer_quoted_identifiers: required(
                        &patch.references_quoting.prefer_quoted_identifiers,
                        "prefer_quoted_identifiers",
                        "references.quoting",
                    )?,
                    prefer_quoted_keywords: required(
                        &patch.references_quoting.prefer_quoted_keywords,
                        "prefer_quoted_keywords",
                        "references.quoting",
                    )?,
                    ignore_words: lower_list(&patch.references_quoting.ignore_words),
                    ignore_words_regex: regex_list_values(
                        list(&patch.references_quoting.ignore_words_regex),
                        "ignore_words_regex",
                    )?,
                    force_enable: required(
                        &patch.references_quoting.force_enable,
                        "force_enable",
                        "references.quoting",
                    )?,
                },
            },
            structure: StructureRuleConfigs {
                subquery: StructureSubqueryConfig {
                    forbid_subquery_in: required(
                        &patch.structure_subquery.forbid_subquery_in,
                        "forbid_subquery_in",
                        "structure.subquery",
                    )?,
                },
                join_condition_order: StructureJoinConditionOrderConfig {
                    preferred_first_table_in_join_clause: required(
                        &patch
                            .structure_join_condition_order
                            .preferred_first_table_in_join_clause,
                        "preferred_first_table_in_join_clause",
                        "structure.join_condition_order",
                    )?,
                },
            },
        })
    }
}

fn required<T: Clone>(
    value: &Setting<T>,
    key: &'static str,
    section: &'static str,
) -> Result<T, ConfigError> {
    value
        .clone()
        .into_option()
        .ok_or_else(|| ConfigError::InvalidSection {
            section: section.to_string(),
            reason: format!("missing rule option '{key}'"),
        })
}

fn setting_or<T: Copy>(value: &Setting<T>, default: T) -> T {
    value.clone().into_option().unwrap_or(default)
}

fn list(value: &Setting<Vec<String>>) -> Vec<String> {
    value.clone().into_option().unwrap_or_default()
}

fn lower_list(value: &Setting<Vec<String>>) -> Vec<String> {
    list(value)
        .into_iter()
        .map(|value| value.to_lowercase())
        .collect()
}

fn upper_list(value: &Setting<Vec<String>>) -> Vec<String> {
    list(value)
        .into_iter()
        .map(|value| value.to_uppercase())
        .collect()
}

fn regex_list_values(values: Vec<String>, key: &'static str) -> Result<Vec<Regex>, ConfigError> {
    values
        .into_iter()
        .map(|pattern| {
            Regex::new(&pattern).map_err(|err| ConfigError::InvalidField {
                field: key,
                reason: err.to_string(),
            })
        })
        .collect()
}
