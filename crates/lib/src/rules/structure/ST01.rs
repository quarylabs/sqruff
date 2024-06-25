use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Default, Debug, Clone)]
pub struct RuleST01 {}

impl Rule for RuleST01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleST01::default().erased()
    }

    fn name(&self) -> &'static str {
        "structure.else_null"
    }

    fn description(&self) -> &'static str {
        "Do not specify 'else null' in a case when statement (redundant)."
    }

    fn long_description(&self) -> Option<&'static str> {
        r#"
**Anti-pattern**

In this example, the reference `vee` has not been declared.

```sql
SELECT
    vee.a
FROM foo
```

**Best practice**

Remove the reference.

```sql
SELECT
    a
FROM foo
```
"#
        .into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let anchor = context.segment.clone();

        let children = FunctionalContext::new(context).segment().children(None);
        let else_clause = children.find_first(Some(|it: &ErasedSegment| it.is_type("else_clause")));

        if !else_clause.children(Some(|child| child.raw().eq_ignore_ascii_case("NULL"))).is_empty()
        {
            let before_else = children.reversed().select(
                None,
                Some(|it| matches!(it.get_type(), "whitespace" | "newline") | it.is_meta()),
                else_clause.first().unwrap().into(),
                None,
            );

            let mut fixes = Vec::with_capacity(before_else.len() + 1);
            fixes.push(LintFix::delete(else_clause.first().unwrap().clone()));
            fixes.extend(before_else.into_iter().map(LintFix::delete));

            vec![LintResult::new(anchor.into(), fixes, None, None, None)]
        } else {
            Vec::new()
        }
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["case_expression"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::fix;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::structure::ST01::RuleST01;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleST01::default().erased()]
    }

    #[test]
    fn redundant_else_null() {
        let fail_str = "
    select
        case name
            when 'cat' then 'meow'
            when 'dog' then 'woof'
            else null
        end
    from x";

        let fix_str = "
    select
        case name
            when 'cat' then 'meow'
            when 'dog' then 'woof'
        end
    from x";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn alternate_case_when_syntax() {
        let fail_str = "
    select
        case name
            when 'cat' then 'meow'
            when 'dog' then 'woof'
            else null
        end
    from x";

        let fix_str = "
    select
        case name
            when 'cat' then 'meow'
            when 'dog' then 'woof'
        end
    from x";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn alternate_case_when_syntax_boolean() {
        let pass_str = "
    select
        case name
            when 'cat' then true
            when 'dog' then true
            else name is null
        end
    from x";

        let fixed = fix(pass_str.into(), rules());
        assert_eq!(pass_str, fixed);
    }

    #[test]
    fn else_expression() {
        let pass_str = "
    select
        case name
            when 'cat' then 'meow'
            when 'dog' then 'woof'
            else iff(wing_type is not null, 'tweet', 'invalid')
        end
    from x";

        let fixed = fix(pass_str.into(), rules());
        assert_eq!(pass_str, fixed);
    }
}
