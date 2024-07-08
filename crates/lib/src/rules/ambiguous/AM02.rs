use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::{WhitespaceSegment, WhitespaceSegmentNewArgs};
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{CloneRule, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::ToErasedSegment;

#[derive(Clone, Debug, Default)]
pub struct RuleAM02;

impl Rule for RuleAM02 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM02.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.union"
    }

    fn description(&self) -> &'static str {
        "Look for UNION keyword not immediately followed by DISTINCT or ALL"
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `UNION DISTINCT` should be preferred over `UNION`, because explicit is better than implicit.


```sql
SELECT a, b FROM table_1
UNION
SELECT a, b FROM table_2
```

**Best practice**

Specify `DISTINCT` or `ALL` after `UNION` (note that `DISTINCT` is the default behavior).

```sql
SELECT a, b FROM table_1
UNION DISTINCT
SELECT a, b FROM table_2
```
"#
    }
    fn eval(&self, rule_cx: RuleContext) -> Vec<LintResult> {
        let supported_dialects = ["ansi", "hive", "mysql", "redshift"];
        if !supported_dialects.contains(&rule_cx.dialect.name) {
            return Vec::new();
        }

        let raw = rule_cx.segment.raw();
        let raw_upper = raw.to_uppercase();

        if rule_cx.segment.raw().contains("union")
            && !(raw_upper.contains("ALL") || raw_upper.contains("DISTINCT"))
        {
            let edits = vec![
                KeywordSegment::new("union".into(), None).to_erased_segment(),
                WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs),
                KeywordSegment::new("distinct".into(), None).to_erased_segment(),
            ];

            let segments = rule_cx.segment.clone();
            let fixes = vec![LintFix::replace(rule_cx.segment.segments()[0].clone(), edits, None)];

            return vec![LintResult::new(Some(segments), fixes, None, None, None)];
        } else if raw_upper.contains("UNION")
            && !(raw_upper.contains("ALL") || raw_upper.contains("DISTINCT"))
        {
            let edits = vec![
                KeywordSegment::new("UNION".into(), None).to_erased_segment(),
                WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs),
                KeywordSegment::new("DISTINCT".into(), None).to_erased_segment(),
            ];

            let segments = rule_cx.segment.clone();
            let fixes = vec![LintFix::replace(rule_cx.segment.segments()[0].clone(), edits, None)];

            return vec![LintResult::new(Some(segments), fixes, None, None, None)];
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["set_operator"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::ambiguous::AM02::RuleAM02;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleAM02::default().erased()]
    }

    #[test]
    fn test_pass_union_all() {
        let sql = "SELECT
          a,
          b
        FROM tbl
          UNION ALL
        SELECT
          c,
          d
        FROM tbl1";

        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_bare_union() {
        let fail_str = "
            SELECT
              a,
              b
            FROM tbl
            UNION
            SELECT
              c,
              d
            FROM tbl1
        ";
        let fix_str = "
            SELECT
              a,
              b
            FROM tbl
            UNION DISTINCT
            SELECT
              c,
              d
            FROM tbl1
        ";

        let actual = fix(fail_str, rules());
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_pass_union_distinct() {
        let sql = "SELECT
          a,
          b
        FROM tbl
          UNION DISTINCT
        SELECT
          c,
          d
        FROM tbl1";

        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_union_distinct_with_comment() {
        let sql = "SELECT
          a,
          b
        FROM tbl

        --selecting a and b

        UNION DISTINCT

        SELECT
          c,
          d
        FROM tbl1";

        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_triple_join_with_one_bad() {
        let fail_str = "
            SELECT
              a,
              b
            FROM tbl
            UNION DISTINCT
            SELECT
              c,
              d
            FROM tbl1
            UNION
            SELECT
              e,
              f
            FROM tbl2
        ";
        let fix_str = "
            SELECT
              a,
              b
            FROM tbl
            UNION DISTINCT
            SELECT
              c,
              d
            FROM tbl1
            UNION DISTINCT
            SELECT
              e,
              f
            FROM tbl2
        ";

        let actual = fix(fail_str, rules());
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_fail_triple_join_with_one_bad_lowercase() {
        let fail_str = "
            select
              a,
              b
            from tbl
            union distinct
            select
              c,
              d
            from tbl1
            union
            select
              e,
              f
            from tbl2
        ";
        let fix_str = "
            select
              a,
              b
            from tbl
            union distinct
            select
              c,
              d
            from tbl1
            union distinct
            select
              e,
              f
            from tbl2
        ";

        let actual = fix(fail_str, rules());
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn test_postgres() {
        let pass_str = "
        select
          a,
          b
        from tbl1
        union
        select
          c,
          d
        from tbl2
      ";

        let violations = lint(pass_str.into(), "postgres".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }
}
