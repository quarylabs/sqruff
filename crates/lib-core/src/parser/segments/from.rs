use smol_str::SmolStr;

use crate::dialects::common::AliasInfo;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::parser::segments::ErasedSegment;
use crate::parser::segments::join::JoinClauseSegment;

pub struct FromExpressionElementSegment(pub ErasedSegment);
pub struct FromClauseSegment(pub ErasedSegment);

impl FromClauseSegment {
    pub fn eventual_aliases(&self) -> Vec<(ErasedSegment, AliasInfo)> {
        get_from_expression_aliases(&self.0)
    }
}

/// Resolve aliases in direct FROM expressions, whether they belong to a
/// FROM clause or to a dialect's UPDATE target.
pub fn get_from_expression_aliases(segment: &ErasedSegment) -> Vec<(ErasedSegment, AliasInfo)> {
    let mut buff = Vec::new();
    let mut direct_table_children = Vec::new();
    let mut join_clauses = Vec::new();

    for from_expression in
        segment.children(const { &SyntaxSet::new(&[SyntaxKind::FromExpression]) })
    {
        direct_table_children.extend(
            from_expression
                .children(const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) }),
        );
        join_clauses
            .extend(from_expression.children(const { &SyntaxSet::new(&[SyntaxKind::JoinClause]) }));
    }

    for &clause in &direct_table_children {
        let aliases = FromExpressionElementSegment(clause.clone()).eventual_aliases();
        buff.extend(aliases.into_iter().map(|alias| (clause.clone(), alias)));
    }

    for clause in join_clauses {
        let aliases = JoinClauseSegment(clause.clone()).eventual_aliases();

        if !aliases.is_empty() {
            buff.extend(aliases);
        }
    }

    buff
}

impl FromExpressionElementSegment {
    pub fn eventual_alias(&self) -> AliasInfo {
        self.eventual_aliases()
            .into_iter()
            .next()
            .unwrap_or_else(|| AliasInfo {
                ref_str: SmolStr::new_static(""),
                segment: None,
                aliased: false,
                from_expression_element: self.0.clone(),
                alias_expression: None,
                object_reference: None,
            })
    }

    /// Collect every alias, including a value table function's offset alias.
    pub fn eventual_aliases(&self) -> Vec<AliasInfo> {
        let mut tbl_expression = self
            .0
            .child(const { &SyntaxSet::single(SyntaxKind::TableExpression) });
        if tbl_expression.is_none()
            && let Some(bracketed) = self
                .0
                .child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
        {
            tbl_expression =
                bracketed.child(const { &SyntaxSet::single(SyntaxKind::TableExpression) });
            if tbl_expression.is_none() {
                // The alias belongs to the element inside any redundant
                // bracket layers, rather than to the bracketed join target.
                let mut bracketed = Some(bracketed);
                while let Some(layer) = bracketed {
                    if let Some(inner) =
                        layer.child(const { &SyntaxSet::single(SyntaxKind::FromExpressionElement) })
                    {
                        return FromExpressionElementSegment(inner).eventual_aliases();
                    }
                    bracketed = layer.child(const { &SyntaxSet::single(SyntaxKind::Bracketed) });
                }
            }
        }

        // TSQL nests a table expression under each bracket pair, so unwrap
        // until the first table reference is reached.
        while let Some(inner) = &tbl_expression {
            if inner
                .child(const {
                    &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::TableReference])
                })
                .is_some()
            {
                break;
            }
            let Some(bracketed) = inner.child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
            else {
                break;
            };
            tbl_expression =
                bracketed.child(const { &SyntaxSet::single(SyntaxKind::TableExpression) });
        }

        let reference = tbl_expression.and_then(|tbl_expression| {
            tbl_expression.child(const { &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::TableReference]) })
        });

        let reference = reference.as_ref().map(|reference| reference.reference());

        let mut aliases = Vec::new();
        let mut has_alias = false;
        for alias_expression in self.0.children(
            const { &SyntaxSet::new(&[SyntaxKind::AliasExpression, SyntaxKind::Bracketed]) },
        ) {
            let alias_expression = if alias_expression.is_type(SyntaxKind::Bracketed) {
                let Some(alias_expression) = alias_expression
                    .child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) })
                else {
                    continue;
                };
                alias_expression
            } else {
                alias_expression.clone()
            };
            has_alias = true;
            let segment = alias_expression.child(
                const {
                    &SyntaxSet::new(&[
                        SyntaxKind::Identifier,
                        SyntaxKind::NakedIdentifier,
                        SyntaxKind::QuotedIdentifier,
                        SyntaxKind::Literal,
                    ])
                },
            );
            if let Some(segment) = segment {
                aliases.push(AliasInfo {
                    ref_str: segment.raw_normalized(),
                    segment: segment.into(),
                    aliased: true,
                    from_expression_element: self.0.clone(),
                    alias_expression: alias_expression.into(),
                    object_reference: reference.as_ref().map(|it| it.clone().0),
                });
            }
        }

        if has_alias {
            return aliases;
        }

        if let Some(reference) = &reference {
            let references = reference.iter_raw_references();

            if !references.is_empty() {
                let penultimate_ref = references.last().unwrap();
                return vec![AliasInfo {
                    ref_str: penultimate_ref.part.clone().into(),
                    segment: penultimate_ref.segments[0].clone().into(),
                    aliased: false,
                    from_expression_element: self.0.clone(),
                    alias_expression: None,
                    object_reference: reference.clone().0.into(),
                }];
            }
        }

        vec![AliasInfo {
            ref_str: SmolStr::new_static(""),
            segment: None,
            aliased: false,
            from_expression_element: self.0.clone(),
            alias_expression: None,
            object_reference: reference.map(|it| it.clone().0),
        }]
    }
}
