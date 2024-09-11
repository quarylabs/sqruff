use std::collections::HashMap;

use sqruff_lib::core::parser::segments::base::ErasedSegment;

use crate::ir::{lower_inner, specific_statement_segment, Expr, ExprKind, Tables};

pub(crate) fn expand(tables: &mut Tables, sources: &HashMap<String, ErasedSegment>, expr: Expr) {
    let exprs: Vec<_> = tables.walk::<fn(&Tables, _) -> _>(expr, None).collect();

    for node in exprs {
        if let ExprKind::TableReference(this, None) = &tables.exprs[node].kind {
            let name = this.clone();
            let Some(new_node) = sources.get(this.as_str()).cloned() else {
                continue;
            };

            let mut stmts = specific_statement_segment(new_node);
            let new_node = stmts.pop().unwrap();

            let new_node = lower_inner(tables, new_node, None);

            expand(tables, sources, new_node);

            let new_node = ExprKind::Subquery(new_node, name.clone().into());
            let old_node = &mut tables.exprs[node];

            old_node.kind = new_node;
            old_node.comments = vec![format!("source: {name}")]
        }
    }
}
