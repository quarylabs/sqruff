use ahash::{AHashMap, AHashSet};
use itertools::Itertools;
use regex::Regex;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::helpers::capitalize;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintPhase, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

fn is_capitalizable(character: char) -> bool {
    character.to_lowercase().ne(character.to_uppercase())
}

#[derive(Debug, Clone)]
pub struct RuleCP01 {
    pub(crate) capitalisation_policy: String,
    pub(crate) ignore_words: Vec<String>,
    pub(crate) ignore_words_regex: Vec<Regex>,
    pub(crate) cap_policy_name: String,
    pub(crate) skip_literals: bool,
    pub(crate) exclude_parent_types: &'static [SyntaxKind],
    pub(crate) description_elem: &'static str,
}

impl Default for RuleCP01 {
    fn default() -> Self {
        Self {
            capitalisation_policy: "consistent".into(),
            cap_policy_name: "capitalisation_policy".into(),
            skip_literals: true,
            exclude_parent_types: &[
                SyntaxKind::DataType,
                SyntaxKind::DatetimeTypeIdentifier,
                SyntaxKind::PrimitiveType,
                SyntaxKind::NakedIdentifier,
            ],
            description_elem: "Keywords",
            ignore_words: Vec::new(),
            ignore_words_regex: Vec::new(),
        }
    }
}

impl Rule for RuleCP01 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCP01 {
            capitalisation_policy: config["capitalisation_policy"].as_string().unwrap().into(),
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

        if self
            .ignore_words
            .contains(&context.segment.raw().to_lowercase())
        {
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
            &self.capitalisation_policy,
            &self.cap_policy_name,
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
struct RefutedCases(AHashSet<&'static str>);

#[derive(Clone)]
struct LatestPossibleCase(String);

pub fn handle_segment(
    description_elem: &str,
    extended_capitalisation_policy: &str,
    cap_policy_name: &str,
    seg: ErasedSegment,
    context: &RuleContext,
) -> LintResult {
    if seg.raw().is_empty() || seg.is_templated() {
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
        refuted_cases.extend(["upper", "capitalise", "pascal"]);
        if seg.raw().as_str() != seg.raw().to_lowercase() {
            refuted_cases.insert("lower");
        }
    } else {
        refuted_cases.insert("lower");

        let segment_raw = seg.raw();
        if segment_raw.as_str() != segment_raw.to_uppercase() {
            refuted_cases.insert("upper");
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
            refuted_cases.insert("capitalise");
        }
        if !segment_raw.chars().all(|c| c.is_alphanumeric()) {
            refuted_cases.insert("pascal");
        }
    }

    context.set(RefutedCases(refuted_cases.clone()));

    let concrete_policy = if extended_capitalisation_policy == "consistent" {
        let cap_policy_opts = match cap_policy_name {
            "capitalisation_policy" => ["upper", "lower", "capitalise"].as_slice(),
            "extended_capitalisation_policy" => {
                ["upper", "lower", "pascal", "capitalise"].as_slice()
            }
            _ => unimplemented!("Unknown capitalisation policy name: {cap_policy_name}"),
        };

        let possible_cases = cap_policy_opts
            .iter()
            .filter(|&it| !refuted_cases.contains(it))
            .collect_vec();

        if !possible_cases.is_empty() {
            context.set(LatestPossibleCase(possible_cases[0].to_string()));
            return LintResult::new(None, Vec::new(), None, None);
        } else {
            context
                .try_get::<LatestPossibleCase>()
                .unwrap_or_else(|| LatestPossibleCase("upper".into()))
                .0
        }
    } else {
        extended_capitalisation_policy.to_string()
    };

    let concrete_policy = concrete_policy.as_str();

    let mut fixed_raw = seg.raw().to_string();
    fixed_raw = match concrete_policy {
        "upper" => fixed_raw.to_uppercase(),
        "lower" => fixed_raw.to_lowercase(),
        "capitalise" => capitalize(&fixed_raw),
        "pascal" => {
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
        _ => fixed_raw,
    };

    if fixed_raw == seg.raw().as_str() {
        LintResult::new(None, Vec::new(), None, None)
    } else {
        let consistency = if extended_capitalisation_policy == "consistent" {
            "consistently "
        } else {
            ""
        };
        let policy = match concrete_policy {
            concrete_policy @ ("upper" | "lower") => format!("{} case.", concrete_policy),
            "capitalise" => "capitalised.".to_string(),
            "pascal" => "pascal case.".to_string(),
            _ => "".to_string(),
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
