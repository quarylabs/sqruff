use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::{CapitalisationPolicyName, ExtendedCapitalisationPolicy, handle_segment};
use crate::core::config::Value;
use crate::core::rules::config::{RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

crate::rule_config! {
    /// Configuration for `capitalisation.types` (CP05).
    RuleCP05Config {
        /// The capitalisation to enforce on data types.
        extended_capitalisation_policy: ExtendedCapitalisationPolicy =
            ExtendedCapitalisationPolicy::Consistent,
    }
}

#[derive(Debug, Default, Clone)]
pub struct RuleCP05 {
    extended_capitalisation_policy: ExtendedCapitalisationPolicy,
}

impl Rule for RuleCP05 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleCP05Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleCP05Config::from_config(config)?;

        Ok(RuleCP05 {
            extended_capitalisation_policy: config.extended_capitalisation_policy,
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "capitalisation.types"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of datatypes."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `int` and `unsigned` are in lower-case whereas `VARCHAR` is in upper-case.

```sql
CREATE TABLE t (
    a int unsigned,
    b VARCHAR(15)
);
```

**Best practice**

Ensure all datatypes are consistently upper or lower case

```sql
CREATE TABLE t (
    a INT UNSIGNED,
    b VARCHAR(15)
);
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[
            RuleGroups::All,
            RuleGroups::Core,
            RuleGroups::Capitalisation,
        ]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut results = Vec::new();

        if context.segment.is_type(SyntaxKind::PrimitiveType)
            || context.segment.is_type(SyntaxKind::DatetimeTypeIdentifier)
            || context.segment.is_type(SyntaxKind::DataType)
        {
            for seg in context.segment.segments() {
                if seg.is_type(SyntaxKind::Symbol)
                    || seg.is_type(SyntaxKind::Identifier)
                    || seg.is_type(SyntaxKind::QuotedIdentifier)
                    || seg.is_type(SyntaxKind::QuotedLiteral)
                    || !seg.segments().is_empty()
                {
                    continue;
                }

                results.push(handle_segment(
                    "Datatypes",
                    self.extended_capitalisation_policy,
                    CapitalisationPolicyName::Extended,
                    seg.clone(),
                    context,
                ));
            }
        }

        results
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::DataTypeIdentifier,
                    SyntaxKind::PrimitiveType,
                    SyntaxKind::DatetimeTypeIdentifier,
                    SyntaxKind::DataType,
                ])
            },
        )
        .into()
    }
}
