use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

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

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let functional_context = FunctionalContext::new(context);
        let segment = functional_context.segment();
        let children = segment.children_all();

        if children.any_match(|it| it.get_type() == SyntaxKind::AliasExpression) {
            return Vec::new();
        }

        // Ignore if it's a function with EMITS clause as EMITS is equivalent to AS
        let functions = children.filter(|sp: &ErasedSegment| sp.is_type(SyntaxKind::Function));
        if !functions
            .children_all()
            .filter(|sp: &ErasedSegment| sp.is_type(SyntaxKind::EmitsSegment))
            .is_empty()
        {
            return Vec::new();
        }

        let casts = children
            .children_all()
            .filter(|it: &ErasedSegment| it.is_type(SyntaxKind::CastExpression));
        if !casts.is_empty()
            && !casts
                .children_all()
                .any_match(|it| it.is_type(SyntaxKind::Function))
        {
            return Vec::new();
        }

        let parent_stack = functional_context.parent_stack();

        if parent_stack
            .find_last_where(|it| it.is_type(SyntaxKind::CommonTableExpression))
            .children_all()
            .any_match(|it| it.is_type(SyntaxKind::CTEColumnList))
        {
            return Vec::new();
        }

        let select_clause_children =
            children.filter(|it: &ErasedSegment| !it.is_type(SyntaxKind::Star));
        let is_complex_clause = recursively_check_is_complex(select_clause_children);

        if !is_complex_clause {
            return Vec::new();
        }

        if context
            .config
            .get("allow_scalar", "rules")
            .as_bool()
            .unwrap()
        {
            let immediate_parent = parent_stack.last().unwrap().clone();
            let elements = Segments::new(immediate_parent, None)
                .children_where(|it| it.is_type(SyntaxKind::SelectClauseElement));

            if elements.len() > 1 {
                return vec![LintResult::new(
                    context.segment.clone().into(),
                    Vec::new(),
                    None,
                    None,
                )];
            }

            return Vec::new();
        }

        vec![LintResult::new(
            context.segment.clone().into(),
            Vec::new(),
            None,
            None,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectClauseElement]) })
            .into()
    }
}

fn recursively_check_is_complex(select_clause_or_exp_children: Segments) -> bool {
    let filtered = select_clause_or_exp_children.filter(|it: &ErasedSegment| {
        !matches!(
            it.get_type(),
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::ColumnReference
                | SyntaxKind::WildcardExpression
                | SyntaxKind::Bracketed
        )
    });
    let remaining_count = filtered.len();

    if remaining_count == 0 {
        return false;
    }

    let first_el = filtered.head();

    if remaining_count > 1 || !first_el.all_match(|it| it.is_type(SyntaxKind::Expression)) {
        return true;
    }

    recursively_check_is_complex(first_el.children_all())
}
