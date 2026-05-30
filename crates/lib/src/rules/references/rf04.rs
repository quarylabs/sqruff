use regex::Regex;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::config::RuleConfigs;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeeker};
use crate::core::rules::{Erased as _, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::identifers::identifiers_policy_applicable;

#[derive(Debug, Clone, Default)]
pub struct RuleRF04 {
    unquoted_identifiers_policy: String,
    quoted_identifiers_policy: Option<String>,
    ignore_words: Vec<String>,
    ignore_words_regex: Vec<Regex>,
}

impl Rule for RuleRF04 {
    fn load_from_config(&self, config: &RuleConfigs) -> Result<ErasedRule, String> {
        Ok(RuleRF04 {
            unquoted_identifiers_policy: config
                .references
                .keywords
                .unquoted_identifiers_policy
                .as_str()
                .to_owned(),
            quoted_identifiers_policy: config
                .references
                .keywords
                .quoted_identifiers_policy
                .map(|policy| policy.as_str().to_owned()),
            ignore_words: config.references.keywords.ignore_words.clone(),
            ignore_words_regex: config.references.keywords.ignore_words_regex.clone(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "references.keywords"
    }

    fn description(&self) -> &'static str {
        "Keywords should not be used as identifiers."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `SUM` (a built-in function) is used as an alias.

```sql
SELECT
    sum.a
FROM foo AS sum
```

**Best practice**

Avoid using keywords as the name of an alias.

```sql
SELECT
    vee.a
FROM foo AS vee
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::References]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.segment.raw().len() == 1
            || self
                .ignore_words
                .contains(&context.segment.raw().to_lowercase())
            || self
                .ignore_words_regex
                .iter()
                .any(|regex| regex.is_match(context.segment.raw()))
        {
            return vec![LintResult::new(None, Vec::new(), None, None)];
        }

        let raw_segment = context.segment.raw();
        let upper_segment = {
            if context.segment.is_type(SyntaxKind::NakedIdentifier) {
                raw_segment.to_uppercase()
            } else {
                raw_segment[1..raw_segment.len() - 1].to_uppercase()
            }
        };

        // FIXME: simplify the condition
        if (context.segment.is_type(SyntaxKind::NakedIdentifier)
            && identifiers_policy_applicable(
                &self.unquoted_identifiers_policy,
                &context.parent_stack,
            )
            && context
                .dialect
                .sets("unreserved_keywords")
                .contains(context.segment.raw().to_uppercase().as_str()))
            || (context.segment.is_type(SyntaxKind::QuotedIdentifier)
                && self.quoted_identifiers_policy.as_ref().is_some_and(
                    |quoted_identifiers_policy| {
                        identifiers_policy_applicable(
                            quoted_identifiers_policy,
                            &context.parent_stack,
                        )
                    },
                )
                && (context
                    .dialect
                    .sets("unreserved_keywords")
                    .contains(upper_segment.as_str())
                    || context
                        .dialect
                        .sets("reserved_keywords")
                        .contains(upper_segment.as_str())
                    || context
                        .dialect
                        .sets("future_reserved_keywords")
                        .contains(upper_segment.as_str())))
        {
            vec![LintResult::new(
                Some(context.segment.clone()),
                Vec::new(),
                None,
                None,
            )]
        } else {
            Vec::new()
        }
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeeker::new(
            const { SyntaxSet::new(&[SyntaxKind::NakedIdentifier, SyntaxKind::QuotedIdentifier]) },
        )
        .into()
    }
}
