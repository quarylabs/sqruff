use std::fmt::Debug;

use ahash::{AHashMap, AHashSet};
use smol_str::SmolStr;

use crate::core::config::Value;
use crate::core::dialects::common::{AliasInfo, ColumnAliasInfo};
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::ansi::ObjectReferenceSegment;
use crate::helpers::IndexSet;
use crate::utils::analysis::select::get_select_statement_info;

#[derive(Debug, Clone)]
pub struct RuleAL04<T = ()> {
    pub(crate) lint_references_and_aliases: fn(
        Vec<AliasInfo>,
        Vec<SmolStr>,
        Vec<ObjectReferenceSegment>,
        Vec<ColumnAliasInfo>,
        Vec<SmolStr>,
        &T,
    ) -> Vec<LintResult>,
    pub(crate) context: T,
}

impl Default for RuleAL04 {
    fn default() -> Self {
        RuleAL04 { lint_references_and_aliases: Self::lint_references_and_aliases, context: () }
    }
}

impl<T: Clone + Debug + Send + Sync + 'static> Rule for RuleAL04<T> {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL04::default().erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.unique.table"
    }

    fn description(&self) -> &'static str {
        "Table aliases should be unique within each clause."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the alias t is reused for two different tables:

```sql
SELECT
    t.a,
    t.b
FROM foo AS t, bar AS t

-- This can also happen when using schemas where the
-- implicit alias is the table name:

SELECT
    a,
    b
FROM
    2020.foo,
    2021.foo
```

**Best practice**

Make all tables have a unique alias.

```sql
SELECT
    f.a,
    b.b
FROM foo AS f, bar AS b

-- Also use explicit aliases when referencing two tables
-- with the same name from two different schemas.

SELECT
    f1.a,
    f2.b
FROM
    2020.foo AS f1,
    2021.foo AS f2
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let Some(select_info) =
            get_select_statement_info(&context.segment, context.dialect.into(), true)
        else {
            return Vec::new();
        };

        let _parent_select =
            context.parent_stack.iter().rev().find(|seg| seg.is_type("select_statement"));

        (self.lint_references_and_aliases)(
            select_info.table_aliases,
            select_info.standalone_aliases,
            select_info.reference_buffer,
            select_info.col_aliases,
            select_info.using_cols,
            &self.context,
        )
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["select_statement"].into()).into()
    }
}

impl RuleAL04 {
    pub fn lint_references_and_aliases(
        table_aliases: Vec<AliasInfo>,
        _: Vec<SmolStr>,
        _: Vec<ObjectReferenceSegment>,
        _: Vec<ColumnAliasInfo>,
        _: Vec<SmolStr>,
        _: &(),
    ) -> Vec<LintResult> {
        let mut duplicates = IndexSet::default();
        let mut seen: AHashSet<_> = AHashSet::new();

        for alias in table_aliases.iter() {
            if !seen.insert(&alias.ref_str) && !alias.ref_str.is_empty() {
                duplicates.insert(alias);
            }
        }

        duplicates
            .into_iter()
            .map(|alias| {
                LintResult::new(
                    alias.segment.clone(),
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
            .collect()
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
