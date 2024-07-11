use ahash::AHashMap;
use itertools::Itertools;
use smol_str::{SmolStr, ToSmolStr};

use crate::core::config::Value;
use crate::core::parser::segments::base::{
    CodeSegmentNewArgs, ErasedSegment, IdentifierSegment, SymbolSegment, SymbolSegmentNewArgs,
    WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{CloneRule, ErasedRule, LintFix, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::ansi::Node;
use crate::dialects::SyntaxKind;
use crate::helpers::ToErasedSegment;
use crate::utils::analysis::select::get_select_statement_info;
use crate::utils::functional::context::FunctionalContext;
use crate::utils::functional::segments::Segments;

#[derive(Clone, Debug, Default)]
pub struct RuleST07;

impl Rule for RuleST07 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST07.erased())
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "structure.using"
    }

    fn description(&self) -> &'static str {
        "Prefer specifying join keys instead of using ``USING``."
    }

    fn long_description(&self) -> &'static str {
        r"
**Anti-pattern**

```sql
SELECT
    table_a.field_1,
    table_b.field_2
FROM
    table_a
INNER JOIN table_b USING (id)
```

**Best practice**

Specify the keys directly

```sql
SELECT
    table_a.field_1,
    table_b.field_2
FROM
    table_a
INNER JOIN table_b
    ON table_a.id = table_b.id
```"
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if context.dialect.name == "clickhouse" {
            return Vec::new();
        }

        let functional_context = FunctionalContext::new(context.clone());
        let segment = functional_context.segment();
        let parent_stack = functional_context.parent_stack();

        let usings = segment.children(Some(|it: &ErasedSegment| it.is_keyword("using")));
        let using_anchor = usings.first();

        let Some(using_anchor) = using_anchor else {
            return Vec::new();
        };

        let unfixable_result = LintResult::new(
            using_anchor.clone().into(),
            Vec::new(),
            None,
            Some("Found USING statement. Expected only ON statements.".into()),
            None,
        );

        let tables_in_join = parent_stack
            .last()
            .unwrap()
            .segments()
            .iter()
            .filter(|it| matches!(it.get_type(), "join_clause" | "from_expression_element"))
            .cloned()
            .collect_vec();

        if segment.get(0, None) != tables_in_join.get(1).cloned() {
            return vec![unfixable_result];
        }

        let stmts =
            parent_stack.find_last(Some(|it: &ErasedSegment| it.is_type("select_statement")));
        let parent_select = stmts.first();

        let Some(parent_select) = parent_select else { return vec![unfixable_result] };

        let select_info = get_select_statement_info(parent_select, context.dialect.into(), true);
        let mut table_aliases =
            select_info.map_or(Vec::new(), |select_info| select_info.table_aliases);
        table_aliases.retain(|it| !it.ref_str.is_empty());

        if table_aliases.len() < 2 {
            return vec![unfixable_result];
        }

        let (to_delete, insert_after_anchor) = extract_deletion_sequence_and_anchor(&segment);

        let [table_a, table_b, ..] = &table_aliases[..] else { unreachable!() };

        let mut edit_segments = vec![
            KeywordSegment::new("ON".into(), None).to_erased_segment(),
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs {}),
        ];

        edit_segments.append(&mut generate_join_conditions(
            &table_a.ref_str,
            &table_b.ref_str,
            extract_cols_from_using(segment, using_anchor),
        ));

        let mut fixes = Vec::with_capacity(1 + to_delete.len());

        fixes.push(LintFix::create_before(insert_after_anchor, edit_segments));
        fixes.extend(to_delete.into_iter().map(LintFix::delete));

        vec![LintResult::new(using_anchor.clone().into(), fixes, None, None, None)]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["join_clause"].into()).into()
    }
}

fn extract_cols_from_using(join_clause: Segments, using_segs: &ErasedSegment) -> Vec<SmolStr> {
    join_clause
        .children(None)
        .select(Some(|it: &ErasedSegment| it.is_type("bracketed")), None, Some(using_segs), None)
        .find_first::<fn(&ErasedSegment) -> bool>(None)
        .children(Some(|it: &ErasedSegment| {
            it.is_type("identifier") || it.is_type("naked_identifier")
        }))
        .into_iter()
        .map(|it| it.raw().to_smolstr())
        .collect()
}

fn generate_join_conditions(
    table_a_ref: &str,
    table_b_ref: &str,
    columns: Vec<SmolStr>,
) -> Vec<ErasedSegment> {
    let mut edit_segments = Vec::new();

    for col in columns {
        edit_segments.extend_from_slice(&[
            create_col_reference(table_a_ref, &col),
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs {}),
            SymbolSegment::create("=", None, SymbolSegmentNewArgs { r#type: "symbol" }),
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs {}),
            create_col_reference(table_b_ref, &col),
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs {}),
            KeywordSegment::new("AND".into(), None).to_erased_segment(),
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs {}),
        ]);
    }

    edit_segments
        .get(..edit_segments.len().saturating_sub(3))
        .map_or(Vec::new(), ToOwned::to_owned)
        .to_vec()
}

fn extract_deletion_sequence_and_anchor(
    join_clause: &Segments,
) -> (Vec<ErasedSegment>, ErasedSegment) {
    let mut insert_anchor = None;
    let mut to_delete = Vec::new();

    for seg in join_clause.children(None) {
        if seg.raw().eq_ignore_ascii_case("USING") {
            to_delete.push(seg.clone());
            continue;
        }

        if to_delete.is_empty() {
            continue;
        }

        if to_delete.last().unwrap().is_type("bracketed") {
            insert_anchor = Some(seg);
            break;
        }

        to_delete.push(seg);
    }

    (to_delete, insert_anchor.unwrap())
}

fn create_col_reference(table_ref: &str, column_name: &str) -> ErasedSegment {
    Node::new(
        SyntaxKind::ColumnReference,
        vec![
            IdentifierSegment::create(
                table_ref,
                None,
                CodeSegmentNewArgs {
                    code_type: "naked_identifier",
                    ..CodeSegmentNewArgs::default()
                },
            ),
            SymbolSegment::create(".", None, SymbolSegmentNewArgs { r#type: "symbol" }),
            IdentifierSegment::create(
                column_name,
                None,
                CodeSegmentNewArgs {
                    code_type: "naked_identifier",
                    ..CodeSegmentNewArgs::default()
                },
            ),
        ],
        false,
    )
    .to_erased_segment()
}
