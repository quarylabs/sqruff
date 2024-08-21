use std::collections::HashMap;

use sqruff_lib::core::parser::segments::base::ErasedSegment;
use sqruff_lib::dialects::{SyntaxKind, SyntaxSet};
use sqruff_lib::helpers::Config;

pub(crate) struct Scope {
    master: ErasedSegment,
    parent: Option<ErasedSegment>,
    sources: HashMap<String, ErasedSegment>,
    raw_columns: Vec<ErasedSegment>,
    collected: bool,
}

impl Scope {
    pub(crate) fn new(master: ErasedSegment) -> Self {
        Self {
            master,
            parent: None,
            sources: HashMap::new(),
            raw_columns: Vec::new(),
            collected: false,
        }
    }

    pub(crate) fn columns(&self) -> &[ErasedSegment] {
        &self.raw_columns
    }

    pub(crate) fn segment(&self) -> &ErasedSegment {
        &self.master
    }

    pub(crate) fn sources(&self) -> &HashMap<String, ErasedSegment> {
        &self.sources
    }

    pub(crate) fn branch(&self, segment: ErasedSegment) -> Self {
        Self::new(segment).config(|this| this.parent = self.master.clone().into())
    }

    pub(crate) fn collect(&mut self) {
        if self.collected {
            return;
        }

        for segment in self.master.recursive_crawl_all(false) {
            if segment.is(&self.master) {
                continue;
            }

            match segment.get_type() {
                SyntaxKind::ColumnReference => {
                    self.raw_columns.push(segment);
                }
                _ => {}
            }
        }

        self.collected = true;
    }
}

pub(crate) fn build_scope(segment: ErasedSegment) -> Scope {
    traverse(segment).pop().unwrap()
}

pub(crate) fn traverse(segment: ErasedSegment) -> Vec<Scope> {
    traverse_scope(Scope::new(segment))
}

fn traverse_scope(mut scope: Scope) -> Vec<Scope> {
    let mut acc = Vec::new();

    match scope.master.get_type() {
        SyntaxKind::SelectStatement => traverse_select(&mut scope, &mut acc),
        _ => return acc,
    };

    acc.push(scope);

    acc
}

fn traverse_select(scope: &mut Scope, acc: &mut Vec<Scope>) {
    traverse_tables(scope, acc);
}

fn traverse_tables(scope: &mut Scope, acc: &mut Vec<Scope>) {
    let mut sources = HashMap::new();
    let mut expressions = Vec::new();

    let from_expression_element_list = scope.master.recursive_crawl(
        const { &SyntaxSet::single(SyntaxKind::TableExpression) },
        true,
        &SyntaxSet::EMPTY,
        false,
    );

    let from_expression_element = from_expression_element_list.into_iter().next();
    if let Some(from_expression_element) = from_expression_element {
        expressions.push(from_expression_element.segments()[0].clone());
    }

    for expression in expressions {
        if expression.is_type(SyntaxKind::TableReference) {
            sources.insert(expression.raw().to_string(), expression.clone());
            continue;
        }

        let scope = scope.branch(expression);
        acc.append(&mut traverse_scope(scope));
    }

    scope.sources.extend(sources);
}
