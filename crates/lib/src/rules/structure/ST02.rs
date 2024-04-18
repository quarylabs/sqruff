use ahash::AHashSet;
use itertools::{chain, Itertools};

use crate::core::parser::segments::base::{
    ErasedSegment, SymbolSegment, WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::ToErasedSegment;
use crate::utils::functional::context::FunctionalContext;
use crate::utils::functional::segments::Segments;

#[derive(Default, Debug, Clone)]
pub struct RuleST02 {}

impl Rule for RuleST02 {
    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["case_expression"].into()).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if context.segment.segments()[0].get_raw().unwrap().eq_ignore_ascii_case("CASE") {
            let children = FunctionalContext::new(context.clone()).segment().children(None);

            let when_clauses =
                children.select(Some(|it| it.is_type("when_clause")), None, None, None);
            let else_clauses =
                children.select(Some(|it| it.is_type("else_clause")), None, None, None);

            if when_clauses.len() > 1 {
                return Vec::new();
            }

            let condition_expression =
                when_clauses.children(Some(|it| it.is_type("expression")))[0].clone_box();
            let then_expression =
                when_clauses.children(Some(|it| it.is_type("expression")))[1].clone_box();

            if !else_clauses.is_empty() {
                if let Some(else_expression) =
                    else_clauses.children(Some(|it| it.is_type("expression"))).first()
                {
                    let upper_bools = ["TRUE", "FALSE"];

                    let then_expression_upper = then_expression.get_raw_upper().unwrap();
                    let else_expression_upper = else_expression.get_raw_upper().unwrap();

                    if upper_bools.contains(&then_expression_upper.as_str())
                        && upper_bools.contains(&else_expression_upper.as_str())
                        && then_expression_upper != else_expression_upper
                    {
                        let coalesce_arg_1 = condition_expression.clone_box();
                        let coalesce_arg_2 =
                            KeywordSegment::new("false".into(), None).to_erased_segment();
                        let preceding_not = then_expression_upper == "FALSE";

                        let fixes = Self::coalesce_fix_list(
                            &context,
                            coalesce_arg_1,
                            coalesce_arg_2,
                            preceding_not,
                        );

                        return vec![LintResult::new(
                            condition_expression.into(),
                            fixes,
                            None,
                            "Unnecessary CASE statement. Use COALESCE function instead."
                                .to_owned()
                                .into(),
                            None,
                        )];
                    }
                }
            }

            let condition_expression_segments_raw: AHashSet<String> = AHashSet::from_iter(
                condition_expression
                    .segments()
                    .iter()
                    .map(|segment| segment.get_raw_upper().unwrap()),
            );

            if condition_expression_segments_raw.contains("IS")
                && condition_expression_segments_raw.contains("NULL")
                && condition_expression_segments_raw
                    .intersection(&AHashSet::from_iter(["AND".into(), "OR".into()]))
                    .next()
                    .is_none()
            {
                let is_not_prefix = condition_expression_segments_raw.contains("NOT");

                let tmp = Segments::new(condition_expression.clone_box(), None)
                    .children(Some(|it| it.is_type("column_reference")));

                let Some(column_reference_segment) = tmp.first() else {
                    return Vec::new();
                };

                if !else_clauses.is_empty() {
                    let else_expression =
                        else_clauses.children(Some(|it| it.is_type("expression")))[0].clone();
                    let (coalesce_arg_1, coalesce_arg_2) = if !is_not_prefix
                        && column_reference_segment.get_raw_upper().unwrap()
                            == else_expression.get_raw_upper().unwrap()
                    {
                        (else_expression, then_expression)
                    } else if is_not_prefix
                        && column_reference_segment.get_raw_upper().unwrap()
                            == then_expression.get_raw_upper().unwrap()
                    {
                        (then_expression, else_expression)
                    } else {
                        return Vec::new();
                    };

                    if coalesce_arg_2.get_raw_upper().unwrap() == "NULL" {
                        let fixes = Self::column_only_fix_list(
                            &context,
                            column_reference_segment.clone_box(),
                        );
                        return vec![LintResult::new(
                            condition_expression.into(),
                            fixes,
                            None,
                            Some(String::new()),
                            None,
                        )];
                    }

                    let fixes =
                        Self::coalesce_fix_list(&context, coalesce_arg_1, coalesce_arg_2, false);

                    return vec![LintResult::new(
                        condition_expression.into(),
                        fixes,
                        None,
                        "Unnecessary CASE statement. Use COALESCE function instead."
                            .to_owned()
                            .into(),
                        None,
                    )];
                } else if column_reference_segment.get_raw_upper().unwrap()
                    == then_expression.get_raw_upper().unwrap()
                {
                    let fixes =
                        Self::column_only_fix_list(&context, column_reference_segment.clone_box());

                    return vec![LintResult::new(
                        condition_expression.into(),
                        fixes,
                        None,
                        format!(
                            "Unnecessary CASE statement. Just use column '{}'.",
                            column_reference_segment.get_raw().unwrap()
                        )
                        .into(),
                        None,
                    )];
                }
            }

            Vec::new()
        } else {
            Vec::new()
        }
    }
}

impl RuleST02 {
    fn coalesce_fix_list(
        context: &RuleContext,
        coalesce_arg_1: ErasedSegment,
        coalesce_arg_2: ErasedSegment,
        preceding_not: bool,
    ) -> Vec<LintFix> {
        let mut edits = vec![
            SymbolSegment::create("coalesce", &<_>::default(), <_>::default()),
            SymbolSegment::create("(", &<_>::default(), <_>::default()),
            coalesce_arg_1,
            SymbolSegment::create(",", &<_>::default(), <_>::default()),
            WhitespaceSegment::create(" ", &<_>::default(), WhitespaceSegmentNewArgs),
            coalesce_arg_2,
            SymbolSegment::create(")", &<_>::default(), <_>::default()),
        ];

        if preceding_not {
            edits = chain(
                [
                    KeywordSegment::new("not".into(), None).to_erased_segment(),
                    WhitespaceSegment::create(" ", &<_>::default(), WhitespaceSegmentNewArgs),
                ],
                edits,
            )
            .collect_vec();
        }

        vec![LintFix::replace(context.segment.clone_box(), edits, None)]
    }

    fn column_only_fix_list(
        context: &RuleContext,
        column_reference_segment: ErasedSegment,
    ) -> Vec<LintFix> {
        vec![LintFix::replace(context.segment.clone_box(), vec![column_reference_segment], None)]
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::structure::ST02::RuleST02;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleST02::default().erased()]
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_1() {
        let pass_str = "
select
    fab > 0 as is_fab
from fancy_table";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_2() {
        let pass_str = "
select
    case when fab > 0 then true end as is_fab
from fancy_table";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_3() {
        let pass_str = "
select
    case when fab is not null then false end as is_fab
from fancy_table";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_4() {
        let pass_str = "
select
    case when fab > 0 then true else true end as is_fab
from fancy_table";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_5() {
        let pass_str = "
select
    case when fab <> 0 then 'just a string' end as fab_category
from fancy_table";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_6() {
        let pass_str = "
select
    case
      when fab <> 0 then true
      when fab < 0 then 'not a bool'
    end as fab_category
from fancy_table";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_7() {
        let pass_str = "
select
    foo,
    case
        when
            bar is null then bar
        else '123'
    end as test
from baz;";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_8() {
        let pass_str = "
select
    foo,
    case
        when
            bar is not null then '123'
        else bar
    end as test
from baz;";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_9() {
        let pass_str = "
select
    foo,
    case
        when
            bar is not null then '123'
        when
            foo is not null then '456'
        else bar
    end as test
from baz;";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_10() {
        let pass_str = "
select
    foo,
    case
        when
            bar is not null and abs(foo) > 0 then '123'
        else bar
    end as test
from baz;";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_11() {
        let pass_str = "
SELECT
    dv_runid,
    CASE
        WHEN LEAD(dv_startdateutc) OVER (
            PARTITION BY rowid ORDER BY dv_startdateutc
        ) IS NULL
        THEN 1
        ELSE 0
    END AS loadstate
FROM d;";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_case_cannot_be_reduced_12() {
        let pass_str = "
select
    field_1,
    field_2,
    field_3,
    case
        when coalesce(field_2, field_3) is null then 1 else 0
    end as field_4
from my_table;";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    #[ignore = "dialect: postgres"]
    fn test_pass_case_cannot_be_reduced_13() {}

    #[test]
    fn test_fail_unnecessary_case_1() {
        let fail_str = "
select
    case
        when fab > 0 then true else false end as is_fab
from fancy_table";

        let fix_str = "
select
    coalesce(fab > 0, false) as is_fab
from fancy_table";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_2() {
        let fail_str = "
select
    case
        when fab > 0 then false else true end as is_fab
from fancy_table";

        let fix_str = "
select
    not coalesce(fab > 0, false) as is_fab
from fancy_table";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_3() {
        let fail_str = "
select
    case
        when fab > 0 and tot > 0 then true else false end as is_fab
from fancy_table";

        let fix_str = "
select
    coalesce(fab > 0 and tot > 0, false) as is_fab
from fancy_table";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_4() {
        let fail_str = "
select
    case
        when fab > 0 and tot > 0 then false else true end as is_fab
from fancy_table";

        let fix_str = "
select
    not coalesce(fab > 0 and tot > 0, false) as is_fab
from fancy_table";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_5() {
        let fail_str = "
select
    case
        when not fab > 0 or tot > 0 then false else true end as is_fab
from fancy_table";

        let fix_str = "
select
    not coalesce(not fab > 0 or tot > 0, false) as is_fab
from fancy_table";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_6() {
        let fail_str = "
select
    subscriptions_xf.metadata_migrated,

    case  -- BEFORE ST02 FIX
        when perks.perk is null then false
        else true
    end as perk_redeemed,

    perks.received_at as perk_received_at

from subscriptions_xf";

        let fix_str = "
select
    subscriptions_xf.metadata_migrated,

    not coalesce(perks.perk is null, false) as perk_redeemed,

    perks.received_at as perk_received_at

from subscriptions_xf";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_7() {
        let fail_str = "
select
    foo,
    case
        when
            bar is null then '123'
        else bar
    end as test
from baz;";

        let fix_str = "
select
    foo,
    coalesce(bar, '123') as test
from baz;";

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_8() {
        let fail_str = r#"
    select
        foo,
        case
            when
                bar is not null then bar
            else '123'
        end as test
    from baz;
    "#;

        let fix_str = r#"
    select
        foo,
        coalesce(bar, '123') as test
    from baz;
    "#;

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str.trim(), fixed.trim());
    }

    #[test]
    fn test_fail_unnecessary_case_9() {
        let fail_str = r#"
select
    foo,
    case
        when
            bar is null then null
        else bar
    end as test
from baz;"#;

        let fix_str = r#"
select
    foo,
    bar as test
from baz;"#;

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_10() {
        let fail_str = r#"
    select
        foo,
        case
            when
                bar is not null then bar
            else null
        end as test
    from baz;
    "#;

        let fix_str = r#"
    select
        foo,
        bar as test
    from baz;
    "#;

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    fn test_fail_unnecessary_case_11() {
        let fail_str = r#"
    select
        foo,
        case
            when
                bar is not null then bar
        end as test
    from baz;
    "#;

        let fix_str = r#"
    select
        foo,
        bar as test
    from baz;
    "#;

        let fixed = fix(fail_str.into(), rules());
        assert_eq!(fix_str, fixed);
    }

    #[test]
    #[ignore = "templates are not implemented"]
    fn test_fail_no_copy_code_out_of_template() {}
}
