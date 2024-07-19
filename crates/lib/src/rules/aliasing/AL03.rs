use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::SyntaxKind;
use crate::utils::functional::context::FunctionalContext;
use crate::utils::functional::segments::Segments;

#[derive(Debug, Clone)]
pub struct RuleAL03;

impl Rule for RuleAL03 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL03.erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.expression"
    }

    fn description(&self) -> &'static str {
        "Column expression without alias. Use explicit `AS` clause."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, there is no alias for both sums.

```sql
SELECT
    sum(a),
    sum(b)
FROM foo
```

**Best practice**

Add aliases.

```sql
SELECT
    sum(a) AS a_sum,
    sum(b) AS b_sum
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let functional_context = FunctionalContext::new(context.clone());
        let segment = functional_context.segment();
        let children = segment.children(None);

        if children.any(Some(|it| it.get_type() == SyntaxKind::AliasExpression)) {
            return Vec::new();
        }

        // Ignore if it's a function with EMITS clause as EMITS is equivalent to AS
        if !children
            .select(Some(|sp: &ErasedSegment| sp.is_type(SyntaxKind::Function)), None, None, None)
            .children(None)
            .select(
                Some(|sp: &ErasedSegment| sp.is_type(SyntaxKind::EmitsSegment)),
                None,
                None,
                None,
            )
            .is_empty()
        {
            return Vec::new();
        }

        if !children
            .children(None)
            .select(
                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::CastExpression)),
                None,
                None,
                None,
            )
            .is_empty()
            && !children
                .children(None)
                .select(
                    Some(|it: &ErasedSegment| it.is_type(SyntaxKind::CastExpression)),
                    None,
                    None,
                    None,
                )
                .children(None)
                .any(Some(|it| it.is_type(SyntaxKind::Function)))
        {
            return Vec::new();
        }

        let parent_stack = functional_context.parent_stack();

        if parent_stack
            .find_last(Some(|it| it.is_type(SyntaxKind::CommonTableExpression)))
            .children(None)
            .any(Some(|it| it.is_type(SyntaxKind::CTEColumnList)))
        {
            return Vec::new();
        }

        let select_clause_children = children.select(
            Some(|it: &ErasedSegment| !it.is_type(SyntaxKind::Star)),
            None,
            None,
            None,
        );
        let is_complex_clause = recursively_check_is_complex(select_clause_children);

        if !is_complex_clause {
            return Vec::new();
        }

        if context.config.unwrap().get("allow_scalar", "rules").as_bool().unwrap() {
            let immediate_parent = parent_stack.find_last(None);
            let elements =
                immediate_parent.children(Some(|it| it.is_type(SyntaxKind::SelectClauseElement)));

            if elements.len() > 1 {
                return vec![LintResult::new(context.segment.into(), Vec::new(), None, None, None)];
            }

            return Vec::new();
        }

        vec![LintResult::new(context.segment.into(), Vec::new(), None, None, None)]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new([SyntaxKind::SelectClauseElement].into()).into()
    }
}

fn recursively_check_is_complex(select_clause_or_exp_children: Segments) -> bool {
    let selector: Option<fn(&ErasedSegment) -> bool> = Some(|it: &ErasedSegment| {
        !matches!(
            it.get_type(),
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::ColumnReference
                | SyntaxKind::WildcardExpression
                | SyntaxKind::Bracketed
        )
    });

    let filtered = select_clause_or_exp_children.select(selector, None, None, None);
    let remaining_count = filtered.len();

    if remaining_count == 0 {
        return false;
    }

    let first_el = filtered.find_first::<fn(&ErasedSegment) -> _>(None);

    if remaining_count > 1 || !first_el.all(Some(|it| it.is_type(SyntaxKind::Expression))) {
        return true;
    }

    recursively_check_is_complex(first_el.children(None))
}

#[cfg(test)]
mod tests {
    use crate::api::simple::lint;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::aliasing::AL03::RuleAL03;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleAL03.erased()]
    }

    #[test]
    fn test_pass_column_exp_without_alias_1() {
        let violations = lint(
            "select ps.*, pandgs.blah from ps join pandgs using(moo)".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_column_exp_without_alias_allow_scalar_true() {
        let violations =
            lint("SELECT 1 from blah".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_column_exp_without_alias() {
        let violations =
            lint("SELECT upper(foo), bar from blah".into(), "ansi".into(), rules(), None, None)
                .unwrap();

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 8);
        assert_eq!(
            violations[0].desc(),
            "Column expression without alias. Use explicit `AS` clause."
        );
    }

    #[test]
    fn test_pass_column_exp_without_alias_if_only_cast() {
        let violations = lint(
            "SELECT foo_col::VARCHAR(28) , bar from blah".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_column_exp_without_alias_if_only_cast_inc_double_cast() {
        let violations = lint(
            "SELECT foo_col::INT::VARCHAR , bar from blah".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_column_exp_without_alias_if_bracketed() {
        let violations = lint(
            "SELECT (foo_col::INT)::VARCHAR , bar from blah".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_column_exp_without_alias_and_cast_fn() {
        let violations = lint(
            "SELECT CAST(foo_col AS INT)::VARCHAR , bar from blah".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_ne!(violations, []);
    }

    #[test]
    #[ignore = "CONFIG"]
    fn test_fail_column_exp_without_alias_allow_scalar_false() {
        // Some(json!({ "rules": { "allow_scalar": false } }))

        let violations =
            lint("SELECT 1 from blah".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_ne!(violations, []);
    }

    #[test]
    fn test_pass_column_exp_with_alias() {
        let violations = lint(
            "SELECT upper(foo) as foo_up, bar from blah".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }
}
