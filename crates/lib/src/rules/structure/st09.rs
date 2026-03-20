use hashbrown::HashMap;
use itertools::Itertools;
use smol_str::{SmolStr, StrExt};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::from::FromExpressionElementSegment;
use sqruff_lib_core::parser::segments::join::JoinClauseSegment;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

const REORDERABLE_OPERATORS: &[&str] = &["=", "!=", "<>", "<=>", "<", ">", "<=", ">="];

fn normalize_identifier(raw: &str) -> SmolStr {
    let is_bracket_quoted = raw.starts_with('[') && raw.ends_with(']') && raw.len() >= 2;
    let is_matching_quote_quoted = matches!(raw.chars().next(), Some('"') | Some('\'') | Some('`'))
        && raw.len() >= 2
        && raw.chars().next() == raw.chars().last();

    if is_bracket_quoted || is_matching_quote_quoted {
        raw[1..raw.len() - 1].into()
    } else {
        raw.into()
    }
}

#[derive(Default, Debug, Clone)]
pub struct RuleST09 {
    preferred_first_table_in_join_clause: String,
}

impl Rule for RuleST09 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        match config["preferred_first_table_in_join_clause"].as_string() {
            Some("earlier" | "later") => Ok(RuleST09 {
                preferred_first_table_in_join_clause:
                    config["preferred_first_table_in_join_clause"]
                        .as_string()
                        .unwrap()
                        .to_owned(),
            }
            .erased()),
            Some(value) => Err(format!(
                "Invalid value for preferred_first_table_in_join_clause: {value}. Must be one of \
                 [earlier, later]"
            )),
            None => {
                Err("Rule ST09 expects a string for `preferred_first_table_in_join_clause`".into())
            }
        }
    }

    fn name(&self) -> &'static str {
        "structure.join_condition_order"
    }

    fn description(&self) -> &'static str {
        "Joins should list the table referenced earlier/later first."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the tables that were referenced later are listed first
and the `preferred_first_table_in_join_clause` configuration
is set to `earlier`.

```sql
select
    foo.a,
    foo.b,
    bar.c
from foo
left join bar
    -- This subcondition does not list
    -- the table referenced earlier first:
    on bar.a = foo.a
    -- Neither does this subcondition:
    and bar.b = foo.b
```

**Best practice**

List the tables that were referenced earlier first.

```sql
select
    foo.a,
    foo.b,
    bar.c
from foo
left join bar
    on foo.a = bar.a
    and foo.b = bar.b
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut table_aliases = Vec::new();
        let children = FunctionalContext::new(context).segment().children_all();
        let join_clauses =
            children.recursive_crawl(const { &SyntaxSet::new(&[SyntaxKind::JoinClause]) }, true);
        let join_on_conditions = join_clauses.children_all().recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::JoinOnCondition]) },
            true,
        );

        if join_on_conditions.is_empty() {
            return Vec::new();
        }

        let from_expression_alias_info = FromExpressionElementSegment(
            children.recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) },
                true,
            )[0]
            .clone(),
        )
        .eventual_alias();
        let from_expression_alias = from_expression_alias_info
            .segment
            .as_ref()
            .map(|segment| normalize_identifier(segment.raw()))
            .unwrap_or_else(|| normalize_identifier(from_expression_alias_info.ref_str.as_str()));

        table_aliases.push(from_expression_alias);

        let mut join_clause_aliases = join_clauses
            .into_iter()
            .map(|join_clause| {
                JoinClauseSegment(join_clause)
                    .eventual_aliases()
                    .first()
                    .unwrap()
                    .1
                    .clone()
            })
            .map(|alias_info| {
                alias_info
                    .segment
                    .as_ref()
                    .map(|segment| normalize_identifier(segment.raw()))
                    .unwrap_or_else(|| normalize_identifier(alias_info.ref_str.as_str()))
            })
            .collect_vec();

        table_aliases.append(&mut join_clause_aliases);

        let table_aliases = table_aliases
            .iter()
            .map(|it| it.to_uppercase_smolstr())
            .collect_vec();
        let mut conditions = Vec::new();

        let join_on_condition_expressions = join_on_conditions
            .children_all()
            .recursive_crawl(const { &SyntaxSet::new(&[SyntaxKind::Expression]) }, true);

        for expression in join_on_condition_expressions {
            let mut expression_group = Vec::new();
            for element in Segments::new(expression, None).children_all() {
                if !matches!(
                    element.get_type(),
                    SyntaxKind::Whitespace | SyntaxKind::Newline
                ) {
                    expression_group.push(element);
                }
            }
            conditions.push(expression_group);
        }

        let mut subconditions = Vec::new();

        for expression_group in conditions {
            subconditions.append(&mut split_list_by_segment_type(
                expression_group,
                SyntaxKind::BinaryOperator,
                vec!["and".into(), "or".into()],
            ));
        }

        let column_operator_column_subconditions = subconditions
            .into_iter()
            .filter(|it| is_qualified_column_operator_qualified_column_sequence(it))
            .collect_vec();

        let mut fixes = Vec::new();

        for subcondition in column_operator_column_subconditions {
            let comparison_operator = subcondition[1].clone();
            let first_column_reference = subcondition[0].clone();
            let second_column_reference = subcondition[2].clone();
            let raw_comparison_operators: Vec<_> = comparison_operator
                .children(const { &SyntaxSet::new(&[SyntaxKind::RawComparisonOperator]) })
                .collect();
            let operator_str = if raw_comparison_operators.is_empty() {
                comparison_operator.raw().trim().to_owned()
            } else {
                raw_comparison_operators.iter().map(|it| it.raw()).join("")
            };

            if !REORDERABLE_OPERATORS.contains(&operator_str.as_str()) {
                continue;
            }

            let first_table_seg = first_column_reference
                .child(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::QuotedIdentifier,
                        ])
                    },
                )
                .unwrap();
            let second_table_seg = second_column_reference
                .child(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::QuotedIdentifier,
                        ])
                    },
                )
                .unwrap();

            let first_table = normalize_identifier(first_table_seg.raw()).to_uppercase_smolstr();
            let second_table = normalize_identifier(second_table_seg.raw()).to_uppercase_smolstr();

            let raw_comparison_operator_opposites = |op| match op {
                "<" => ">",
                ">" => "<",
                _ => unimplemented!(),
            };

            if !table_aliases.contains(&first_table) || !table_aliases.contains(&second_table) {
                continue;
            }

            if (table_aliases
                .iter()
                .position(|x| x == &first_table)
                .unwrap()
                > table_aliases
                    .iter()
                    .position(|x| x == &second_table)
                    .unwrap()
                && self.preferred_first_table_in_join_clause == "earlier")
                || (table_aliases
                    .iter()
                    .position(|x| x == &first_table)
                    .unwrap()
                    < table_aliases
                        .iter()
                        .position(|x| x == &second_table)
                        .unwrap()
                    && self.preferred_first_table_in_join_clause == "later")
            {
                fixes.push(LintFix::replace(
                    first_column_reference.clone(),
                    vec![second_column_reference.clone()],
                    None,
                ));
                fixes.push(LintFix::replace(
                    second_column_reference.clone(),
                    vec![first_column_reference.clone()],
                    None,
                ));

                if raw_comparison_operators
                    .first()
                    .is_some_and(|op| matches!(op.raw().as_ref(), "<" | ">"))
                    && raw_comparison_operators
                        .iter()
                        .map(|it| it.raw())
                        .ne(["<", ">"])
                {
                    fixes.push(LintFix::replace(
                        raw_comparison_operators[0].clone(),
                        vec![
                            SegmentBuilder::token(
                                context.tables.next_id(),
                                raw_comparison_operator_opposites(
                                    raw_comparison_operators[0].raw().as_ref(),
                                ),
                                SyntaxKind::RawComparisonOperator,
                            )
                            .finish(),
                        ],
                        None,
                    ));
                }
            }
        }

        if fixes.is_empty() {
            return Vec::new();
        }

        vec![LintResult::new(
            context.segment.clone().into(),
            fixes,
            format!(
                "Joins should list the table referenced {} first.",
                self.preferred_first_table_in_join_clause
            )
            .into(),
            None,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::FromExpression]) }).into()
    }
}

fn split_list_by_segment_type(
    segment_list: Vec<ErasedSegment>,
    delimiter_type: SyntaxKind,
    delimiters: Vec<SmolStr>,
) -> Vec<Vec<ErasedSegment>> {
    let delimiters = delimiters
        .into_iter()
        .map(|it| it.to_uppercase_smolstr())
        .collect_vec();
    let mut new_list = Vec::new();
    let mut sub_list = Vec::new();

    for i in 0..segment_list.len() {
        if i == segment_list.len() - 1 {
            sub_list.push(segment_list[i].clone());
            new_list.push(sub_list.clone());
        } else if segment_list[i].get_type() == delimiter_type
            && delimiters.contains(&segment_list[i].raw().to_uppercase_smolstr())
        {
            new_list.push(sub_list.clone());
            sub_list.clear();
        } else {
            sub_list.push(segment_list[i].clone());
        }
    }

    new_list
}

fn is_qualified_column_operator_qualified_column_sequence(segment_list: &[ErasedSegment]) -> bool {
    if segment_list.len() != 3 {
        return false;
    }

    if segment_list[0].get_type() == SyntaxKind::ColumnReference
        && segment_list[0]
            .direct_descendant_type_set()
            .contains(SyntaxKind::Dot)
        && segment_list[1].get_type() == SyntaxKind::ComparisonOperator
        && segment_list[2].get_type() == SyntaxKind::ColumnReference
        && segment_list[2]
            .direct_descendant_type_set()
            .contains(SyntaxKind::Dot)
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use hashbrown::HashMap;

    use crate::core::config::Value;

    #[test]
    fn st09_is_fix_compatible() {
        assert!(RuleST09::default().is_fix_compatible());
    }

    #[test]
    fn st09_description_matches_python() {
        let rule = RuleST09 {
            preferred_first_table_in_join_clause: "earlier".into(),
        };

        let result = format!(
            "Joins should list the table referenced {} first.",
            rule.preferred_first_table_in_join_clause
        );
        assert_eq!(
            result,
            "Joins should list the table referenced earlier first."
        );
    }

    #[test]
    fn st09_load_from_config_rejects_invalid_value() {
        let config = HashMap::from_iter([(
            "preferred_first_table_in_join_clause".into(),
            Value::String("middle".into()),
        )]);

        let err = RuleST09::default().load_from_config(&config).unwrap_err();
        assert_eq!(
            err,
            "Invalid value for preferred_first_table_in_join_clause: middle. Must be one of \
             [earlier, later]"
        );
    }

    #[test]
    fn st09_load_from_config_accepts_valid_value() {
        let config = HashMap::from_iter([(
            "preferred_first_table_in_join_clause".into(),
            Value::String("later".into()),
        )]);

        assert!(RuleST09::default().load_from_config(&config).is_ok());
    }
}
