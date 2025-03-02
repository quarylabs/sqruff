use regex::Regex;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Default, Debug, Clone)]
pub struct RuleRF06 {
    prefer_quoted_identifiers: bool,
    prefer_quoted_keywords: bool,
    ignore_words: Vec<String>,
    ignore_words_regex: Vec<Regex>,
    force_enable: bool,
}

impl Rule for RuleRF06 {
    fn load_from_config(
        &self,
        config: &ahash::AHashMap<String, Value>,
    ) -> Result<ErasedRule, String> {
        Ok(Self {
            prefer_quoted_identifiers: config["prefer_quoted_identifiers"].as_bool().unwrap(),
            prefer_quoted_keywords: config["prefer_quoted_keywords"].as_bool().unwrap(),
            ignore_words: config["ignore_words"]
                .map(|it| {
                    it.as_array()
                        .unwrap()
                        .iter()
                        .map(|it| it.as_string().unwrap().to_lowercase())
                        .collect()
                })
                .unwrap_or_default(),
            ignore_words_regex: config["ignore_words_regex"]
                .map(|it| {
                    it.as_array()
                        .unwrap()
                        .iter()
                        .map(|it| Regex::new(it.as_string().unwrap()).unwrap())
                        .collect()
                })
                .unwrap_or_default(),
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "references.quoting"
    }

    fn description(&self) -> &'static str {
        "Unnecessary quoted identifier."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, a valid unquoted identifier, that is also not a reserved keyword, is needlessly quoted.

```sql
SELECT 123 as "foo"
```

**Best practice**

Use unquoted identifiers where possible.

```sql
SELECT 123 as foo
```

When `prefer_quoted_identifiers = True`, the quotes are always necessary, no matter if the identifier is valid, a reserved keyword, or contains special characters.

> **Note**
> Note due to different quotes being used by different dialects supported by `SQLFluff`, and those quotes meaning different things in different contexts, this mode is not `sqlfluff fix` compatible.

**Anti-pattern**

In this example, a valid unquoted identifier, that is also not a reserved keyword, is required to be quoted.

```sql
SELECT 123 as foo
```

**Best practice**

Use quoted identifiers.

```sql
SELECT 123 as "foo" -- For ANSI, ...
-- or
SELECT 123 as `foo` -- For BigQuery, MySql, ...
```"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::References]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if matches!(
            context.dialect.name,
            DialectKind::Postgres | DialectKind::Snowflake
        ) && !self.force_enable
        {
            return Vec::new();
        }

        if FunctionalContext::new(context)
            .parent_stack()
            .any(Some(|it| {
                [SyntaxKind::PasswordAuth, SyntaxKind::ExecuteAsClause]
                    .into_iter()
                    .any(|ty| it.is_type(ty))
            }))
        {
            return Vec::new();
        }

        let identifier_is_quoted =
            !lazy_regex::regex_is_match!(r#"^[^"\'\[].+[^"\'\]]$"#, context.segment.raw().as_ref());

        let identifier_contents = context.segment.raw();
        let identifier_contents = if identifier_is_quoted {
            identifier_contents
                .get(1..identifier_contents.len() - 1)
                .map(ToOwned::to_owned)
                .unwrap_or_default()
        } else {
            identifier_contents.to_string()
        };

        let identifier_is_keyword = context
            .dialect
            .sets("reserved_keywords")
            .contains(identifier_contents.to_uppercase().as_str())
            || context
                .dialect
                .sets("unreserved_keywords")
                .contains(identifier_contents.to_uppercase().as_str());

        let context_policy = if self.prefer_quoted_identifiers {
            SyntaxKind::NakedIdentifier
        } else {
            SyntaxKind::QuotedIdentifier
        };

        if self
            .ignore_words
            .contains(&identifier_contents.to_lowercase())
        {
            return Vec::new();
        }

        if self
            .ignore_words_regex
            .iter()
            .any(|regex| regex.is_match(identifier_contents.as_ref()))
        {
            return Vec::new();
        }

        if self.prefer_quoted_keywords && identifier_is_keyword {
            return if !identifier_is_quoted {
                vec![LintResult::new(
                    context.segment.clone().into(),
                    Vec::new(),
                    Some(format!(
                        "Missing quoted keyword identifier {identifier_contents}."
                    )),
                    None,
                )]
            } else {
                Vec::new()
            };
        }

        if !context.segment.is_type(context_policy)
            || context
                .segment
                .raw()
                .eq_ignore_ascii_case("quoted_identifier")
            || context
                .segment
                .raw()
                .eq_ignore_ascii_case("naked_identifier")
        {
            return Vec::new();
        }

        if self.prefer_quoted_identifiers {
            return vec![LintResult::new(
                context.segment.clone().into(),
                Vec::new(),
                Some(format!("Missing quoted identifier {identifier_contents}.")),
                None,
            )];
        }

        let owned = context.dialect.grammar("NakedIdentifierSegment");

        let naked_identifier_parser = owned.as_regex().unwrap();

        if is_full_match(
            naked_identifier_parser.template.as_str(),
            &identifier_contents,
        ) && naked_identifier_parser
            .anti_template
            .as_ref()
            .is_none_or(|anti_template| {
                !is_full_match(anti_template.as_str(), &identifier_contents)
            })
        {
            return vec![LintResult::new(
                context.segment.clone().into(),
                vec![LintFix::replace(
                    context.segment.clone(),
                    vec![
                        SegmentBuilder::token(
                            context.tables.next_id(),
                            &identifier_contents,
                            SyntaxKind::NakedIdentifier,
                        )
                        .finish(),
                    ],
                    None,
                )],
                Some(format!(
                    "Unnecessary quoted identifier {}.",
                    context.segment.raw()
                )),
                None,
            )];
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const { SyntaxSet::new(&[SyntaxKind::QuotedIdentifier, SyntaxKind::NakedIdentifier]) },
        )
        .into()
    }
}

fn is_full_match(pattern: &str, text: &str) -> bool {
    let full_pattern = format!("(?i)^{}$", pattern); // Adding (?i) for case insensitivity
    let regex = fancy_regex::Regex::new(&full_pattern).unwrap();
    regex.is_match(text).unwrap()
}
