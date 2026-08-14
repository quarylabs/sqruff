use regex::Regex;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::rules::config::{IgnoreWords, RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased as _, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::identifers::{IdentifiersPolicy, identifiers_policy_applicable};

crate::rule_config! {
    /// Configuration for `references.keywords` (RF04).
    RuleRF04Config {
        /// Which unquoted identifiers are checked against the keyword list.
        unquoted_identifiers_policy: IdentifiersPolicy = IdentifiersPolicy::Aliases,
        /// Which quoted identifiers are checked against the keyword list.
        quoted_identifiers_policy: IdentifiersPolicy = IdentifiersPolicy::None,
        /// Comma separated list of words to ignore, compared case-insensitively.
        ignore_words: IgnoreWords = IgnoreWords::default(),
        /// Comma separated list of regular expressions matching words to ignore.
        ignore_words_regex: Vec<Regex> = Vec::new(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuleRF04 {
    unquoted_identifiers_policy: IdentifiersPolicy,
    quoted_identifiers_policy: IdentifiersPolicy,
    ignore_words: IgnoreWords,
    ignore_words_regex: Vec<Regex>,
}

impl Rule for RuleRF04 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleRF04Config::config_options()
    }

    fn load_from_config(
        &self,
        config: &hashbrown::HashMap<String, crate::core::config::Value>,
    ) -> Result<ErasedRule, String> {
        let config = RuleRF04Config::from_config(config)?;

        Ok(RuleRF04 {
            unquoted_identifiers_policy: config.unquoted_identifiers_policy,
            quoted_identifiers_policy: config.quoted_identifiers_policy,
            ignore_words: config.ignore_words,
            ignore_words_regex: config.ignore_words_regex,
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
                self.unquoted_identifiers_policy,
                &context.parent_stack,
            )
            && context
                .dialect
                .sets("unreserved_keywords")
                .contains(context.segment.raw().to_uppercase().as_str()))
            || (context.segment.is_type(SyntaxKind::QuotedIdentifier)
                && identifiers_policy_applicable(
                    self.quoted_identifiers_policy,
                    &context.parent_stack,
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
        SegmentSeekerCrawler::new(
            const { SyntaxSet::new(&[SyntaxKind::NakedIdentifier, SyntaxKind::QuotedIdentifier]) },
        )
        .into()
    }
}
