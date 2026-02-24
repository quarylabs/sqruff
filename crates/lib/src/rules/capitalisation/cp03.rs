use hashbrown::HashMap;
use regex::Regex;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;

use super::clickhouse_function_casing::{
    canonical_clickhouse_function_name, is_clickhouse_case_insensitive_function,
};
use super::cp01::RuleCP01;
use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleCP03 {
    base: RuleCP01,
}

impl Default for RuleCP03 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                skip_literals: false,
                exclude_parent_types: &[],
                ..Default::default()
            },
        }
    }
}

impl Rule for RuleCP03 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCP03 {
            base: RuleCP01 {
                capitalisation_policy: config["extended_capitalisation_policy"]
                    .as_string()
                    .unwrap()
                    .into(),
                description_elem: "Function names",
                ignore_words: config["ignore_words"]
                    .map(|it| {
                        it.as_array()
                            .unwrap()
                            .iter()
                            .map(|it| it.as_string().unwrap().to_lowercase())
                            .collect()
                    })
                    .unwrap_or_default(),
                ignore_words_regex: config["ignore_words_regex"]
                    .map(|it| {
                        it.as_array()
                            .unwrap()
                            .iter()
                            .map(|it| Regex::new(it.as_string().unwrap()).unwrap())
                            .collect()
                    })
                    .unwrap_or_default(),

                ..Default::default()
            },
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "capitalisation.functions"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of function names."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the two `SUM` functions don’t have the same capitalisation.

```sql
SELECT
    sum(a) AS aa,
    SUM(b) AS bb
FROM foo
```

**Best practice**

Make the case consistent.


```sql
SELECT
    sum(a) AS aa,
    sum(b) AS bb
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[
            RuleGroups::All,
            RuleGroups::Core,
            RuleGroups::Capitalisation,
        ]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.dialect.name == DialectKind::Clickhouse {
            return self.eval_clickhouse(context);
        }

        self.base.eval(context)
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const {SyntaxSet::new(&[
            SyntaxKind::FunctionNameIdentifier,
            SyntaxKind::BareFunction,
        ]) })
        .into()
    }
}

impl RuleCP03 {
    fn eval_clickhouse(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.segment.raw().is_empty() || context.segment.is_templated() {
            return Vec::new();
        }

        let segment_raw = context.segment.raw();
        let Some(canonical_name) = canonical_clickhouse_function_name(&segment_raw) else {
            if is_clickhouse_case_insensitive_function(&segment_raw) {
                // Case-insensitive functions are safe to rewrite using configured policy.
                return self.base.eval(context);
            }

            // ClickHouse function names can be case-sensitive, so avoid unsafe case rewrites
            // for functions that are not explicitly classified.
            return Vec::new();
        };

        if segment_raw.as_str() == canonical_name {
            return Vec::new();
        }

        let fix = LintFix::replace(
            context.segment.clone(),
            vec![context.segment.edit(
                context.tables.next_id(),
                canonical_name.to_string().into(),
                None,
            )],
            None,
        );

        vec![LintResult::new(
            Some(context.segment.clone()),
            vec![fix],
            Some(format!(
                "Function names must use ClickHouse canonical case ('{canonical_name}')."
            )),
            None,
        )]
    }
}
