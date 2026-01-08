use ahash::AHashSet;
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::SyntaxKind;

use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, TokenSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};

#[derive(Clone)]
struct CachedBlockedWords(AHashSet<String>);

#[derive(Default, Clone, Debug)]
pub struct RuleCV09;

impl Rule for RuleCV09 {
    fn name(&self) -> &'static str {
        "convention.blocked_words"
    }

    fn description(&self) -> &'static str {
        "Block a list of configurable words from being used."
    }

    fn long_description(&self) -> &'static str {
        r#"
This generic rule can be useful to prevent certain keywords, functions, or objects
from being used. Only whole words can be blocked, not phrases, nor parts of words.

This block list is case insensitive.

**Example use cases**

* We prefer ``BOOL`` over ``BOOLEAN`` and there is no existing rule to enforce
  this. Until such a rule is written, we can add ``BOOLEAN`` to the deny list
  to cause a linting error to flag this.
* We have deprecated a schema/table/function and want to prevent it being used
  in future. We can add that to the denylist and then add a ``-- noqa: CV09`` for
  the few exceptions that still need to be in the code base for now.

**Anti-pattern**

If the ``blocked_words`` config is set to ``deprecated_table,bool`` then the following will flag:

```sql
SELECT * FROM deprecated_table WHERE 1 = 1;
CREATE TABLE myschema.t1 (a BOOL);
```

**Best practice**

Do not used any blocked words.

```sql
SELECT * FROM my_table WHERE 1 = 1;
CREATE TABLE myschema.t1 (a BOOL);
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let rules = &context.config.rules.convention_blocked_words;
        if matches!(
            context.segment.get_type(),
            SyntaxKind::Comment | SyntaxKind::InlineComment | SyntaxKind::BlockComment
        ) || rules.blocked_words.is_empty() && rules.blocked_regex.is_empty()
        {
            return vec![];
        }

        let blocked_words = context
            .try_get::<CachedBlockedWords>()
            .map(|cached| cached.0)
            .unwrap_or_else(|| {
                let set = rules
                    .blocked_words
                    .iter()
                    .map(|value| value.to_uppercase())
                    .collect::<AHashSet<_>>();
                context.set(CachedBlockedWords(set.clone()));
                set
            });

        let raw_upper = context.segment.raw().to_uppercase();

        if blocked_words.contains(&raw_upper) {
            return vec![LintResult::new(
                Some(context.segment.clone()),
                vec![],
                Some(format!("Use of blocked word '{raw_upper}'.")),
                None,
            )];
        }

        for regex in &rules.blocked_regex {
            if regex.is_match(&raw_upper) {
                return vec![LintResult::new(
                    Some(context.segment.clone()),
                    vec![],
                    Some(format!("Use of blocked regex '{raw_upper}'.")),
                    None,
                )];
            }

            if rules.match_source {
                for (segment, _) in context.segment.raw_segments_with_ancestors() {
                    if regex.is_match(segment.raw().to_uppercase_smolstr().as_str()) {
                        return vec![LintResult::new(
                            Some(context.segment.clone()),
                            vec![],
                            Some(format!("Use of blocked regex '{raw_upper}'.")),
                            None,
                        )];
                    }
                }
            }
        }

        vec![]
    }

    fn crawl_behaviour(&self) -> Crawler {
        TokenSeekerCrawler.into()
    }
}
