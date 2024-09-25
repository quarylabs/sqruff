use crate::dialects::common::AliasInfo;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::parser::segments::base::ErasedSegment;
use crate::parser::segments::from::FromExpressionElementSegment;

pub struct JoinClauseSegment(pub ErasedSegment);

impl JoinClauseSegment {
    pub fn eventual_aliases(&self) -> Vec<(ErasedSegment, AliasInfo)> {
        let mut buff = Vec::new();

        let from_expression = self
            .0
            .child(const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) })
            .unwrap();
        let alias = FromExpressionElementSegment(from_expression.clone()).eventual_alias();

        buff.push((from_expression.clone(), alias));

        for join_clause in self.0.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::JoinClause]) },
            true,
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            true,
        ) {
            if join_clause.id() == join_clause.id() {
                continue;
            }

            let aliases = JoinClauseSegment(join_clause).eventual_aliases();

            if !aliases.is_empty() {
                buff.extend(aliases);
            }
        }

        buff
    }
}
