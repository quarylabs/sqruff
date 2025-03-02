use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleCV01 {
    preferred_not_equal_style: PreferredNotEqualStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum PreferredNotEqualStyle {
    #[default]
    Consistent,
    CStyle,
    Ansi,
}

impl Rule for RuleCV01 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        if let Some(value) = config["preferred_not_equal_style"].as_string() {
            let preferred_not_equal_style = match value {
                "consistent" => PreferredNotEqualStyle::Consistent,
                "c_style" => PreferredNotEqualStyle::CStyle,
                "ansi" => PreferredNotEqualStyle::Ansi,
                _ => {
                    return Err(format!(
                        "Invalid value for preferred_not_equal_style: {}",
                        value
                    ));
                }
            };
            Ok(RuleCV01 {
                preferred_not_equal_style,
            }
            .erased())
        } else {
            Err("Missing value for preferred_not_equal_style".to_string())
        }
    }

    fn name(&self) -> &'static str {
        "convention.not_equal"
    }

    fn description(&self) -> &'static str {
        "Consistent usage of ``!=`` or ``<>`` for \"not equal to\" operator."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Consistent usage of `!=` or `<>` for "not equal to" operator.

```sql
SELECT * FROM X WHERE 1 <> 2 AND 3 != 4;
```

**Best practice**

Ensure all "not equal to" comparisons are consistent, not mixing `!=` and `<>`.

```sql
SELECT * FROM X WHERE 1 != 2 AND 3 != 4;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // Get the comparison operator children
        let segment = FunctionalContext::new(context).segment();
        let raw_comparison_operators = segment.children(None);

        // Only check ``<>`` or ``!=`` operators
        let raw_operator_list = raw_comparison_operators
            .iter()
            .map(|r| r.raw())
            .collect::<Vec<_>>();
        if raw_operator_list != ["<", ">"] && raw_operator_list != ["!", "="] {
            return Vec::new();
        }

        // If style is consistent, add the style of the first occurrence to memory
        let preferred_style =
            if self.preferred_not_equal_style == PreferredNotEqualStyle::Consistent {
                if let Some(preferred_style) = context.try_get::<PreferredNotEqualStyle>() {
                    preferred_style
                } else {
                    let style = if raw_operator_list == ["<", ">"] {
                        PreferredNotEqualStyle::Ansi
                    } else {
                        PreferredNotEqualStyle::CStyle
                    };
                    context.set(style);
                    style
                }
            } else {
                self.preferred_not_equal_style
            };

        // Define the replacement
        let replacement = match preferred_style {
            PreferredNotEqualStyle::CStyle => {
                vec!["!", "="]
            }
            PreferredNotEqualStyle::Ansi => {
                vec!["<", ">"]
            }
            PreferredNotEqualStyle::Consistent => {
                unreachable!("Consistent style should have been handled earlier")
            }
        };

        // This operator already matches the existing style
        if raw_operator_list == replacement {
            return Vec::new();
        }

        // Provide a fix and replace ``<>`` with ``!=``
        // As each symbol is a separate symbol this is done in two steps:
        // Depending on style type, flip any inconsistent operators
        // 1. Flip < and !
        // 2. Flip > and =
        let fixes = vec![
            LintFix::replace(
                raw_comparison_operators[0].clone(),
                vec![
                    SegmentBuilder::token(
                        context.tables.next_id(),
                        replacement[0],
                        SyntaxKind::ComparisonOperator,
                    )
                    .finish(),
                ],
                None,
            ),
            LintFix::replace(
                raw_comparison_operators[1].clone(),
                vec![
                    SegmentBuilder::token(
                        context.tables.next_id(),
                        replacement[1],
                        SyntaxKind::ComparisonOperator,
                    )
                    .finish(),
                ],
                None,
            ),
        ];

        vec![LintResult::new(
            context.segment.clone().into(),
            fixes,
            None,
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::ComparisonOperator]) })
            .into()
    }
}
