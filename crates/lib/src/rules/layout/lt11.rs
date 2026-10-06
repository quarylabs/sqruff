use std::rc::Rc;

use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::reflow::sequence::{
    RebreakType, ReflowSequence, ReflowSequenceIndex, TargetSide,
};

#[derive(Debug, Default, Clone)]
pub struct RuleLT11;

impl Rule for RuleLT11 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT11.erased())
    }

    fn name(&self) -> &'static str {
        "layout.set_operators"
    }

    fn description(&self) -> &'static str {
        "Set operators should be surrounded by newlines."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `UNION ALL` is not on a line itself.

```sql
SELECT 'a' AS col UNION ALL
SELECT 'b' AS col
```

**Best practice**

Place `UNION ALL` on its own line.

```sql
SELECT 'a' AS col
UNION ALL
SELECT 'b' AS col
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let root = context.parent_stack.first().unwrap();
        let index = context
            .try_get::<Rc<ReflowSequenceIndex>>()
            .unwrap_or_else(|| {
                let index = Rc::new(ReflowSequenceIndex::from_root(root));
                context.set(index.clone());
                index
            });
        ReflowSequence::from_around_target_with_index(
            &context.segment,
            root,
            TargetSide::Both,
            context.config,
            &index,
        )
        .rebreak(context.tables, RebreakType::Lines)
        .results()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SetOperator]) }).into()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::RuleLT11;
    use crate::core::config::FluffConfig;
    use crate::core::rules::Rule;
    use crate::core::rules::context::RuleContext;
    use crate::core::rules::crawlers::BaseCrawler;
    use crate::core::test_functions::{fresh_ansi_dialect, parse_ansi_string};
    use crate::utils::reflow::sequence::ReflowSequenceIndex;
    use sqruff_lib_core::parser::segments::Tables;

    #[test]
    fn long_union_chain_reuses_one_index_per_pass() {
        let sql = (0..800)
            .map(|idx| format!("SELECT {idx} AS id"))
            .collect::<Vec<_>>()
            .join("\nUNION ALL\n");
        let root = parse_ansi_string(&sql);
        let tables = Tables::default();
        let dialect = fresh_ansi_dialect();
        let config = FluffConfig::default();
        let rule = RuleLT11;
        // A new context is used for each file and each autofix pass. The index
        // must be shared within a pass and released when that pass finishes.
        for _ in 0..2 {
            let mut context = RuleContext::new(&tables, &dialect, &config, root.clone());
            let mut count = 0;
            let mut first_index = None;
            rule.crawl_behaviour().crawl(&mut context, &mut |context| {
                assert!(rule.eval(context).is_empty());
                count += 1;
                let index = context.try_get::<Rc<ReflowSequenceIndex>>().unwrap();
                if let Some(first) = &first_index {
                    assert!(Rc::ptr_eq(&index, first));
                } else {
                    first_index = Some(index);
                }
            });
            assert_eq!(count, 799);
            let weak = Rc::downgrade(first_index.as_ref().unwrap());
            drop(first_index);
            drop(context);
            assert!(weak.upgrade().is_none());
        }
    }
}
