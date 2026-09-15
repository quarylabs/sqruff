use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Aliasing {
    Explicit,
    Implicit,
}

#[derive(Debug, Clone)]
pub struct RuleAL01 {
    aliasing: Aliasing,
    target_parent_types: SyntaxSet,
}

impl RuleAL01 {
    pub fn aliasing(mut self, aliasing: Aliasing) -> Self {
        self.aliasing = aliasing;
        self
    }

    pub fn target_parent_types(mut self, target_parent_types: SyntaxSet) -> Self {
        self.target_parent_types = target_parent_types;
        self
    }
}

impl Default for RuleAL01 {
    fn default() -> Self {
        Self {
            aliasing: Aliasing::Explicit,
            target_parent_types: const {
                SyntaxSet::new(&[
                    SyntaxKind::FromExpressionElement,
                    SyntaxKind::MergeStatement,
                ])
            },
        }
    }
}

impl Rule for RuleAL01 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let aliasing = match _config.get("aliasing").unwrap().as_string().unwrap() {
            "explicit" => Aliasing::Explicit,
            "implicit" => Aliasing::Implicit,
            _ => unreachable!(),
        };

        Ok(RuleAL01 {
            aliasing,
            target_parent_types: const {
                SyntaxSet::new(&[
                    SyntaxKind::FromExpressionElement,
                    SyntaxKind::MergeStatement,
                ])
            },
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.table"
    }

    fn description(&self) -> &'static str {
        "Implicit/explicit aliasing of table."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the alias `voo` is implicit.

```sql
SELECT
    voo.a
FROM foo voo
```

**Best practice**

Add `AS` to make the alias explicit.

```sql
SELECT
    voo.a
FROM foo AS voo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Aliasing]
    }

    fn eval(&self, rule_cx: &RuleContext) -> Vec<LintResult> {
        // Oracle does not allow AS for table aliases. AL02 delegates its column
        // alias evaluation to this implementation, so distinguish AL01 by its
        // table-parent target rather than by the concrete Rust type.
        if rule_cx.dialect.name == DialectKind::Oracle
            && self
                .target_parent_types
                .contains(SyntaxKind::FromExpressionElement)
        {
            return Vec::new();
        }

        let last_seg = rule_cx.parent_stack.last().unwrap();
        let last_seg_ty = last_seg.get_type();

        if self.target_parent_types.contains(last_seg_ty) {
            let as_keyword = rule_cx
                .segment
                .child(&SyntaxSet::new(&[SyntaxKind::AliasOperator]));

            if let Some(as_keyword) = as_keyword {
                if self.aliasing == Aliasing::Implicit {
                    return vec![LintResult::new(
                        as_keyword.clone().into(),
                        rule_cx
                            .segment
                            .child(&SyntaxSet::new(&[SyntaxKind::Whitespace]))
                            .into_iter()
                            .map(LintFix::delete)
                            .chain(std::iter::once(LintFix::delete(as_keyword)))
                            .collect(),
                        None,
                        None,
                    )];
                }
            } else if self.aliasing != Aliasing::Implicit {
                let identifier = rule_cx
                    .segment
                    .get_raw_segments()
                    .into_iter()
                    .find(|seg| seg.is_code())
                    .expect("Failed to find identifier. Raise this as a bug on GitHub.");

                let operator = SegmentBuilder::node(
                    rule_cx.tables.next_id(),
                    SyntaxKind::AliasOperator,
                    rule_cx.dialect.name,
                    vec![SegmentBuilder::keyword(rule_cx.tables.next_id(), "AS")],
                );
                let mut edit = Vec::new();
                if !rule_cx
                    .parent_stack
                    .last()
                    .and_then(|parent| parent.segments().get(..rule_cx.segment_idx))
                    .and_then(|siblings| siblings.last())
                    .is_some_and(|segment| segment.is_whitespace())
                {
                    edit.push(SegmentBuilder::whitespace(rule_cx.tables.next_id(), " "));
                }
                edit.push(operator.finish());
                edit.push(SegmentBuilder::whitespace(rule_cx.tables.next_id(), " "));
                return vec![LintResult::new(
                    rule_cx.segment.clone().into(),
                    vec![LintFix::create_before(identifier, edit)],
                    None,
                    None,
                )];
            }
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::AliasExpression]) }).into()
    }
}
