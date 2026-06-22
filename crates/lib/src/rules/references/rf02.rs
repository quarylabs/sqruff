use hashbrown::HashMap;
use itertools::Itertools;
use regex::Regex;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::common::{AliasInfo, ColumnAliasInfo};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::parser::segments::object_reference::ObjectReferenceSegment;
use sqruff_lib_core::utils::analysis::select::get_select_statement_info;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased as _, ErasedRule, LintResult, Rule, RuleGroups};
use crate::rules::aliasing::al04::RuleAL04;

#[derive(Clone, Debug, Default)]
pub struct RuleRF02Config {
    ignore_words: Vec<String>,
    ignore_words_regex: Vec<Regex>,
    subqueries_ignore_external_references: bool,
}

#[derive(Clone, Debug)]
pub struct RuleRF02 {
    base: RuleAL04<RuleRF02Config>,
}

impl Default for RuleRF02 {
    fn default() -> Self {
        Self {
            base: RuleAL04 {
                lint_references_and_aliases: Self::lint_references_and_aliases,
                context: RuleRF02Config::default(),
            },
        }
    }
}

impl Rule for RuleRF02 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let ignore_words = config["ignore_words"]
            .map(|it| {
                it.as_array()
                    .unwrap()
                    .iter()
                    .map(|it| it.as_string().unwrap().to_lowercase())
                    .collect()
            })
            .unwrap_or_default();

        let ignore_words_regex = config["ignore_words_regex"]
            .map(|it| {
                it.as_array()
                    .unwrap()
                    .iter()
                    .map(|it| Regex::new(it.as_string().unwrap()).unwrap())
                    .collect()
            })
            .unwrap_or_default();

        let subqueries_ignore_external_references = config["subqueries_ignore_external_references"]
            .as_bool()
            .unwrap_or(false);

        Ok(Self {
            base: RuleAL04 {
                lint_references_and_aliases: Self::lint_references_and_aliases,
                context: RuleRF02Config {
                    ignore_words,
                    ignore_words_regex,
                    subqueries_ignore_external_references,
                },
            },
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "references.qualification"
    }

    fn description(&self) -> &'static str {
        "References should be qualified if select has more than one referenced table/view."
    }

    fn long_description(&self) -> &'static str {
        r"
**Anti-pattern**

In this example, the reference `vee` has not been declared, and the variables `a` and `b` are potentially ambiguous.

```sql
SELECT a, b
FROM foo
LEFT JOIN vee ON vee.a = foo.a
```

**Best practice**

Add the references.

```sql
SELECT foo.a, vee.b
FROM foo
LEFT JOIN vee ON vee.a = foo.a
```
"
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::References]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        self.base.eval(context)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}

impl RuleRF02 {
    /// Determine if a subquery is part of the `from` clause.
    ///
    /// Any subqueries in the `from_clause` should be ignored, unless they are a
    /// nested correlated query (i.e. inside a `where_clause`).
    fn is_root_from_clause(rule_context: &RuleContext) -> bool {
        for x in rule_context.parent_stack.iter().rev() {
            if x.is_type(SyntaxKind::FromClause) {
                return true;
            } else if x.is_type(SyntaxKind::WhereClause) {
                return false;
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn lint_references_and_aliases(
        mut table_aliases: Vec<AliasInfo>,
        standalone_aliases: Vec<SmolStr>,
        references: Vec<ObjectReferenceSegment>,
        col_aliases: Vec<ColumnAliasInfo>,
        using_cols: Vec<SmolStr>,
        parent_select: Option<ErasedSegment>,
        rule_context: &RuleContext,
        context: &RuleRF02Config,
    ) -> Vec<LintResult> {
        let parent_select_info = parent_select.and_then(|parent| {
            get_select_statement_info(&parent, rule_context.dialect.into(), true)
        });
        if let Some(parent_select_info) = parent_select_info {
            // If we are looking at a subquery, include any table references
            // from the parent (outer) select.
            for table_alias in parent_select_info.table_aliases {
                let is_from = Self::is_root_from_clause(rule_context);
                if !table_alias
                    .from_expression_element
                    .path_to(&rule_context.segment)
                    .is_empty()
                    || is_from
                    || context.subqueries_ignore_external_references
                {
                    // Skip the subquery alias itself, or if the subquery is
                    // inside a `from`/`join` clause that isn't a nested
                    // `where` clause.
                    continue;
                }
                table_aliases.push(table_alias);
            }
        }

        if table_aliases.len() <= 1 {
            return Vec::new();
        }

        let mut violation_buff = Vec::new();
        for r in references {
            if context.ignore_words.contains(&r.0.raw().to_lowercase()) {
                continue;
            }

            if context
                .ignore_words_regex
                .iter()
                .any(|regex| regex.is_match(r.0.raw().as_ref()))
            {
                continue;
            }

            let this_ref_type = r.qualification();
            let col_alias_names = col_aliases
                .iter()
                .filter_map(|c| {
                    if !c.column_reference_segments.contains(&r.0) {
                        Some(c.alias_identifier_name.as_str())
                    } else {
                        None
                    }
                })
                .collect_vec();

            if this_ref_type == "unqualified"
                && !col_alias_names.contains(&r.0.raw().as_ref())
                && !using_cols.contains(r.0.raw())
                && !standalone_aliases.contains(r.0.raw())
            {
                violation_buff.push(LintResult::new(
                    r.0.clone().into(),
                    Vec::new(),
                    format!(
                        "Unqualified reference {} found in select with more than one referenced \
                         table/view.",
                        r.0.raw()
                    )
                    .into(),
                    None,
                ));
            }
        }

        violation_buff
    }
}
