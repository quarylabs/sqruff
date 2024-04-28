use ahash::AHashMap;
use itertools::Itertools;

use crate::core::config::Value;
use crate::core::rules::base::{ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::functional::segments::Segments;

#[derive(Debug, Default, Clone)]
pub struct RuleLT13 {}

impl Rule for RuleLT13 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        unimplemented!()
    }

    fn name(&self) -> &'static str {
        "layout.start_of_file"
    }

    fn description(&self) -> &'static str {
        "Files must not begin with newlines or whitespace."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut raw_segments = Vec::new();

        for seg in context.segment.recursive_crawl_all(false) {
            if !seg.segments().is_empty() {
                continue;
            }

            if matches!(seg.get_type(), "newline" | "whitespace" | "indent" | "dedent") {
                raw_segments.push(seg);
                continue;
            }

            let raw_stack =
                Segments::from_vec(raw_segments.clone(), context.templated_file.clone());
            // Non-whitespace segment.
            if !raw_stack.all(Some(|seg| seg.is_meta())) {
                return vec![LintResult::new(
                    context.segment.into(),
                    raw_stack.into_iter().map(LintFix::delete).collect_vec(),
                    None,
                    None,
                    None,
                )];
            } else {
                break;
            }
        }

        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

#[cfg(test)]
mod tests {
    use super::RuleLT13;
    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT13::default().erased()]
    }

    #[test]
    fn test_pass_leading_whitespace_statement() {
        let lints =
            lint("SELECT foo FROM bar\n".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_pass_leading_whitespace_comment() {
        let lints = lint(
            "/*I am a comment*/\nSELECT foo FROM bar\n".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_pass_leading_whitespace_inline_comment() {
        let lints = lint(
            "--I am a comment\nSELECT foo FROM bar\n".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    #[ignore = "dialect: bigquery"]
    fn test_pass_leading_whitespace_inline_comment_hash() {}

    #[test]
    fn test_pass_leading_whitespace_jinja_comment() {
        let lints = lint(
            "{# I am a comment #}\nSELECT foo FROM bar\n".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_pass_leading_whitespace_jinja_if() {
        let lints = lint(
            "{% if True %}\nSELECT foo\nFROM bar;\n{% endif %}\n".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_pass_leading_whitespace_jinja_for() {
        let lints = lint(
            "{% for item in range(10) %}\nSELECT foo_{{ item }}\nFROM bar;\n{% endfor %}\n".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_fail_leading_whitespace_statement() {
        let fixed = fix("\n  SELECT foo FROM bar\n".into(), rules());
        assert_eq!(fixed, "SELECT foo FROM bar\n");
    }

    #[test]
    fn test_fail_leading_whitespace_comment() {
        let fixed = fix("\n  /*I am a comment*/\nSELECT foo FROM bar\n".into(), rules());
        assert_eq!(fixed, "/*I am a comment*/\nSELECT foo FROM bar\n");
    }

    #[test]
    fn test_fail_leading_whitespace_inline_comment() {
        let fixed = fix("\n  --I am a comment\nSELECT foo FROM bar\n".into(), rules());
        assert_eq!(fixed, "--I am a comment\nSELECT foo FROM bar\n");
    }

    #[test]
    fn test_fail_leading_whitespace_jinja_comment() {
        let fixed = fix("\n  {# I am a comment #}\nSELECT foo FROM bar\n".into(), rules());
        assert_eq!(fixed, "{# I am a comment #}\nSELECT foo FROM bar\n");
    }

    #[test]
    fn test_fail_leading_whitespace_jinja_if() {
        let fixed = fix("\n  {% if True %}\nSELECT foo\nFROM bar;\n{% endif %}\n".into(), rules());
        assert_eq!(fixed, "{% if True %}\nSELECT foo\nFROM bar;\n{% endif %}\n");
    }

    #[test]
    fn test_fail_leading_whitespace_jinja_for() {
        let fixed = fix(
            "\n  {% for item in range(10) %}\nSELECT foo_{{ item }}\nFROM bar;\n{% endfor %}\n"
                .into(),
            rules(),
        );
        assert_eq!(
            fixed,
            "{% for item in range(10) %}\nSELECT foo_{{ item }}\nFROM bar;\n{% endfor %}\n"
        );
    }
}
