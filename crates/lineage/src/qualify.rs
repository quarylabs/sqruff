use std::collections::HashMap;

use sqruff_lib::core::dialects::init::DialectKind;
use sqruff_lib::core::linter::linter::compute_anchor_edit_info;
use sqruff_lib::core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib::core::rules::base::LintFix;
use sqruff_lib::dialects::SyntaxKind;

use crate::schema::Schema;
use crate::scope::Scope;

pub(crate) fn qualify(segment: ErasedSegment, schema: Schema) -> ErasedSegment {
    let segment = qualify_tables(segment);
    let segment = qualify_columns(segment, schema);

    segment
}

fn qualify_tables(segment: ErasedSegment) -> ErasedSegment {
    let mut fixes = Vec::new();

    for scope in crate::scope::traverse(segment.clone()) {
        for (name, source) in scope.sources() {
            if source.is_type(SyntaxKind::TableReference) {
                let fix = LintFix::replace(
                    source.clone(),
                    vec![
                        SegmentBuilder::node(
                            0,
                            SyntaxKind::AliasExpression,
                            DialectKind::Ansi,
                            vec![
                                source.clone(),
                                SegmentBuilder::whitespace(0, " "),
                                SegmentBuilder::keyword(0, "AS"),
                                SegmentBuilder::whitespace(0, " "),
                                SegmentBuilder::token(0, name, SyntaxKind::NakedIdentifier)
                                    .finish(),
                            ],
                        )
                        .finish(),
                    ],
                    None,
                );
                fixes.push(fix);
            }
        }
    }

    let mut edit_info = compute_anchor_edit_info(fixes);
    let (new_root, ..) = segment.apply_fixes(&mut edit_info);

    new_root
}

fn qualify_columns(segment: ErasedSegment, schema: Schema) -> ErasedSegment {
    for mut scope in crate::scope::traverse(segment) {
        scope.collect();

        let resolver = Resolver::new(&scope, &schema);
        // let using_column_tables = HashMap::new();

        qualify_columns_inner(&scope, &resolver);
    }

    todo!()
}

fn qualify_columns_inner(scope: &Scope, resolver: &Resolver) {
    for column in scope.columns() {}
}

struct Resolver<'a> {
    scope: &'a Scope,
    schema: &'a Schema,
}

impl<'a> Resolver<'a> {
    fn new(scope: &'a Scope, schema: &'a Schema) -> Self {
        Self { scope, schema }
    }

    fn table(&self, column_name: &str) {}
}
