use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::{ErasedSegment, NewlineSegment};
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::functional::context::FunctionalContext;
use crate::utils::functional::segments::Segments;

fn get_trailing_newlines(segment: &ErasedSegment) -> Vec<ErasedSegment> {
    let mut result = Vec::new();

    for seg in segment.recursive_crawl_all(true) {
        if seg.is_type("newline") {
            result.push(seg.clone_box());
        } else if !seg.is_whitespace() && !seg.is_type("dedent") && !seg.is_type("end_of_file") {
            break;
        }
    }

    result
}

fn get_last_segment(mut segment: Segments) -> (Vec<ErasedSegment>, Segments) {
    let mut parent_stack = Vec::new();

    loop {
        let children = segment.children(None);

        if !children.is_empty() {
            parent_stack.push(segment.first().unwrap().clone_box());
            segment = children.find_last(Some(|s| !s.is_type("end_of_file")));
        } else {
            return (parent_stack, segment);
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RuleLT12 {}

impl Rule for RuleLT12 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleLT12::default().erased()
    }

    fn name(&self) -> &'static str {
        "layout.end_of_file"
    }

    fn description(&self) -> &'static str {
        "Files must end with a single trailing newline."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let (parent_stack, segment) =
            get_last_segment(FunctionalContext::new(context.clone()).segment());

        if segment.is_empty() {
            return Vec::new();
        }

        let trailing_newlines = Segments::from_vec(get_trailing_newlines(&context.segment), None);
        if trailing_newlines.is_empty() {
            let fix_anchor_segment = if parent_stack.len() == 1 {
                segment.first().unwrap().clone_box()
            } else {
                parent_stack[1].clone()
            };

            vec![LintResult::new(
                segment.first().unwrap().clone_box().into(),
                vec![LintFix::create_after(
                    fix_anchor_segment,
                    vec![NewlineSegment::create("\n", &<_>::default(), <_>::default())],
                    None,
                )],
                None,
                None,
                None,
            )]
        } else if trailing_newlines.len() > 1 {
            vec![LintResult::new(
                segment.first().unwrap().clone_box().into(),
                trailing_newlines.into_iter().skip(1).map(|d| LintFix::delete(d.clone())).collect(),
                None,
                None,
                None,
            )]
        } else {
            vec![]
        }
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

#[cfg(test)]
mod tests {
    use super::RuleLT12;
    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT12::default().erased()]
    }

    #[test]
    fn test_pass_single_final_newline() {
        let lints =
            lint("SELECT foo FROM bar\n".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_fail_no_final_newline() {
        let fixed = fix("SELECT foo FROM bar".into(), rules());
        assert_eq!(fixed, "SELECT foo FROM bar\n");
    }

    #[test]
    fn test_fail_multiple_final_newlines() {
        let fixed = fix("SELECT foo FROM bar\n\n".into(), rules());
        assert_eq!(fixed, "SELECT foo FROM bar\n");
    }

    #[test]
    fn test_pass_templated_plus_raw_newlines() {
        let lints = lint("{{ '\n\n' }}\n".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_fail_templated_plus_raw_newlines() {
        let fixed = fix("{{ '\n\n' }}".into(), rules());
        assert_eq!(fixed, "{{ '\n\n' }}\n");
    }

    #[test]
    fn test_fail_templated_plus_raw_newlines_extra_newline() {
        let fixed = fix("{{ '\n\n' }}\n\n".into(), rules());
        assert_eq!(fixed, "{{ '\n\n' }}\n");
    }

    #[test]
    #[ignore = "parser needs further development"]
    fn test_pass_templated_macro_newlines() {
        let macro_code = "{% macro get_keyed_nulls(columns) %}\n  {{ columns }}\n{% endmacro \
                          %}\nSELECT {{ get_keyed_nulls(\"other_id\") }}";
        let lints = lint(macro_code.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_fail_templated_no_newline() {
        let fixed = fix("{% if true %}\nSELECT 1 + 1\n{%- endif %}".into(), rules());
        assert_eq!(fixed, "{% if true %}\nSELECT 1 + 1\n{%- endif %}\n");
    }
}
