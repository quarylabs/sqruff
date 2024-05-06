use itertools::Itertools;

use crate::core::config::Value;
use crate::core::parser::segments::base::{
    ErasedSegment, Segment, SymbolSegment, SymbolSegmentNewArgs, WhitespaceSegment,
    WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::ToErasedSegment;
use crate::utils::functional::segments::Segments;
use crate::utils::reflow::sequence::{Filter, ReflowSequence};

#[derive(Debug)]
enum CorrectionListItem {
    WhitespaceSegment,
    KeywordSegment(String),
}

type CorrectionList = Vec<CorrectionListItem>;

#[derive(Default, Clone, Debug)]
pub struct RuleCV05 {}

fn create_base_is_null_sequence(is_upper: bool, operator_raw: String) -> CorrectionList {
    let is_seg = CorrectionListItem::KeywordSegment(if is_upper { "IS" } else { "is" }.to_string());
    let not_seg =
        CorrectionListItem::KeywordSegment(if is_upper { "NOT" } else { "not" }.to_string());

    if operator_raw == "=" {
        vec![is_seg]
    } else {
        vec![is_seg, CorrectionListItem::WhitespaceSegment, not_seg]
    }
}

impl Rule for RuleCV05 {
    fn name(&self) -> &'static str {
        "convention.is_null"
    }

    fn description(&self) -> &'static str {
        "Relational operators should not be used to check for NULL values."
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["comparison_operator"].into()).into()
    }

    fn load_from_config(&self, _config: &ahash::AHashMap<String, Value>) -> ErasedRule {
        unimplemented!()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if context.parent_stack.len() >= 2 {
            for type_str in &["set_clause_list", "execute_script_statement", "options_segment"] {
                if context.parent_stack[context.parent_stack.len() - 2].is_type(type_str) {
                    return Vec::new();
                }
            }
        }

        if !context.parent_stack.is_empty() {
            for type_str in &["set_clause_list", "execute_script_statement", "assignment_operator"]
            {
                if context.parent_stack[context.parent_stack.len() - 1].is_type(type_str) {
                    return Vec::new();
                }
            }
        }

        if !context.parent_stack.is_empty()
            && context.parent_stack[context.parent_stack.len() - 1]
                .is_type("exclusion_constraint_element")
        {
            return Vec::new();
        }

        if let Some(raw_consist) = context.segment.get_raw() {
            if !["=", "!=", "<>"].contains(&raw_consist.as_str()) {
                return Vec::new();
            }
        }

        let segment = context.parent_stack.last().unwrap().segments().to_vec();

        let siblings = Segments::from_vec(segment, None);
        let after_op_list = siblings.select(None, None, Some(&context.segment), None);

        let next_code = after_op_list.find_first(Some(|sp: &ErasedSegment| sp.is_code()));

        if !next_code.all(Some(|it| it.is_type("null_literal"))) {
            return Vec::new();
        }

        let sub_seg = next_code.get(0, None);
        let edit = create_base_is_null_sequence(
            sub_seg.as_ref().unwrap().get_raw().unwrap().chars().next().unwrap() == 'N',
            context.segment.get_raw().unwrap(),
        );

        let mut seg = Vec::with_capacity(edit.len());

        for item in edit {
            match item {
                CorrectionListItem::KeywordSegment(keyword) => {
                    seg.push(KeywordSegment::new(keyword, None).to_erased_segment());
                }
                CorrectionListItem::WhitespaceSegment => {
                    seg.push(WhitespaceSegment::create(
                        " ",
                        &<_>::default(),
                        WhitespaceSegmentNewArgs,
                    ));
                }
            };
        }

        let fixes = ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack[0].clone(),
            "both",
            context.config.unwrap(),
        )
        .replace(context.segment.clone(), &seg)
        .respace(false, Filter::All)
        .fixes();

        vec![LintResult::new(Some(context.segment.clone()), fixes, None, None, None)]
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::Erased;
    use crate::rules::convention::CV05::RuleCV05;

    #[test]
    fn test_is_null() {
        let pass_str = r#"SELECT a 
                                FROM foo
                                WHERE a IS NULL"#;

        let violations = lint(
            pass_str.to_owned(),
            "ansi".into(),
            vec![RuleCV05::default().erased()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_is_not_null() {
        let pass_str = r#"SELECT a 
        FROM foo
        WHERE a IS NOT NULL"#;

        let violations = lint(
            pass_str.to_owned(),
            "ansi".into(),
            vec![RuleCV05::default().erased()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_not_equals_null_upper() {
        let fail_str = "SELECT a FROM foo WHERE a <> NULL";
        let fix_str = "SELECT a FROM foo WHERE a IS NOT NULL";

        let result = fix(fail_str.into(), vec![RuleCV05::default().erased()]);
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_not_equals_null_multi_nulls() {
        let fail_str = r#"SELECT a 
                                FROM foo
                                WHERE a <> NULL AND b != NULL AND c = 'foo'"#;

        let fix_str = r#"SELECT a 
                                FROM foo
                                WHERE a IS NOT NULL AND b IS NOT NULL AND c = 'foo'"#;

        let result = fix(fail_str.into(), vec![RuleCV05::default().erased()]);
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_not_equals_null_lower() {
        let fail_str = "SELECT a FROM foo WHERE a <> null";
        let fix_str = "SELECT a FROM foo WHERE a is not null";

        let actual = fix(fail_str.into(), vec![RuleCV05::default().erased()]);
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_equals_null_spaces() {
        let fail_str = r#"SELECT a 
                                FROM foo
                                WHERE a = NULL"#;

        let fix_str = r#"SELECT a 
                                FROM foo
                                WHERE a IS NULL"#;

        let result = fix(fail_str.into(), vec![RuleCV05::default().erased()]);
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_equals_null_no_spaces() {
        let fail_str = "SELECT a FROM foo WHERE a=NULL";
        let fix_str = "SELECT a FROM foo WHERE a IS NULL";

        let result = fix(fail_str.into(), vec![RuleCV05::default().erased()]);
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_complex_case_1() {
        let fail_str = "SELECT a FROM foo WHERE a = b or (c > d or e = NULL)";
        let fix_str = "SELECT a FROM foo WHERE a = b or (c > d or e IS NULL)";

        let actual = fix(fail_str.into(), vec![RuleCV05::default().erased()]);
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_set_clause() {
        let pass_str = r#"UPDATE table1 SET col = NULL 
                                WHERE col = """#;

        let violations = lint(
            pass_str.to_owned(),
            "ansi".into(),
            vec![RuleCV05::default().erased()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    #[ignore]
    fn test_bigquery_set_options() {
        let pass_str = r#"ALTER TABLE table
                                SET OPTIONS (expiration_timestamp = NULL);"#;

        let violations = lint(
            pass_str.to_owned(),
            "bigquery".into(),
            vec![RuleCV05::default().erased()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    #[ignore]
    fn test_tsql_exec_clause() {
        let pass_str = r#"exec something
                                @param1 = 'blah',
                                @param2 = 'blah',
                                @param3 = null,
                                @param4 = 'blah'"#;

        let violations = lint(
            pass_str.to_owned(),
            "tsql".into(),
            vec![RuleCV05::default().erased()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    #[ignore]
    fn test_tsql_alternate_alias_syntax() {
        let pass_str = r#"select name = null from t"#;

        let violations = lint(
            pass_str.to_owned(),
            "tsql".into(),
            vec![RuleCV05::default().erased()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    #[ignore]
    fn test_exclude_constraint() {
        let pass_str = r#"alter table abc add constraint xyz exclude (field WITH =);"#;

        let violations = lint(
            pass_str.to_owned(),
            "postgres".into(),
            vec![RuleCV05::default().erased()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }
}
