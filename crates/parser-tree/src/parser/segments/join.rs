use crate::dialects::common::AliasInfo;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::parser::segments::ErasedSegment;
use crate::parser::segments::from::FromExpressionElementSegment;

pub struct JoinClauseSegment(pub ErasedSegment);

impl JoinClauseSegment {
    pub fn eventual_aliases(&self) -> Vec<(ErasedSegment, AliasInfo)> {
        let mut buff = Vec::new();

        // Check if this is an APPLY clause (CROSS APPLY or OUTER APPLY)
        // APPLY clauses have a different structure where FromExpressionElement is in the sequence
        let is_apply = self
            .0
            .children(const { &SyntaxSet::new(&[SyntaxKind::Keyword]) })
            .any(|kw| kw.raw().to_uppercase() == "APPLY");

        let from_expression = if is_apply {
            // For APPLY clauses, find the FromExpressionElement in the sequence
            self.0
                .children(const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) })
                .next()
                .cloned()
        } else {
            // For regular JOIN clauses, get the nested child
            self.0
                .child(const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) })
        };

        if let Some(from_expr) = from_expression {
            let alias = FromExpressionElementSegment(from_expr.clone()).eventual_alias();
            buff.push((from_expr.clone(), alias));
        }

        for join_clause in self.0.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::JoinClause]) },
            true,
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            true,
        ) {
            if join_clause.id() == self.0.id() {
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
