use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use regex::Regex;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::helpers::capitalize;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::config::{IgnoreWords, RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintPhase, LintResult, Rule, RuleGroups};

fn is_capitalizable(character: char) -> bool {
    character.to_lowercase().ne(character.to_uppercase())
}

crate::rule_config_enum! {
    /// The capitalisation styles available to keywords and literals.
    #[derive(Default)]
    pub enum CapitalisationPolicy {
        /// Any style, as long as the file sticks to one of them.
        #[default]
        Consistent => "consistent",
        /// `UPPER CASE`.
        Upper => "upper",
        /// `lower case`.
        Lower => "lower",
        /// `Capitalised case`.
        Capitalise => "capitalise",
    }
}

crate::rule_config_enum! {
    /// The capitalisation styles available to identifiers, functions and types.
    ///
    /// This is [`CapitalisationPolicy`] plus `pascal`.
    #[derive(Default)]
    pub enum ExtendedCapitalisationPolicy {
        /// Any style, as long as the file sticks to one of them.
        #[default]
        Consistent => "consistent",
        /// `UPPER CASE`.
        Upper => "upper",
        /// `lower case`.
        Lower => "lower",
        /// `PascalCase`.
        Pascal => "pascal",
        /// `Capitalised case`.
        Capitalise => "capitalise",
    }
}

impl From<CapitalisationPolicy> for ExtendedCapitalisationPolicy {
    fn from(value: CapitalisationPolicy) -> Self {
        match value {
            CapitalisationPolicy::Consistent => Self::Consistent,
            CapitalisationPolicy::Upper => Self::Upper,
            CapitalisationPolicy::Lower => Self::Lower,
            CapitalisationPolicy::Capitalise => Self::Capitalise,
        }
    }
}

/// Which set of concrete styles a `consistent` policy may settle on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapitalisationPolicyName {
    /// The styles of [`CapitalisationPolicy`].
    #[default]
    Capitalisation,
    /// The styles of [`ExtendedCapitalisationPolicy`], i.e. including `pascal`.
    Extended,
}

impl CapitalisationPolicyName {
    /// The concrete styles a `consistent` policy may settle on, in preference
    /// order.
    fn candidates(self) -> &'static [ExtendedCapitalisationPolicy] {
        match self {
            Self::Capitalisation => &[
                ExtendedCapitalisationPolicy::Upper,
                ExtendedCapitalisationPolicy::Lower,
                ExtendedCapitalisationPolicy::Capitalise,
            ],
            Self::Extended => &[
                ExtendedCapitalisationPolicy::Upper,
                ExtendedCapitalisationPolicy::Lower,
                ExtendedCapitalisationPolicy::Pascal,
                ExtendedCapitalisationPolicy::Capitalise,
            ],
        }
    }
}

crate::rule_config! {
    /// Configuration for `capitalisation.keywords` (CP01).
    RuleCP01Config {
        /// The capitalisation to enforce on keywords.
        capitalisation_policy: CapitalisationPolicy = CapitalisationPolicy::Consistent,
        /// Comma separated list of words to ignore, compared case-insensitively.
        ignore_words: IgnoreWords = IgnoreWords::default(),
        /// Comma separated list of regular expressions matching words to ignore.
        ignore_words_regex: Vec<Regex> = Vec::new(),
    }
}

#[derive(Debug, Clone)]
pub struct RuleCP01 {
    pub(crate) capitalisation_policy: ExtendedCapitalisationPolicy,
    pub(crate) ignore_words: IgnoreWords,
    pub(crate) ignore_words_regex: Vec<Regex>,
    pub(crate) cap_policy_name: CapitalisationPolicyName,
    pub(crate) skip_literals: bool,
    pub(crate) exclude_parent_types: &'static [SyntaxKind],
    pub(crate) description_elem: &'static str,
}

impl Default for RuleCP01 {
    fn default() -> Self {
        Self {
            capitalisation_policy: ExtendedCapitalisationPolicy::Consistent,
            cap_policy_name: CapitalisationPolicyName::Capitalisation,
            skip_literals: true,
            exclude_parent_types: &[
                SyntaxKind::DataType,
                SyntaxKind::DatetimeTypeIdentifier,
                SyntaxKind::PrimitiveType,
                SyntaxKind::NakedIdentifier,
            ],
            description_elem: "Keywords",
            ignore_words: IgnoreWords::default(),
            ignore_words_regex: Vec::new(),
        }
    }
}

impl Rule for RuleCP01 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleCP01Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleCP01Config::from_config(config)?;

        Ok(RuleCP01 {
            capitalisation_policy: config.capitalisation_policy.into(),
            ignore_words: config.ignore_words,
            ignore_words_regex: config.ignore_words_regex,
            ..Default::default()
        }
        .erased())
    }

    fn lint_phase(&self) -> LintPhase {
        LintPhase::Post
    }

    fn name(&self) -> &'static str {
        "capitalisation.keywords"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of keywords."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, select is in lower-case whereas `FROM` is in upper-case.

```sql
select
    a
FROM foo
```

**Best practice**

Make all keywords either in upper-case or in lower-case.

```sql
SELECT
    a
FROM foo

-- Also good

select
    a
from foo
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
        let parent = context.parent_stack.last().unwrap();

        if self.ignore_words.matches(context.segment.raw().as_ref()) {
            return Vec::new();
        }

        if self
            .ignore_words_regex
            .iter()
            .any(|regex| regex.is_match(context.segment.raw().as_ref()))
        {
            return Vec::new();
        }

        if (self.skip_literals && context.segment.is_type(SyntaxKind::Literal))
            || !self.exclude_parent_types.is_empty()
                && self
                    .exclude_parent_types
                    .iter()
                    .any(|&it| parent.is_type(it))
        {
            return vec![LintResult::new(None, Vec::new(), None, None)];
        }

        if parent.get_type() == SyntaxKind::FunctionName && parent.segments().len() != 1 {
            return vec![LintResult::new(None, Vec::new(), None, None)];
        }

        vec![handle_segment(
            self.description_elem,
            self.capitalisation_policy,
            self.cap_policy_name,
            context.segment.clone(),
            context,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::Keyword,
                    SyntaxKind::BinaryOperator,
                    SyntaxKind::DatePart,
                ])
            },
        )
        .into()
    }
}

#[derive(Clone, Default)]
struct RefutedCases(HashSet<ExtendedCapitalisationPolicy>);

#[derive(Clone)]
struct LatestPossibleCase(ExtendedCapitalisationPolicy);

pub fn handle_segment(
    description_elem: &str,
    extended_capitalisation_policy: ExtendedCapitalisationPolicy,
    cap_policy_name: CapitalisationPolicyName,
    seg: ErasedSegment,
    context: &RuleContext,
) -> LintResult {
    // Skip templated segments only when configured to ignore templated areas (#4697).
    // Default is true, matching the previous unconditional skip.
    let ignore_templated_areas = context
        .config
        .get("ignore_templated_areas", "core")
        .as_bool()
        .unwrap_or(true);
    if seg.raw().is_empty() || (seg.is_templated() && ignore_templated_areas) {
        return LintResult::new(None, Vec::new(), None, None);
    }

    let mut refuted_cases = context.try_get::<RefutedCases>().unwrap_or_default().0;

    let mut first_letter_is_lowercase = false;
    for ch in seg.raw().chars() {
        if is_capitalizable(ch) {
            first_letter_is_lowercase = Some(ch).into_iter().ne(ch.to_uppercase());
            break;
        }
        first_letter_is_lowercase = false;
    }

    if first_letter_is_lowercase {
        refuted_cases.extend([
            ExtendedCapitalisationPolicy::Upper,
            ExtendedCapitalisationPolicy::Capitalise,
            ExtendedCapitalisationPolicy::Pascal,
        ]);
        if seg.raw().as_str() != seg.raw().to_lowercase() {
            refuted_cases.insert(ExtendedCapitalisationPolicy::Lower);
        }
    } else {
        refuted_cases.insert(ExtendedCapitalisationPolicy::Lower);

        let segment_raw = seg.raw();
        if segment_raw.as_str() != segment_raw.to_uppercase() {
            refuted_cases.insert(ExtendedCapitalisationPolicy::Upper);
        }
        if segment_raw.as_str()
            != segment_raw
                .to_uppercase()
                .chars()
                .next()
                .unwrap()
                .to_string()
                + segment_raw[1..].to_lowercase().as_str()
        {
            refuted_cases.insert(ExtendedCapitalisationPolicy::Capitalise);
        }
        if !segment_raw.chars().all(|c| c.is_alphanumeric()) {
            refuted_cases.insert(ExtendedCapitalisationPolicy::Pascal);
        }
    }

    context.set(RefutedCases(refuted_cases.clone()));

    let concrete_policy =
        if extended_capitalisation_policy == ExtendedCapitalisationPolicy::Consistent {
            let possible_cases = cap_policy_name
                .candidates()
                .iter()
                .filter(|it| !refuted_cases.contains(*it))
                .collect_vec();

            if !possible_cases.is_empty() {
                context.set(LatestPossibleCase(*possible_cases[0]));
                return LintResult::new(None, Vec::new(), None, None);
            } else {
                context
                    .try_get::<LatestPossibleCase>()
                    .unwrap_or(LatestPossibleCase(ExtendedCapitalisationPolicy::Upper))
                    .0
            }
        } else {
            extended_capitalisation_policy
        };

    let mut fixed_raw = seg.raw().to_string();
    fixed_raw = match concrete_policy {
        ExtendedCapitalisationPolicy::Upper => fixed_raw.to_uppercase(),
        ExtendedCapitalisationPolicy::Lower => fixed_raw.to_lowercase(),
        ExtendedCapitalisationPolicy::Capitalise => capitalize(&fixed_raw),
        ExtendedCapitalisationPolicy::Pascal => {
            let re = lazy_regex::regex!(r"([^a-zA-Z0-9]+|^)([a-zA-Z0-9])([a-zA-Z0-9]*)");
            re.replace_all(&fixed_raw, |caps: &regex::Captures| {
                let mut replacement_string = String::from(&caps[1]);
                let capitalized = caps[2].to_uppercase();
                replacement_string.push_str(&capitalized);
                replacement_string.push_str(&caps[3]);
                replacement_string
            })
            .into()
        }
        ExtendedCapitalisationPolicy::Consistent => fixed_raw,
    };

    if fixed_raw == seg.raw().as_str() {
        LintResult::new(None, Vec::new(), None, None)
    } else {
        let consistency =
            if extended_capitalisation_policy == ExtendedCapitalisationPolicy::Consistent {
                "consistently "
            } else {
                ""
            };
        let policy =
            match concrete_policy {
                policy @ (ExtendedCapitalisationPolicy::Upper
                | ExtendedCapitalisationPolicy::Lower) => format!("{policy} case."),
                ExtendedCapitalisationPolicy::Capitalise => "capitalised.".to_string(),
                ExtendedCapitalisationPolicy::Pascal => "pascal case.".to_string(),
                ExtendedCapitalisationPolicy::Consistent => "".to_string(),
            };

        LintResult::new(
            seg.clone().into(),
            vec![LintFix::replace(
                seg.clone(),
                vec![seg.edit(context.tables.next_id(), fixed_raw.to_string().into(), None)],
                None,
            )],
            format!("{description_elem} must be {consistency}{policy}").into(),
            None,
        )
    }
}
