use smol_str::SmolStr;

use crate::dialects::common::AliasInfo;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::parser::segments::base::ErasedSegment;
use crate::parser::segments::join::JoinClauseSegment;

pub struct FromExpressionElementSegment(pub ErasedSegment);
pub struct FromClauseSegment(pub ErasedSegment);

impl FromClauseSegment {
    pub fn eventual_aliases(&self) -> Vec<(ErasedSegment, AliasInfo)> {
        let mut buff = Vec::new();
        let mut direct_table_children = Vec::new();
        let mut join_clauses = Vec::new();

        for from_expression in self
            .0
            .children(const { &SyntaxSet::new(&[SyntaxKind::FromExpression]) })
        {
            direct_table_children.extend(
                from_expression
                    .children(const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) }),
            );
            join_clauses.extend(
                from_expression.children(const { &SyntaxSet::new(&[SyntaxKind::JoinClause]) }),
            );
        }

        for &clause in &direct_table_children {
            let tmp;

            let alias = FromExpressionElementSegment(clause.clone()).eventual_alias();

            let table_expr = if direct_table_children.contains(&clause) {
                clause
            } else {
                tmp = clause
                    .child(const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) })
                    .unwrap();
                &tmp
            };

            buff.push((table_expr.clone(), alias));
        }

        for clause in join_clauses {
            let aliases = JoinClauseSegment(clause.clone()).eventual_aliases();

            if !aliases.is_empty() {
                buff.extend(aliases);
            }
        }

        buff
    }
}

impl FromExpressionElementSegment {
    pub fn eventual_alias(&self) -> AliasInfo {
        let mut tbl_expression = self
            .0
            .child(const { &SyntaxSet::new(&[SyntaxKind::TableExpression]) })
            .or_else(|| {
                self.0
                    .child(const { &SyntaxSet::new(&[SyntaxKind::Bracketed]) })
                    .and_then(|bracketed| {
                        bracketed.child(const { &SyntaxSet::new(&[SyntaxKind::TableExpression]) })
                    })
            });

        if let Some(tbl_expression_inner) = &tbl_expression {
            if tbl_expression_inner
                    .child(const { &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::TableReference]) })
                    .is_none()
                {
                    if let Some(bracketed) = tbl_expression_inner.child(const { &SyntaxSet::new(&[SyntaxKind::Bracketed]) }) {
                        tbl_expression = bracketed.child(const { &SyntaxSet::new(&[SyntaxKind::TableExpression]) });
                    }
                }
        }

        let reference = tbl_expression.and_then(|tbl_expression| {
            tbl_expression.child(const { &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::TableReference]) })
        });

        let reference = reference.as_ref().map(|reference| reference.reference());

        let alias_expression = self
            .0
            .child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) });
        if let Some(alias_expression) = alias_expression {
            let segment = alias_expression.child(
                const { &SyntaxSet::new(&[SyntaxKind::Identifier, SyntaxKind::NakedIdentifier]) },
            );
            if let Some(segment) = segment {
                return AliasInfo {
                    ref_str: segment.raw().clone(),
                    segment: segment.into(),
                    aliased: true,
                    from_expression_element: self.0.clone(),
                    alias_expression: alias_expression.into(),
                    object_reference: reference.map(|it| it.clone().0),
                };
            }
        }

        if let Some(reference) = &reference {
            let references = reference.iter_raw_references();

            if !references.is_empty() {
                let penultimate_ref = references.last().unwrap();
                return AliasInfo {
                    ref_str: penultimate_ref.part.clone().into(),
                    segment: penultimate_ref.segments[0].clone().into(),
                    aliased: false,
                    from_expression_element: self.0.clone(),
                    alias_expression: None,
                    object_reference: reference.clone().0.into(),
                };
            }
        }

        AliasInfo {
            ref_str: SmolStr::new_static(""),
            segment: None,
            aliased: false,
            from_expression_element: self.0.clone(),
            alias_expression: None,
            object_reference: reference.map(|it| it.clone().0),
        }
    }
}
