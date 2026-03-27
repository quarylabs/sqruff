use std::cell::RefCell;

use hashbrown::{HashMap, HashSet};
use smol_str::SmolStr;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::common::AliasInfo;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::parser::segments::object_reference::ObjectReferenceLevel;
use sqruff_lib_core::utils::analysis::query::{Query, QueryInner};
use sqruff_lib_core::utils::analysis::select::{
    SelectStatementColumnsAndTables, get_select_statement_info,
};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Default, Clone)]
struct AL05QueryData {
    aliases: Vec<AliasInfo>,
    tbl_refs: Vec<SmolStr>,
}

type QueryKey<'a> = *const RefCell<QueryInner<'a>>;
type AL05State<'a> = HashMap<QueryKey<'a>, AL05QueryData>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum AliasCaseCheck {
    #[default]
    Dialect,
    CaseInsensitive,
    QuotedCaseSensitiveNakedUpper,
    QuotedCaseSensitiveNakedLower,
    CaseSensitive,
}

impl AliasCaseCheck {
    fn from_config(value: &str) -> Result<Self, String> {
        match value {
            "dialect" => Ok(Self::Dialect),
            "case_insensitive" => Ok(Self::CaseInsensitive),
            "quoted_cs_naked_upper" => Ok(Self::QuotedCaseSensitiveNakedUpper),
            "quoted_cs_naked_lower" => Ok(Self::QuotedCaseSensitiveNakedLower),
            "case_sensitive" => Ok(Self::CaseSensitive),
            other => Err(format!("Invalid alias_case_check value: {other}")),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RuleAL05 {
    alias_case_check: AliasCaseCheck,
}

impl Rule for RuleAL05 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL05 {
            alias_case_check: AliasCaseCheck::from_config(
                config["alias_case_check"].as_string().unwrap(),
            )?,
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.unused"
    }

    fn description(&self) -> &'static str {
        "Tables should not be aliased if that alias is not used."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, alias `zoo` is not used.

```sql
SELECT
    a
FROM foo AS zoo
```

**Best practice**

Use the alias or remove it. An unused alias makes code harder to read without changing any functionality.

```sql
SELECT
    zoo.a
FROM foo AS zoo

-- Alternatively...

SELECT
    a
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut violations = Vec::new();
        let select_info = get_select_statement_info(&context.segment, context.dialect.into(), true);

        let Some(select_info) = select_info else {
            return Vec::new();
        };

        if select_info.table_aliases.is_empty() {
            return Vec::new();
        }

        let query = Query::from_segment(&context.segment, context.dialect, None);
        let mut payloads = AL05State::default();
        self.analyze_table_aliases(query.clone(), context.dialect, &mut payloads);

        let payload = payloads.get(&query.id()).cloned().unwrap_or_default();

        if matches!(
            context.dialect.name,
            DialectKind::Redshift | DialectKind::Bigquery
        ) {
            let mut references: HashSet<SmolStr> = HashSet::new();
            let mut aliases: HashSet<SmolStr> = HashSet::new();

            for alias in &payload.aliases {
                aliases.insert(self.alias_name(alias, context.dialect.name));
                if let Some(alias_segment) = &alias.segment {
                    aliases.insert(self.normalize_identifier(alias_segment, context.dialect.name));
                }
                if let Some(object_reference) = &alias.object_reference {
                    for seg in object_reference.segments() {
                        if const {
                            SyntaxSet::new(&[
                                SyntaxKind::Identifier,
                                SyntaxKind::NakedIdentifier,
                                SyntaxKind::QuotedIdentifier,
                                SyntaxKind::ObjectReference,
                            ])
                        }
                        .contains(seg.get_type())
                        {
                            references.insert(self.normalize_identifier(seg, context.dialect.name));
                        }
                    }
                }
            }

            if aliases.intersection(&references).next().is_some() {
                return Vec::new();
            }
        }

        let mut ref_counter: HashMap<SmolStr, usize> = HashMap::new();
        for alias in &payload.aliases {
            let Some(object_reference) = &alias.object_reference else {
                continue;
            };
            let Some(last_segment) = object_reference.segments().last() else {
                continue;
            };

            *ref_counter
                .entry(self.normalize_identifier(last_segment, context.dialect.name))
                .or_default() += 1;
        }

        for alias in &payload.aliases {
            if Self::is_alias_required(&alias.from_expression_element, context.dialect.name) {
                continue;
            }

            if let Some(object_reference) = &alias.object_reference
                && let Some(last_segment) = object_reference.segments().last()
                && ref_counter
                    .get(&self.normalize_identifier(last_segment, context.dialect.name))
                    .copied()
                    .unwrap_or_default()
                    > 1
            {
                continue;
            }

            if context.dialect.name == DialectKind::Redshift
                && alias.alias_expression.is_some()
                && self.followed_by_qualify(context, alias)
            {
                continue;
            }

            if self.has_function_alias_reference(alias, &select_info, context.dialect.name) {
                continue;
            }

            if alias.aliased
                && !payload
                    .tbl_refs
                    .contains(&self.alias_name(alias, context.dialect.name))
            {
                if Self::has_used_column_aliases(alias, &select_info, context.dialect.name) {
                    continue;
                }
                let violation = self.report_unused_alias(alias);
                violations.push(violation);
            }
        }

        violations
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}

impl RuleAL05 {
    #[allow(clippy::only_used_in_recursion)]
    fn analyze_table_aliases<'a>(
        &self,
        query: Query<'a>,
        dialect: &Dialect,
        payloads: &mut AL05State<'a>,
    ) {
        payloads.entry(query.id()).or_default();
        let selectables = std::mem::take(&mut RefCell::borrow_mut(&query.inner).selectables);

        for selectable in &selectables {
            if let Some(select_info) = selectable.select_info() {
                let table_aliases = select_info.table_aliases;
                let reference_buffer = select_info.reference_buffer;
                let table_reference_buffer = select_info.table_reference_buffer;

                payloads
                    .entry(query.id())
                    .or_default()
                    .aliases
                    .extend(table_aliases);

                for r in reference_buffer.into_iter().chain(table_reference_buffer) {
                    for tr in
                        r.extract_possible_references(ObjectReferenceLevel::Table, dialect.name)
                    {
                        self.resolve_and_mark_reference(query.clone(), &tr, dialect.name, payloads);
                    }
                }
            }
        }

        RefCell::borrow_mut(&query.inner).selectables = selectables;

        for child in query.children() {
            self.analyze_table_aliases(child, dialect, payloads);
        }
    }

    fn resolve_and_mark_reference<'a>(
        &self,
        query: Query<'a>,
        reference: &sqruff_lib_core::parser::segments::object_reference::ObjectReferencePart,
        dialect: DialectKind,
        payloads: &mut AL05State<'a>,
    ) {
        let Some(reference_segment) = reference.segments.first() else {
            return;
        };
        let normalized_ref = self.normalize_identifier(reference_segment, dialect);

        if let Some(payload) = payloads.get_mut(&query.id())
            && payload
                .aliases
                .iter()
                .any(|it| self.alias_name(it, dialect) == normalized_ref)
        {
            payload.tbl_refs.push(normalized_ref);
            return;
        }

        if let Some(parent) = RefCell::borrow(&query.inner).parent.clone() {
            self.resolve_and_mark_reference(parent, reference, dialect, payloads);
        }
    }

    fn is_alias_required(
        from_expression_element: &ErasedSegment,
        dialect_name: DialectKind,
    ) -> bool {
        for segment in from_expression_element
            .iter_segments(const { &SyntaxSet::new(&[SyntaxKind::Bracketed]) }, false)
        {
            if segment.is_type(SyntaxKind::TableExpression) {
                return if segment
                    .child(const { &SyntaxSet::new(&[SyntaxKind::ValuesClause]) })
                    .is_some()
                {
                    matches!(
                        dialect_name,
                        DialectKind::Athena
                            | DialectKind::Snowflake
                            | DialectKind::Tsql
                            | DialectKind::Postgres
                    )
                } else {
                    segment
                        .iter_segments(const { &SyntaxSet::new(&[SyntaxKind::Bracketed]) }, false)
                        .iter()
                        .any(|seg| {
                            const {
                                SyntaxSet::new(&[
                                    SyntaxKind::SelectStatement,
                                    SyntaxKind::SetExpression,
                                    SyntaxKind::WithCompoundStatement,
                                ])
                            }
                            .contains(seg.get_type())
                        })
                };
            }
        }
        false
    }

    fn has_used_column_aliases(
        alias: &AliasInfo,
        select_info: &SelectStatementColumnsAndTables,
        dialect_name: DialectKind,
    ) -> bool {
        let Some(alias_expression) = &alias.alias_expression else {
            return false;
        };

        // Look for a Bracketed child in the alias expression (the column alias
        // list, e.g. `(value)` or `(c1, c2)`).
        let Some(bracketed) =
            alias_expression.child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
        else {
            return false;
        };

        // Collect all identifier names from the bracketed column alias list.
        let col_alias_names: Vec<SmolStr> = bracketed
            .recursive_crawl(
                const {
                    &SyntaxSet::new(&[
                        SyntaxKind::NakedIdentifier,
                        SyntaxKind::Identifier,
                        SyntaxKind::QuotedIdentifier,
                    ])
                },
                true,
                &SyntaxSet::EMPTY,
                true,
            )
            .into_iter()
            .map(|seg| seg.raw().to_uppercase().into())
            .collect();

        if col_alias_names.is_empty() {
            return false;
        }

        // Check if any *unqualified* reference (or one qualified with this
        // alias) has an Object-level part matching a column alias name.
        // Qualified references like `o.value` belong to table `o`, not to our
        // alias, so they must not count as usage of the column alias list.
        for reference in &select_info.reference_buffer {
            let table_refs =
                reference.extract_possible_references(ObjectReferenceLevel::Table, dialect_name);
            if let Some(tbl) = table_refs.first() {
                // Qualified reference — only count it if the qualifier is our
                // own table alias.
                if tbl.part.to_uppercase() != alias.ref_str.to_uppercase() {
                    continue;
                }
            }
            // Unqualified reference (no table part) or qualified with our alias.
            for obj_ref in
                reference.extract_possible_references(ObjectReferenceLevel::Object, dialect_name)
            {
                if col_alias_names.contains(&SmolStr::from(obj_ref.part.to_uppercase())) {
                    return true;
                }
            }
        }

        false
    }

    fn has_function_alias_reference(
        &self,
        alias: &AliasInfo,
        select_info: &SelectStatementColumnsAndTables,
        dialect_name: DialectKind,
    ) -> bool {
        let Some(table_expression) = alias
            .from_expression_element
            .child(const { &SyntaxSet::single(SyntaxKind::TableExpression) })
        else {
            return false;
        };

        if table_expression
            .child(const { &SyntaxSet::single(SyntaxKind::Function) })
            .is_none()
        {
            return false;
        }

        let alias_name = self.alias_name(alias, dialect_name);

        select_info.reference_buffer.iter().any(|reference| {
            let references = reference.iter_raw_references();
            if references.len() != 1 {
                return false;
            }

            references
                .first()
                .and_then(|reference_part| reference_part.segments.first())
                .is_some_and(|segment| {
                    self.normalize_identifier(segment, dialect_name) == alias_name
                })
        })
    }

    fn report_unused_alias(&self, alias: &AliasInfo) -> LintResult {
        let mut fixes = vec![LintFix::delete(alias.alias_expression.clone().unwrap())];

        // Delete contiguous whitespace/meta immediately preceding the alias expression
        // without allocating intermediate Segments collections.
        if let Some(alias_idx) = alias
            .from_expression_element
            .segments()
            .iter()
            .position(|s| s == alias.alias_expression.as_ref().unwrap())
        {
            for seg in alias.from_expression_element.segments()[..alias_idx]
                .iter()
                .rev()
                .take_while(|s| s.is_whitespace() || s.is_meta())
            {
                fixes.push(LintFix::delete(seg.clone()));
            }
        }

        LintResult::new(
            alias.segment.clone(),
            fixes,
            format!(
                "Alias '{}' is never used in SELECT statement.",
                self.display_alias_name(alias)
            )
            .into(),
            None,
        )
    }

    fn alias_name(&self, alias: &AliasInfo, dialect: DialectKind) -> SmolStr {
        alias
            .segment
            .as_ref()
            .map(|segment| self.normalize_identifier(segment, dialect))
            .unwrap_or_else(|| self.normalize_identifier_str(&alias.ref_str, None, dialect))
    }

    fn display_alias_name(&self, alias: &AliasInfo) -> String {
        alias
            .segment
            .as_ref()
            .map(|segment| {
                self.normalize_identifier_str(
                    segment.raw(),
                    Some(segment.get_type()),
                    DialectKind::Ansi,
                )
            })
            .unwrap_or_else(|| {
                self.normalize_identifier_str(&alias.ref_str, None, DialectKind::Ansi)
            })
            .to_string()
    }

    fn normalize_identifier(&self, identifier: &ErasedSegment, dialect: DialectKind) -> SmolStr {
        self.normalize_identifier_str(identifier.raw(), Some(identifier.get_type()), dialect)
    }

    fn normalize_identifier_str(
        &self,
        raw: &str,
        syntax_kind: Option<SyntaxKind>,
        dialect: DialectKind,
    ) -> SmolStr {
        let is_naked = syntax_kind.is_none_or(|kind| {
            matches!(kind, SyntaxKind::Identifier | SyntaxKind::NakedIdentifier)
        }) && !matches!(
            raw.chars().next(),
            Some('"') | Some('\'') | Some('`') | Some('[')
        );
        let mut normalized = if raw.starts_with('[') && raw.ends_with(']') && raw.len() >= 2 {
            raw[1..raw.len() - 1].to_string()
        } else if matches!(raw.chars().next(), Some('"') | Some('\'') | Some('`'))
            && raw.len() >= 2
            && raw.chars().next() == raw.chars().last()
        {
            let quote = raw.chars().next().unwrap();
            raw[1..raw.len() - 1].replace(&format!("{quote}{quote}"), &quote.to_string())
        } else {
            raw.to_string()
        };

        match self.alias_case_check {
            AliasCaseCheck::Dialect => {
                if is_naked {
                    normalized = match dialect {
                        DialectKind::Postgres | DialectKind::Redshift => normalized.to_lowercase(),
                        _ => normalized.to_uppercase(),
                    };
                }
            }
            AliasCaseCheck::CaseInsensitive => normalized = normalized.to_uppercase(),
            AliasCaseCheck::QuotedCaseSensitiveNakedUpper => {
                if is_naked {
                    normalized = normalized.to_uppercase();
                }
            }
            AliasCaseCheck::QuotedCaseSensitiveNakedLower => {
                if is_naked {
                    normalized = normalized.to_lowercase();
                }
            }
            AliasCaseCheck::CaseSensitive => {}
        }

        normalized.into()
    }

    fn followed_by_qualify(&self, context: &RuleContext, alias: &AliasInfo) -> bool {
        let Some(alias_expression) = &alias.alias_expression else {
            return false;
        };
        let mut current_from_seen = false;

        for seg in context.segment.segments() {
            if alias_expression.get_end_loc() == seg.get_end_loc() {
                current_from_seen = true;
            } else if current_from_seen && !seg.is_code() {
                continue;
            } else if current_from_seen && seg.is_type(SyntaxKind::QualifyClause) {
                return true;
            } else if current_from_seen {
                return false;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    const POSTGRES_JSON_ALIAS_REPRODUCER: &str = r#"with stanza as (
    select
        data -> 'name' as name,
        data -> 'backup' -> (
            jsonb_array_length(data -> 'backup') - 1
        ) as last_backup,
        data -> 'archive' -> (
            jsonb_array_length(data -> 'archive') - 1
        ) as current_archive
    from jsonb_array_elements(monitor.pgbackrest_info()) as data
)

select
    name,
    to_timestamp(
        (last_backup -> 'timestamp' ->> 'stop')::numeric
    ) as last_successful_backup,
    current_archive ->> 'max' as last_archived_wal
from stanza;
"#;

    fn postgres_al05_linter() -> Linter {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
rules = AL05
dialect = postgres
"#,
            None,
        );

        Linter::new(config, None, None, true).unwrap()
    }

    #[test]
    fn test_al05_postgres_json_operator_alias_is_used() {
        let mut linter = postgres_al05_linter();
        let linted = linter
            .lint_string_wrapped(POSTGRES_JSON_ALIAS_REPRODUCER, false)
            .unwrap();

        assert_eq!(linted.violations(), &[]);
    }

    #[test]
    fn test_al05_postgres_json_operator_fix_preserves_alias() {
        let mut linter = postgres_al05_linter();
        let linted = linter
            .lint_string_wrapped(POSTGRES_JSON_ALIAS_REPRODUCER, true)
            .unwrap();

        assert_eq!(linted.fix_string(), POSTGRES_JSON_ALIAS_REPRODUCER);
    }
}
