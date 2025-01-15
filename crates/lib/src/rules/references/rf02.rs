use ahash::AHashMap;
use itertools::Itertools;
use regex::Regex;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::common::{AliasInfo, ColumnAliasInfo};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::object_reference::ObjectReferenceSegment;

use crate::core::config::Value;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::rules::aliasing::al04::RuleAL04;

#[derive(Clone, Debug)]
pub struct RuleRF02 {
    base: RuleAL04<(Vec<String>, Vec<Regex>)>,
}

impl Default for RuleRF02 {
    fn default() -> Self {
        Self {
            base: RuleAL04 {
                lint_references_and_aliases: Self::lint_references_and_aliases,
                context: (Vec::new(), Vec::new()),
            },
        }
    }
}

impl Rule for RuleRF02 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
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

        Ok(Self {
            base: RuleAL04 {
                lint_references_and_aliases: Self::lint_references_and_aliases,
                context: (ignore_words, ignore_words_regex),
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
    fn lint_references_and_aliases(
        table_aliases: Vec<AliasInfo>,
        standalone_aliases: Vec<SmolStr>,
        references: Vec<ObjectReferenceSegment>,
        col_aliases: Vec<ColumnAliasInfo>,
        using_cols: Vec<SmolStr>,
        context: &(Vec<String>, Vec<Regex>),
    ) -> Vec<LintResult> {
        if table_aliases.len() <= 1 {
            return Vec::new();
        }

        let mut violation_buff = Vec::new();
        for r in references {
            if context.0.contains(&r.0.raw().to_lowercase()) {
                continue;
            }

            if context
                .1
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
