use crate::dialects::common::ColumnAliasInfo;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::parser::segments::base::ErasedSegment;

#[derive(Clone)]
pub struct SelectClauseElementSegment(pub ErasedSegment);

impl SelectClauseElementSegment {
    pub fn alias(&self) -> Option<ColumnAliasInfo> {
        let alias_expression_segment = self
            .0
            .recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) },
                true,
                &SyntaxSet::EMPTY,
                true,
            )
            .first()?
            .clone();

        let alias_identifier_segment = alias_expression_segment.segments().iter().find(|it| {
            matches!(
                it.get_type(),
                SyntaxKind::NakedIdentifier | SyntaxKind::Identifier
            )
        })?;

        let aliased_segment = self
            .0
            .segments()
            .iter()
            .find(|&s| !s.is_whitespace() && !s.is_meta() && s != &alias_expression_segment)
            .unwrap();

        let mut column_reference_segments = Vec::new();
        if aliased_segment.is_type(SyntaxKind::ColumnReference) {
            column_reference_segments.push(aliased_segment.clone());
        } else {
            column_reference_segments.extend(aliased_segment.recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::ColumnReference]) },
                true,
                &SyntaxSet::EMPTY,
                true,
            ));
        }

        Some(ColumnAliasInfo {
            alias_identifier_name: alias_identifier_segment.raw().clone(),
            aliased_segment: aliased_segment.clone(),
            column_reference_segments,
        })
    }
}
