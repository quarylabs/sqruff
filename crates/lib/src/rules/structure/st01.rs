use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Default, Debug, Clone)]
pub struct RuleST01;

impl Rule for RuleST01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST01.erased())
    }

    fn name(&self) -> &'static str {
        "structure.else_null"
    }

    fn description(&self) -> &'static str {
        "Do not specify 'else null' in a case when statement (redundant)."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

```sql
select
    case
        when name like '%cat%' then 'meow'
        when name like '%dog%' then 'woof'
        else null
    end
from x
```

**Best practice**

Omit `else null`

```sql
select
    case
        when name like '%cat%' then 'meow'
        when name like '%dog%' then 'woof'
    end
from x
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let anchor = context.segment.clone();

        let children = FunctionalContext::new(context).segment().children(None);
        let else_clause = children.find_first(Some(|it: &ErasedSegment| {
            it.is_type(SyntaxKind::ElseClause)
        }));

        if !else_clause
            .children(Some(|child| child.raw().eq_ignore_ascii_case("NULL")))
            .is_empty()
        {
            let before_else = children.reversed().select::<fn(&ErasedSegment) -> bool>(
                None,
                Some(|it| {
                    matches!(it.get_type(), SyntaxKind::Whitespace | SyntaxKind::Newline)
                        | it.is_meta()
                }),
                else_clause.first().unwrap().into(),
                None,
            );

            let mut fixes = Vec::with_capacity(before_else.len() + 1);
            fixes.push(LintFix::delete(else_clause.first().unwrap().clone()));
            fixes.extend(before_else.into_iter().map(LintFix::delete));

            vec![LintResult::new(anchor.into(), fixes, None, None)]
        } else {
            Vec::new()
        }
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::CaseExpression]) }).into()
    }
}
