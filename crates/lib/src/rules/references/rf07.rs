use hashbrown::{HashMap, HashSet};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::utils::analysis::select::get_select_statement_info;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RuleRF07 {
    force_enable: bool,
}

fn identifier_key(identifier: &ErasedSegment) -> (bool, String) {
    let quoted = identifier.is_type(SyntaxKind::QuotedIdentifier);
    let normalized = identifier.raw_normalized().to_string();
    if quoted {
        (true, normalized)
    } else {
        (false, normalized.to_lowercase())
    }
}

impl Rule for RuleRF07 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(Self {
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "references.window_alias"
    }

    fn description(&self) -> &'static str {
        "Do not reference a column alias inside its own OVER clause."
    }

    fn long_description(&self) -> &'static str {
        r#"
Window functions are evaluated before SELECT-list aliases are applied. An
unqualified reference in PARTITION BY or ORDER BY which matches an alias may
instead resolve to a column from a joined table. This rule is disabled by
default; set `force_enable = true` to enable it.

**Anti-pattern**

```sql
SELECT t1.col1 AS id,
       ROW_NUMBER() OVER (PARTITION BY id ORDER BY t1.ts) AS rn
FROM t1 LEFT JOIN t2 ON t1.col1 = t2.id
```

**Best practice**

```sql
SELECT t1.col1 AS id,
       ROW_NUMBER() OVER (PARTITION BY t1.col1 ORDER BY t1.ts) AS rn
FROM t1 LEFT JOIN t2 ON t1.col1 = t2.id
```"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::References]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if !self.force_enable {
            return Vec::new();
        }

        let Some(select) = context
            .parent_stack
            .iter()
            .rev()
            .find(|parent| parent.is_type(SyntaxKind::SelectStatement))
        else {
            return Vec::new();
        };
        let Some(info) = get_select_statement_info(select, Some(context.dialect), false) else {
            return Vec::new();
        };
        if info.table_aliases.len() <= 1 {
            return Vec::new();
        }

        let mut aliases = HashSet::new();
        for element in select.recursive_crawl(
            const { &SyntaxSet::single(SyntaxKind::SelectClauseElement) },
            true,
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            true,
        ) {
            for alias in element.recursive_crawl(
                const { &SyntaxSet::single(SyntaxKind::AliasExpression) },
                true,
                const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
                true,
            ) {
                for identifier in alias.recursive_crawl(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::Identifier,
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::QuotedIdentifier,
                        ])
                    },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                ) {
                    aliases.insert(identifier_key(&identifier));
                }
            }
        }
        if aliases.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for clause in context.segment.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::PartitionbyClause, SyntaxKind::OrderbyClause]) },
            true,
            &SyntaxSet::EMPTY,
            true,
        ) {
            for reference in clause.recursive_crawl(
                const { &SyntaxSet::single(SyntaxKind::ColumnReference) },
                true,
                const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
                true,
            ) {
                if reference.reference().is_qualified() {
                    continue;
                }
                let identifier = reference.recursive_crawl(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::Identifier,
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::QuotedIdentifier,
                        ])
                    },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                );
                if identifier
                    .first()
                    .is_some_and(|identifier| aliases.contains(&identifier_key(identifier)))
                {
                    results.push(LintResult::new(
                        Some(reference.clone()),
                        Vec::new(),
                        Some(format!(
                            "Reference {:?} in window clause matches a select alias but resolves to a table column. Qualify it with its table.",
                            reference.raw()
                        )),
                        None,
                    ));
                }
            }
        }
        results
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::single(SyntaxKind::WindowSpecification) })
            .into()
    }
}
