use hashbrown::HashSet;
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::SyntaxKind;

use crate::config::RuleConfigs;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, TokenSeeker};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Default, Clone, Debug)]
pub struct RuleCV09 {
    blocked_words: HashSet<String>,
    blocked_regex: Vec<regex::Regex>,
    match_source: bool,
}

impl Rule for RuleCV09 {
    fn load_from_config(&self, config: &RuleConfigs) -> Result<ErasedRule, String> {
        let cfg = &config.convention.blocked_words;
        let blocked_words = cfg.blocked_words.iter().cloned().collect::<HashSet<_>>();
        let blocked_regex = cfg.blocked_regex.clone();
        let match_source = cfg.match_source;
        Ok(RuleCV09 {
            blocked_words,
            blocked_regex,
            match_source,
        }
        .erased())
    }

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
        if matches!(
            context.segment.get_type(),
            SyntaxKind::Comment | SyntaxKind::InlineComment | SyntaxKind::BlockComment
        ) || self.blocked_words.is_empty() && self.blocked_regex.is_empty()
        {
            return vec![];
        }

        let raw_upper = context.segment.raw().to_uppercase();

        if self.blocked_words.contains(&raw_upper) {
            return vec![LintResult::new(
                Some(context.segment.clone()),
                vec![],
                Some(format!("Use of blocked word '{raw_upper}'.")),
                None,
            )];
        }

        for regex in &self.blocked_regex {
            if regex.is_match(&raw_upper) {
                return vec![LintResult::new(
                    Some(context.segment.clone()),
                    vec![],
                    Some(format!("Use of blocked regex '{raw_upper}'.")),
                    None,
                )];
            }

            if self.match_source {
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
        TokenSeeker.into()
    }
}
