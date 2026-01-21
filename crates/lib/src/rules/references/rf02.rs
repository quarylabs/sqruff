use itertools::Itertools;
use regex::Regex;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::common::{AliasInfo, ColumnAliasInfo};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::object_reference::ObjectReferenceSegment;
use sqruff_lib_core::utils::analysis::select::get_select_statement_info;

use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RuleRF02;

impl Rule for RuleRF02 {
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
        let Some(select_info) =
            get_select_statement_info(&context.segment, context.dialect.into(), true)
        else {
            return Vec::new();
        };

        let rules = &context.config.rules.references_qualification;

        Self::lint_references_and_aliases(
            select_info.table_aliases,
            select_info.standalone_aliases,
            select_info.reference_buffer,
            select_info.col_aliases,
            select_info.using_cols,
            (&rules.ignore_words, &rules.ignore_words_regex),
        )
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
        context: (&[String], &[Regex]),
    ) -> Vec<LintResult> {
        if table_aliases.len() <= 1 {
            return Vec::new();
        }

        let mut violation_buff = Vec::new();
        for r in references {
            if context
                .0
                .iter()
                .any(|word| word.eq_ignore_ascii_case(r.0.raw().as_ref()))
            {
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
