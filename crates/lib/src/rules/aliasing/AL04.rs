use ahash::{AHashMap, AHashSet};

use crate::core::config::Value;
use crate::core::dialects::common::AliasInfo;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::IndexSet;
use crate::utils::analysis::select::get_select_statement_info;

#[derive(Debug, Clone, Default)]
pub struct RuleAL04 {}

impl Rule for RuleAL04 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleAL04::default().erased()
    }

    fn name(&self) -> &'static str {
        "aliasing.unique.table"
    }

    fn description(&self) -> &'static str {
        "Table aliases should be unique within each clause."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let Some(select_info) =
            get_select_statement_info(&context.segment, context.dialect.into(), true)
        else {
            return Vec::new();
        };

        let _parent_select =
            context.parent_stack.iter().rev().find(|seg| seg.is_type("select_statement"));

        self.lint_references_and_aliases(select_info.table_aliases).unwrap_or_default()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["select_statement"].into()).into()
    }
}

impl RuleAL04 {
    pub fn lint_references_and_aliases(
        &self,
        table_aliases: Vec<AliasInfo>,
    ) -> Option<Vec<LintResult>> {
        let mut duplicates = IndexSet::default();
        let mut seen: AHashSet<String> = AHashSet::new();

        for alias in table_aliases.iter() {
            if seen.contains(&alias.ref_str) && !alias.ref_str.is_empty() {
                duplicates.insert(alias.clone());
            } else {
                seen.insert(alias.ref_str.clone());
            }
        }

        if duplicates.is_empty() {
            None
        } else {
            Some(
                duplicates
                    .into_iter()
                    .map(|alias| {
                        LintResult::new(
                            alias.segment,
                            Vec::new(),
                            None,
                            format!(
                                "Duplicate table alias '{}'. Table aliases should be unique.",
                                alias.ref_str
                            )
                            .into(),
                            None,
                        )
                    })
                    .collect(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::lint;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::aliasing::AL04::RuleAL04;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleAL04::default().erased()]
    }

    #[test]
    fn test_fail_exactly_once_duplicated_aliases() {
        let sql = "select 1 from table_1 as a join table_2 as a using(pk)";
        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(
            violations[0].desc(),
            "Duplicate table alias 'a'. Table aliases should be unique."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 44);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_two_duplicated_aliases() {
        let sql = r#"
        select 1
        from table_1 as a
        join table_2 as a on a.pk = b.pk
        join table_3 as b on a.pk = b.pk
        join table_4 as b on b.pk = b.pk
    "#;
        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(
            violations[0].desc(),
            "Duplicate table alias 'a'. Table aliases should be unique."
        );
        assert_eq!(violations[0].line_no, 4);
        assert_eq!(violations[0].line_pos, 25);

        assert_eq!(
            violations[1].desc(),
            "Duplicate table alias 'b'. Table aliases should be unique."
        );
        assert_eq!(violations[1].line_no, 6);
        assert_eq!(violations[1].line_pos, 25);

        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_fail_subquery() {
        let sql = r#"
        SELECT 1
        FROM (
            select 1
            from table_1 as a
            join table_2 as a on a.pk = b.pk
            join table_3 as b on a.pk = b.pk
            join table_4 as b on b.pk = b.pk
        )
    "#;
        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(
            violations[0].desc(),
            "Duplicate table alias 'a'. Table aliases should be unique."
        );
        assert_eq!(violations[0].line_no, 6);
        assert_eq!(violations[0].line_pos, 29);

        assert_eq!(
            violations[1].desc(),
            "Duplicate table alias 'b'. Table aliases should be unique."
        );
        assert_eq!(violations[1].line_no, 8);
        assert_eq!(violations[1].line_pos, 29);

        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_pass_subquery() {
        let sql = r#"
        SELECT 1
        FROM (
            select 1
            from table_1 as a
            join table_2 as b on a.pk = b.pk
        ) AS a
    "#;
        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(violations.len(), 0);
    }
}
