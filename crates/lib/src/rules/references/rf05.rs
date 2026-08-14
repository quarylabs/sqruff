use hashbrown::{HashMap, HashSet};
use regex::Regex;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::Value;
use crate::core::rules::config::{IgnoreWords, RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::identifers::{IdentifiersPolicy, identifiers_policy_applicable};

crate::rule_config! {
    /// Configuration for `references.special_chars` (RF05).
    RuleRF05Config {
        /// Which unquoted identifiers are checked for special characters.
        unquoted_identifiers_policy: IdentifiersPolicy = IdentifiersPolicy::All,
        /// Which quoted identifiers are checked for special characters.
        quoted_identifiers_policy: IdentifiersPolicy = IdentifiersPolicy::All,
        /// Whether identifiers may contain spaces.
        allow_space_in_identifier: bool = false,
        /// Extra characters allowed on top of the alphanumeric ones.
        additional_allowed_characters: Option<String> = None,
        /// Comma separated list of words to ignore, compared case-insensitively.
        ignore_words: IgnoreWords = IgnoreWords::default(),
        /// Comma separated list of regular expressions matching words to ignore.
        ignore_words_regex: Vec<Regex> = Vec::new(),
    }
}

#[derive(Clone, Default, Debug)]
pub struct RuleRF05 {
    quoted_identifiers_policy: IdentifiersPolicy,
    unquoted_identifiers_policy: IdentifiersPolicy,
    allow_space_in_identifier: bool,
    additional_allowed_characters: Option<String>,
    ignore_words: IgnoreWords,
    ignore_words_regex: Vec<Regex>,
}

impl Rule for RuleRF05 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleRF05Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleRF05Config::from_config(config)?;

        Ok(RuleRF05 {
            unquoted_identifiers_policy: config.unquoted_identifiers_policy,
            quoted_identifiers_policy: config.quoted_identifiers_policy,
            ignore_words: config.ignore_words,
            ignore_words_regex: config.ignore_words_regex,
            allow_space_in_identifier: config.allow_space_in_identifier,
            additional_allowed_characters: config.additional_allowed_characters,
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "references.special_chars"
    }

    fn description(&self) -> &'static str {
        "Do not use special characters in identifiers."
    }

    fn long_description(&self) -> &'static str {
        r"
**Anti-pattern**

Using special characters within identifiers when creating or aliasing objects.

```sql
CREATE TABLE DBO.ColumnNames
(
    [Internal Space] INT,
    [Greater>Than] INT,
    [Less<Than] INT,
    Number# INT
)
```

**Best practice**

Identifiers should include only alphanumerics and underscores.

```sql
CREATE TABLE DBO.ColumnNames
(
    [Internal_Space] INT,
    [GreaterThan] INT,
    [LessThan] INT,
    NumberVal INT
)
```
"
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::References]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if self
            .ignore_words
            .contains(&context.segment.raw().to_lowercase())
            || self
                .ignore_words_regex
                .iter()
                .any(|it| it.is_match(context.segment.raw().as_ref()))
        {
            return Vec::new();
        }

        let mut policy = self.unquoted_identifiers_policy;
        let mut identifier = context.segment.raw().to_string();

        if context.segment.is_type(SyntaxKind::QuotedIdentifier) {
            policy = self.quoted_identifiers_policy;
            identifier = identifier[1..identifier.len() - 1].to_string();

            if self.ignore_words.contains(&identifier.to_lowercase())
                || self
                    .ignore_words_regex
                    .iter()
                    .any(|it| it.is_match(&identifier))
            {
                return Vec::new();
            }

            if context.dialect.name == DialectKind::Bigquery
                && context
                    .parent_stack
                    .last()
                    .is_some_and(|it| it.is_type(SyntaxKind::TableReference))
            {
                if identifier.ends_with('*') {
                    identifier.pop();
                }
                identifier = identifier.replace(".", "");
            }

            // TODO: add databricks
            if context.dialect.name == DialectKind::Sparksql && !context.parent_stack.is_empty() {
                if context
                    .parent_stack
                    .last()
                    .unwrap()
                    .is_type(SyntaxKind::FileReference)
                {
                    return Vec::new();
                }

                if context
                    .parent_stack
                    .last()
                    .unwrap()
                    .is_type(SyntaxKind::PropertyNameIdentifier)
                {
                    identifier = identifier.replace(".", "");
                }
            }

            if self.allow_space_in_identifier {
                identifier = identifier.replace(" ", "");
            }
        }

        identifier = identifier.replace("_", "");

        if context.dialect.name == DialectKind::Redshift
            && identifier.starts_with('#')
            && context
                .parent_stack
                .last()
                .is_some_and(|it| it.get_type() == SyntaxKind::TableReference)
        {
            identifier = identifier[1..].to_string();
        }

        // T-SQL allows # at the end of identifiers (SQL Server 2017+)
        if context.dialect.name == DialectKind::Tsql && identifier.ends_with('#') {
            identifier = identifier[..identifier.len() - 1].to_string();
        }

        let additional_allowed_characters =
            self.get_additional_allowed_characters(context.dialect.name);
        if !additional_allowed_characters.is_empty() {
            identifier.retain(|it| !additional_allowed_characters.contains(&it));
        }

        if identifiers_policy_applicable(policy, &context.parent_stack)
            && !identifier.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return vec![LintResult::new(
                context.segment.clone().into(),
                Vec::new(),
                None,
                None,
            )];
        }

        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const { SyntaxSet::new(&[SyntaxKind::QuotedIdentifier, SyntaxKind::NakedIdentifier]) },
        )
        .into()
    }
}

impl RuleRF05 {
    fn get_additional_allowed_characters(&self, dialect_name: DialectKind) -> HashSet<char> {
        let mut result = HashSet::new();
        result.extend(
            self.additional_allowed_characters
                .iter()
                .flat_map(|characters| characters.chars()),
        );
        if dialect_name == DialectKind::Bigquery {
            result.insert('-');
        }
        result
    }
}
