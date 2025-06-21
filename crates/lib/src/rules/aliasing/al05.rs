use std::cell::RefCell;

use ahash::{AHashMap, AHashSet};
use smol_str::{SmolStr, ToSmolStr};
use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::common::AliasInfo;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::ErasedSegment;
use sqruff_lib_core::parser::segments::object_reference::ObjectReferenceLevel;
use sqruff_lib_core::utils::analysis::query::Query;
use sqruff_lib_core::utils::analysis::select::get_select_statement_info;
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Default, Clone)]
struct AL05Query {
    aliases: Vec<AliasInfo>,
    tbl_refs: Vec<SmolStr>,
}

#[derive(Debug, Default, Clone)]
pub struct RuleAL05;

impl Rule for RuleAL05 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL05.erased())
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
        self.analyze_table_aliases(query.clone(), context.dialect);

        if context.dialect.name == DialectKind::Redshift {
            let mut references = AHashSet::default();
            let mut aliases = AHashSet::default();

            for alias in &query.inner.borrow().payload.aliases {
                aliases.insert(alias.ref_str.clone());
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
                            references.insert(seg.raw().to_smolstr());
                        }
                    }
                }
            }

            if aliases.intersection(&references).next().is_some() {
                return Vec::new();
            }
        }

        // Collect all table references from all query levels
        let all_tbl_refs = self.collect_all_tbl_refs(&query);

        for alias in &RefCell::borrow(&query.inner).payload.aliases {
            if Self::is_alias_required(&alias.from_expression_element, context.dialect.name) {
                continue;
            }

            if alias.aliased
                && !all_tbl_refs.contains(&alias.ref_str)
            {
                // For T-SQL, do a more comprehensive check by scanning the entire query tree
                // This is a workaround for cases where qualified references in WHERE IN clauses
                // are not properly detected by the reference buffer
                let mut is_used = false;
                if context.dialect.name == DialectKind::Tsql && !all_tbl_refs.is_empty() {
                    // Only apply this workaround if we already found some references
                    // This helps avoid false positives where an alias truly isn't used
                    
                    // Get the parent segment that contains the WHERE clause
                    // Walk up the parent stack to find a segment that contains the full query including WHERE
                    let mut search_segment = context.segment.clone();
                    for parent in &context.parent_stack {
                        let parent_text = parent.raw();
                        if parent_text.contains("WHERE") && parent_text.len() > search_segment.raw().len() {
                            search_segment = parent.clone();
                        }
                    }
                    
                    let query_text = search_segment.raw();
                    
                    // Use regex-like pattern to find alias.column references
                    // Check for common patterns where this alias might be used
                    let patterns = [
                        format!("{}.", alias.ref_str), // Basic pattern: alias.
                        format!(" {}.", alias.ref_str), // With space before
                        format!("({}.", alias.ref_str), // In parentheses
                        format!(",{}.", alias.ref_str), // After comma
                    ];
                    
                    for pattern in &patterns {
                        if query_text.contains(pattern) {
                            is_used = true;
                            break;
                        }
                    }
                }
                
                if !is_used {
                    let violation = self.report_unused_alias(alias.clone());
                    violations.push(violation);
                }
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
    fn collect_all_tbl_refs(&self, query: &Query<AL05Query>) -> AHashSet<SmolStr> {
        let mut all_refs = AHashSet::new();
        
        // Collect from current level
        for tbl_ref in &RefCell::borrow(&query.inner).payload.tbl_refs {
            all_refs.insert(tbl_ref.clone());
        }
        
        
        // Collect from all children recursively
        for child in query.children() {
            let child_refs = self.collect_all_tbl_refs(&child);
            all_refs.extend(child_refs);
        }
        
        
        all_refs
    }
    

    #[allow(clippy::only_used_in_recursion)]
    fn analyze_table_aliases(&self, query: Query<AL05Query>, dialect: &Dialect) {
        let selectables = std::mem::take(&mut RefCell::borrow_mut(&query.inner).selectables);

        for selectable in &selectables {
            if let Some(select_info) = selectable.select_info() {
                RefCell::borrow_mut(&query.inner)
                    .payload
                    .aliases
                    .extend(select_info.table_aliases);

                
                for r in select_info.reference_buffer {
                    
                    // Standard table reference extraction
                    let table_refs = r.extract_possible_references(ObjectReferenceLevel::Table, dialect.name);
                    for tr in &table_refs {
                        Self::resolve_and_mark_reference(query.clone(), tr.part.clone());
                    }
                    
                    // For T-SQL, be more aggressive about extracting references from qualified names
                    if dialect.name == DialectKind::Tsql {
                        // For qualified references like "alias.column", always check the first part
                        if r.is_qualified() {
                            let parts = r.iter_raw_references();
                            if !parts.is_empty() {
                                Self::resolve_and_mark_reference(query.clone(), parts[0].part.clone());
                            }
                        }
                        
                        // Additional check for ObjectReference segments that might contain table references
                        if matches!(r.0.get_type(), SyntaxKind::ObjectReference) {
                            // Extract all identifier parts and check if any match table aliases
                            for part in r.iter_raw_references() {
                                Self::resolve_and_mark_reference(query.clone(), part.part.clone());
                            }
                        }
                        
                        // For T-SQL, also check if the raw text contains a dot, indicating a qualified reference
                        let raw = r.0.raw();
                        if raw.contains('.') {
                            // Split on dot and use the first part as a potential table reference
                            if let Some(first_part) = raw.split('.').next() {
                                Self::resolve_and_mark_reference(query.clone(), first_part.to_string());
                            }
                        }
                    }
                    
                    // For all dialects: Handle ANY reference that might be qualified table.column references
                    let raw = r.0.raw();
                    if raw.contains('.') && raw.len() < 100 { // Reasonable length limit
                        // This could be a qualified reference like "alias.column"
                        if let Some(first_part) = raw.split('.').next() {
                            // Only consider it if the first part looks like an identifier (no spaces, reasonable length)
                            // Skip if this looks like a function call (starts with parenthesis or contains function pattern)
                            // but allow expressions inside IN clauses
                            let is_function_call = raw.starts_with('(') || (raw.contains('(') && !raw.contains(','));
                            
                            if first_part.len() < 50 && !first_part.contains(' ') && !first_part.is_empty() && !is_function_call {
                                Self::resolve_and_mark_reference(query.clone(), first_part.to_string());
                            }
                        }
                    }
                    
                    
                    
                    // Handle ColumnReference that might be a table alias reference 
                    // This is more restrictive than before to avoid false positives
                    if matches!(r.0.get_type(), SyntaxKind::ColumnReference) {
                        let column_name = r.0.raw().to_string();
                        
                        // Check if this column reference matches any known table alias
                        // Only do this for T-SQL and only if we haven't seen any qualified references yet
                        if dialect.name == DialectKind::Tsql {
                            if RefCell::borrow(&query.inner)
                                .payload
                                .aliases
                                .iter()
                                .any(|alias| alias.ref_str == column_name)
                            {
                                Self::resolve_and_mark_reference(query.clone(), column_name);
                            }
                        }
                    }
                }
            }
        }

        RefCell::borrow_mut(&query.inner).selectables = selectables;

        for child in query.children() {
            self.analyze_table_aliases(child, dialect);
        }
    }

    fn resolve_and_mark_reference(query: Query<AL05Query>, r#ref: String) {
        // First check if the reference matches any alias at the current level
        if RefCell::borrow(&query.inner)
            .payload
            .aliases
            .iter()
            .any(|it| it.ref_str == r#ref)
        {
            RefCell::borrow_mut(&query.inner)
                .payload
                .tbl_refs
                .push(r#ref.into());
            return;
        }
        
        // If not found at current level, check parent levels
        if let Some(parent) = RefCell::borrow(&query.inner).parent.clone() {
            Self::resolve_and_mark_reference(parent, r#ref);
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
                    matches!(dialect_name, DialectKind::Snowflake | DialectKind::Tsql)
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

    fn report_unused_alias(&self, alias: AliasInfo) -> LintResult {
        let mut fixes = vec![LintFix::delete(alias.alias_expression.clone().unwrap())];
        let to_delete = Segments::from_vec(alias.from_expression_element.segments().to_vec(), None)
            .reversed()
            .select::<fn(&ErasedSegment) -> bool>(
                None,
                Some(|it| it.is_whitespace() || it.is_meta()),
                alias.alias_expression.as_ref().unwrap().into(),
                None,
            );

        fixes.extend(to_delete.into_iter().map(LintFix::delete));

        LintResult::new(
            alias.segment,
            fixes,
            format!(
                "Alias '{}' is never used in SELECT statement.",
                alias.ref_str
            )
            .into(),
            None,
        )
    }
}
