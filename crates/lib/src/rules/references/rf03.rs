use std::cell::RefCell;

use ahash::{AHashMap, AHashSet};
use itertools::Itertools;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::common::{AliasInfo, ColumnAliasInfo};
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::helpers::capitalize;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder, Tables};
use sqruff_lib_core::parser::segments::object_reference::ObjectReferenceSegment;
use sqruff_lib_core::utils::analysis::query::Query;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Clone, Default)]
pub struct RuleRF03 {
    single_table_references: Option<String>,
    force_enable: bool,
}

impl RuleRF03 {
    fn visit_queries(
        tables: &Tables,
        single_table_references: &str,
        is_struct_dialect: bool,
        query: Query<()>,
        _visited: &mut AHashSet<ErasedSegment>,
    ) -> Vec<LintResult> {
        #[allow(unused_assignments)]
        let mut select_info = None;

        let mut acc = Vec::new();
        let selectables = &RefCell::borrow(&query.inner).selectables;

        if !selectables.is_empty() {
            select_info = selectables[0].select_info();

            if let Some(select_info) = select_info
                .clone()
                .filter(|select_info| select_info.table_aliases.len() == 1)
            {
                let mut fixable = true;
                let possible_ref_tables = iter_available_targets(query.clone());

                if let Some(_parent) = &RefCell::borrow(&query.inner).parent {}

                if possible_ref_tables.len() > 1 {
                    fixable = false;
                }

                let results = check_references(
                    tables,
                    select_info.table_aliases,
                    select_info.standalone_aliases,
                    select_info.reference_buffer,
                    select_info.col_aliases,
                    single_table_references,
                    is_struct_dialect,
                    Some("qualified".into()),
                    fixable,
                );

                acc.extend(results);
            }
        }

        let children = query.children();
        for child in children {
            acc.extend(Self::visit_queries(
                tables,
                single_table_references,
                is_struct_dialect,
                child,
                _visited,
            ));
        }

        acc
    }
}

fn iter_available_targets(query: Query<()>) -> Vec<SmolStr> {
    RefCell::borrow(&query.inner)
        .selectables
        .iter()
        .flat_map(|selectable| {
            selectable
                .select_info()
                .unwrap()
                .table_aliases
                .iter()
                .map(|alias| alias.ref_str.clone())
                .collect_vec()
        })
        .collect_vec()
}

#[allow(clippy::too_many_arguments)]
fn check_references(
    tables: &Tables,
    table_aliases: Vec<AliasInfo>,
    standalone_aliases: Vec<SmolStr>,
    references: Vec<ObjectReferenceSegment>,
    col_aliases: Vec<ColumnAliasInfo>,
    single_table_references: &str,
    is_struct_dialect: bool,
    fix_inconsistent_to: Option<String>,
    fixable: bool,
) -> Vec<LintResult> {
    let mut acc = Vec::new();

    let col_alias_names = col_aliases
        .clone()
        .into_iter()
        .map(|it| it.alias_identifier_name)
        .collect_vec();

    let table_ref_str = &table_aliases[0].ref_str;
    let table_ref_str_source = table_aliases[0].segment.clone();
    let mut seen_ref_types = AHashSet::new();

    for reference in references.clone() {
        let mut this_ref_type = reference.qualification();
        if this_ref_type == "qualified"
            && is_struct_dialect
            && &reference
                .iter_raw_references()
                .into_iter()
                .next()
                .unwrap()
                .part
                != table_ref_str
        {
            this_ref_type = "unqualified";
        }

        let lint_res = validate_one_reference(
            tables,
            single_table_references,
            reference,
            this_ref_type,
            &standalone_aliases,
            table_ref_str,
            table_ref_str_source.clone(),
            &col_alias_names,
            &seen_ref_types,
            fixable,
        );

        seen_ref_types.insert(this_ref_type);
        let Some(lint_res) = lint_res else {
            continue;
        };

        if let Some(fix_inconsistent_to) = fix_inconsistent_to
            .as_ref()
            .filter(|_| single_table_references == "consistent")
        {
            let results = check_references(
                tables,
                table_aliases.clone(),
                standalone_aliases.clone(),
                references.clone(),
                col_aliases.clone(),
                fix_inconsistent_to,
                is_struct_dialect,
                None,
                fixable,
            );

            acc.extend(results);
        }

        acc.push(lint_res);
    }

    acc
}

#[allow(clippy::too_many_arguments)]
fn validate_one_reference(
    tables: &Tables,
    single_table_references: &str,
    ref_: ObjectReferenceSegment,
    this_ref_type: &str,
    standalone_aliases: &[SmolStr],
    table_ref_str: &str,
    _table_ref_str_source: Option<ErasedSegment>,
    col_alias_names: &[SmolStr],
    seen_ref_types: &AHashSet<&str>,
    fixable: bool,
) -> Option<LintResult> {
    if !ref_.is_qualified() && ref_.0.is_type(SyntaxKind::WildcardIdentifier) {
        return None;
    }

    if standalone_aliases.contains(ref_.0.raw()) {
        return None;
    }

    if table_ref_str.is_empty() {
        return None;
    }

    if col_alias_names.contains(ref_.0.raw()) {
        return None;
    }

    if single_table_references == "consistent" {
        return if !seen_ref_types.is_empty() && !seen_ref_types.contains(this_ref_type) {
            LintResult::new(
                ref_.clone().0.into(),
                Vec::new(),
                format!(
                    "{} reference '{}' found in single table select which is inconsistent with \
                     previous references.",
                    capitalize(this_ref_type),
                    ref_.0.raw()
                )
                .into(),
                None,
            )
            .into()
        } else {
            None
        };
    }

    if single_table_references == this_ref_type {
        return None;
    }

    if single_table_references == "unqualified" {
        let fixes = if fixable {
            ref_.0
                .segments()
                .iter()
                .take(2)
                .cloned()
                .map(LintFix::delete)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        return LintResult::new(
            ref_.0.clone().into(),
            fixes,
            format!(
                "{} reference '{}' found in single table select.",
                capitalize(this_ref_type),
                ref_.0.raw()
            )
            .into(),
            None,
        )
        .into();
    }

    let ref_ = ref_.0.clone();
    let fixes = if fixable {
        vec![LintFix::create_before(
            if !ref_.segments().is_empty() {
                ref_.segments()[0].clone()
            } else {
                ref_.clone()
            },
            vec![
                SegmentBuilder::token(tables.next_id(), table_ref_str, SyntaxKind::NakedIdentifier)
                    .finish(),
                SegmentBuilder::symbol(tables.next_id(), "."),
            ],
        )]
    } else {
        Vec::new()
    };

    LintResult::new(
        ref_.clone().into(),
        fixes,
        format!(
            "{} reference '{}' found in single table select.",
            capitalize(this_ref_type),
            ref_.raw()
        )
        .into(),
        None,
    )
    .into()
}

impl Rule for RuleRF03 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleRF03 {
            single_table_references: config
                .get("single_table_references")
                .and_then(|it| it.as_string().map(ToString::to_string)),
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "references.consistent"
    }

    fn description(&self) -> &'static str {
        "References should be consistent in statements with a single table."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, only the field b is referenced.

```sql
SELECT
    a,
    foo.b
FROM foo
```

**Best practice**

Add or remove references to all fields.

```sql
SELECT
    a,
    b
FROM foo

-- Also good

SELECT
    foo.a,
    foo.b
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::References]
    }

    fn force_enable(&self) -> bool {
        self.force_enable
    }

    fn dialect_skip(&self) -> &'static [DialectKind] {
        // TODO: add hive
        &[DialectKind::Bigquery, DialectKind::Redshift]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let single_table_references =
            self.single_table_references.as_deref().unwrap_or_else(|| {
                context.config.raw["rules"]["single_table_references"]
                    .as_string()
                    .unwrap()
            });

        let query: Query<()> = Query::from_segment(&context.segment, context.dialect, None);
        let mut visited: AHashSet<ErasedSegment> = AHashSet::new();
        let is_struct_dialect = self.dialect_skip().contains(&context.dialect.name);

        Self::visit_queries(
            context.tables,
            single_table_references,
            is_struct_dialect,
            query,
            &mut visited,
        )
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::SelectStatement,
                    SyntaxKind::SetExpression,
                    SyntaxKind::WithCompoundStatement,
                ])
            },
        )
        .disallow_recurse()
        .into()
    }
}
