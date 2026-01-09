use ahash::AHashSet;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};
use crate::utils::identifers::identifiers_policy_applicable;

#[derive(Clone, Default, Debug)]
pub struct RuleRF05;

impl Rule for RuleRF05 {
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
        let rules = &context.config.rules.references_special_chars;
        if rules
            .ignore_words
            .iter()
            .any(|word| word.eq_ignore_ascii_case(context.segment.raw().as_ref()))
            || rules
                .ignore_words_regex
                .iter()
                .any(|it| it.is_match(context.segment.raw().as_ref()))
        {
            return Vec::new();
        }

        let mut policy = rules.unquoted_identifiers_policy.as_str();
        let mut identifier = context.segment.raw().to_string();

        if context.segment.is_type(SyntaxKind::QuotedIdentifier) {
            policy = rules.quoted_identifiers_policy.as_str();
            identifier = identifier[1..identifier.len() - 1].to_string();

            if rules
                .ignore_words
                .iter()
                .any(|word| word.eq_ignore_ascii_case(&identifier))
                || rules
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

            if rules.allow_space_in_identifier {
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

        let additional_allowed_characters = Self::get_additional_allowed_characters(
            rules.additional_allowed_characters.as_deref().unwrap_or(""),
            context.dialect.name,
        );
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
    fn get_additional_allowed_characters(
        additional_allowed_characters: &str,
        dialect_name: DialectKind,
    ) -> AHashSet<char> {
        let mut result = AHashSet::new();
        result.extend(additional_allowed_characters.chars());
        if dialect_name == DialectKind::Bigquery {
            result.insert('-');
        }
        result
    }
}
