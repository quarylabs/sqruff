use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

/// Prefer ANSI-style `expression AS alias` over T-SQL's `alias = expression`.
#[derive(Clone, Debug, Default)]
pub struct RuleTQ04 {
    force_enable: bool,
}

impl Rule for RuleTQ04 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(Self {
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "tsql.prefer_as_alias"
    }

    fn description(&self) -> &'static str {
        "Use ANSI-style `AS` aliasing instead of `alias = expression`."
    }

    fn long_description(&self) -> &'static str {
        r#"
T-SQL permits `alias = expression` in SELECT clauses. This opt-in rule
prefers the ANSI-style `expression AS alias` form. Set `force_enable = true`
to enable it for the T-SQL dialect.

**Anti-pattern**

```sql
SELECT help3 = 'hello';
```

**Best practice**

```sql
SELECT 'hello' AS help3;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Tsql]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if !self.force_enable || context.dialect.name != DialectKind::Tsql {
            return Vec::new();
        }

        let alias_expression = &context.segment;
        let Some(alias_operator) = alias_expression
            .segments()
            .iter()
            .find(|segment| segment.is_type(SyntaxKind::AliasOperator))
        else {
            return Vec::new();
        };
        if alias_operator.raw().as_str() != "=" {
            return Vec::new();
        }

        let Some(select_element) = context
            .parent_stack
            .last()
            .filter(|segment| segment.is_type(SyntaxKind::SelectClauseElement))
        else {
            return Vec::new();
        };
        let alias_children = alias_expression.segments();
        let select_children = select_element.segments();
        let Some((alias_identifier_idx, alias_identifier)) = alias_children
            .iter()
            .enumerate()
            .find(|(_, segment)| segment.is_code() && *segment != alias_operator)
        else {
            return Vec::new();
        };
        let Some(alias_operator_idx) = alias_children
            .iter()
            .position(|segment| segment == alias_operator)
        else {
            return Vec::new();
        };
        let Some(alias_expression_idx) = select_children
            .iter()
            .position(|segment| segment == alias_expression)
        else {
            return Vec::new();
        };
        let Some((expression_idx, expression)) = select_children
            .iter()
            .enumerate()
            .find(|(_, segment)| segment.is_code() && *segment != alias_expression)
        else {
            return Vec::new();
        };

        let whitespace_before_operator: Vec<ErasedSegment> = alias_children
            [alias_identifier_idx + 1..alias_operator_idx]
            .iter()
            .filter(|segment| segment.is_whitespace())
            .cloned()
            .collect();
        let segments_before_expression = &select_children[alias_expression_idx + 1..expression_idx];
        let has_non_whitespace_before_expression = segments_before_expression
            .iter()
            .any(|segment| !segment.is_whitespace());
        let expression_prefix = if has_non_whitespace_before_expression {
            let start = usize::from(
                segments_before_expression
                    .first()
                    .is_some_and(ErasedSegment::is_whitespace),
            );
            segments_before_expression[start..].to_vec()
        } else {
            Vec::new()
        };
        let whitespace_after_operator: Vec<ErasedSegment> = segments_before_expression
            .iter()
            .filter(|segment| segment.is_whitespace())
            .cloned()
            .collect();
        let new_space = || SegmentBuilder::whitespace(context.tables.next_id(), " ");
        let expression_to_alias_spacing =
            if has_non_whitespace_before_expression || whitespace_after_operator.is_empty() {
                vec![new_space()]
            } else {
                whitespace_after_operator
            };
        let before_identifier_spacing = if whitespace_before_operator.is_empty() {
            vec![new_space()]
        } else {
            whitespace_before_operator
        };
        let as_operator = SegmentBuilder::node(
            context.tables.next_id(),
            SyntaxKind::AliasOperator,
            context.dialect.name,
            vec![SegmentBuilder::keyword(context.tables.next_id(), "AS")],
        )
        .finish();

        let mut edit = expression_prefix;
        edit.push(expression.clone());
        edit.extend(expression_to_alias_spacing);
        edit.push(as_operator);
        edit.extend(before_identifier_spacing);
        edit.push(alias_identifier.clone());

        vec![LintResult::new(
            Some(alias_operator.clone()),
            vec![LintFix::replace(
                select_element.clone(),
                vec![select_element.new(edit)],
                Some(select_children.to_vec()),
            )],
            Some(self.description().into()),
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::single(SyntaxKind::AliasExpression) }).into()
    }
}
