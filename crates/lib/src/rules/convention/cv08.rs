use ahash::{AHashMap, AHashSet};
use smol_str::{SmolStr, StrExt};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Default, Clone, Debug)]
pub struct RuleCV08;

impl Rule for RuleCV08 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV08.erased())
    }

    fn name(&self) -> &'static str {
        "convention.left_join"
    }

    fn description(&self) -> &'static str {
        "Use LEFT JOIN instead of RIGHT JOIN."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

`RIGHT JOIN` is used.

```sql
SELECT
    foo.col1,
    bar.col2
FROM foo
RIGHT JOIN bar
    ON foo.bar_id = bar.id;
```

**Best practice**

Refactor and use ``LEFT JOIN`` instead.

```sql
SELECT
    foo.col1,
    bar.col2
FROM bar
LEFT JOIN foo
   ON foo.bar_id = bar.id;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        assert!(context.segment.is_type(SyntaxKind::JoinClause));

        let segments = context
            .segment
            .segments()
            .iter()
            .map(|segment| segment.raw().to_uppercase_smolstr())
            .collect::<AHashSet<_>>();

        let mut set = AHashSet::new();
        set.insert(SmolStr::new_static("RIGHT"));
        set.insert(SmolStr::new_static("JOIN"));

        if set.is_subset(&segments) {
            vec![LintResult::new(
                Some(context.segment.segments()[0].clone()),
                vec![],
                None,
                None,
            )]
        } else {
            vec![]
        }
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::JoinClause]) }).into()
    }
}
