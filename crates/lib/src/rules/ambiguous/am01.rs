use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Clone, Default)]
pub struct RuleAM01;

impl Rule for RuleAM01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM01 {}.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.distinct"
    }

    fn description(&self) -> &'static str {
        "Ambiguous use of 'DISTINCT' in a 'SELECT' statement with 'GROUP BY'."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

`DISTINCT` and `GROUP BY` are conflicting.

```sql
SELECT DISTINCT
    a
FROM foo
GROUP BY a
```

**Best practice**

Remove `DISTINCT` or `GROUP BY`. In our case, removing `GROUP BY` is better.


```sql
SELECT DISTINCT
    a
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(context).segment();

        if !segment
            .children(Some(|it| it.is_type(SyntaxKind::GroupbyClause)))
            .is_empty()
        {
            let distinct = segment
                .children(Some(|it| it.is_type(SyntaxKind::SelectClause)))
                .children(Some(|it| it.is_type(SyntaxKind::SelectClauseModifier)))
                .children(Some(|it| it.is_type(SyntaxKind::Keyword)))
                .select(
                    Some(|it: &ErasedSegment| it.is_keyword("DISTINCT")),
                    None,
                    None,
                    None,
                );

            if !distinct.is_empty() {
                return vec![LintResult::new(
                    distinct[0].clone().into(),
                    Vec::new(),
                    None,
                    None,
                )];
            }
        }

        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}
