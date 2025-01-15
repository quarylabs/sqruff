use ahash::{AHashMap, AHashSet};
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::SyntaxKind;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, TokenSeekerCrawler};

#[derive(Default, Clone, Debug)]
pub struct RuleCV09 {
    blocked_words: AHashSet<String>,
    blocked_regex: Vec<regex::Regex>,
    match_source: bool,
}

impl Rule for RuleCV09 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        let blocked_words = config["blocked_words"]
            .as_string()
            .map_or(Default::default(), |it| {
                it.split(',')
                    .map(|s| s.to_string().to_uppercase())
                    .collect::<AHashSet<_>>()
            });
        let blocked_regex = config["blocked_regex"]
            .as_array()
            .unwrap_or_default()
            .into_iter()
            .map(|regex| {
                let regex = regex.as_string();
                if let Some(regex) = regex {
                    Ok(regex::Regex::new(regex).map_err(|e| e.to_string())?)
                } else {
                    Err("blocked_regex must be an array of strings".to_string())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let match_source = config["match_source"].as_bool().unwrap_or_default();
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
                Some(format!("Use of blocked word '{}'.", raw_upper)),
                None,
            )];
        }

        for regex in &self.blocked_regex {
            if regex.is_match(&raw_upper) {
                return vec![LintResult::new(
                    Some(context.segment.clone()),
                    vec![],
                    Some(format!("Use of blocked regex '{}'.", raw_upper)),
                    None,
                )];
            }

            if self.match_source {
                for (segment, _) in context.segment.raw_segments_with_ancestors() {
                    if regex.is_match(segment.raw().to_uppercase_smolstr().as_str()) {
                        return vec![LintResult::new(
                            Some(context.segment.clone()),
                            vec![],
                            Some(format!("Use of blocked regex '{}'.", raw_upper)),
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
