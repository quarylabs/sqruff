use hashbrown::HashMap;
use std::cell::{Cell, OnceCell, Ref, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use sqruff_lib_core::helpers::Config;

use crate::ir::{Expr, ExprData, ExprKind, Tables};

#[derive(Clone, Debug)]
pub(crate) struct Scope {
    inner: Rc<RefCell<ScopeInner>>,
}

impl Scope {
    pub(crate) fn get(&self) -> Ref<'_, ScopeInner> {
        self.inner.borrow()
    }

    pub(crate) fn get_mut(&self) -> std::cell::RefMut<'_, ScopeInner> {
        self.inner.borrow_mut()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ScopeInner {
    expr: Expr,
    kind: ScopeKind,
    parent: Option<Expr>,
    pub sources: HashMap<String, Source>,
    cte_sources: HashMap<String, Source>,
    cte_scopes: Vec<Scope>,
    stats: OnceCell<Stats>,
    selected_sources: OnceCell<IndexMap<String, (Expr, Source)>>,
    references: OnceCell<Vec<(String, Expr)>>,
    subquery_scopes: Vec<Scope>,
    pub union_scopes: Option<[Scope; 2]>,
}

impl ScopeInner {
    pub(crate) fn kind(&self) -> ScopeKind {
        self.kind
    }

    pub(crate) fn subquery_scopes(&self) -> &[Scope] {
        &self.subquery_scopes
    }
}

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScopeKind {
    #[default]
    Root,
    Subquery,
    DerivedTable,
    Cte,
    Union,
    Udtf,
}

#[derive(Clone, Debug)]
pub(crate) enum Source {
    Expr(Expr),
    Scope(Scope),
}

impl Scope {
    pub(crate) fn stats(&self, tables: &Tables) -> Ref<'_, Stats> {
        Ref::map(self.get(), |scope| {
            scope.stats.get_or_init(|| {
                let mut stats = Stats::default();

                let exprs = walk_in_scope(tables, self.expr());
                for node in exprs {
                    if node == self.get().expr {
                        continue;
                    }

                    match &tables.exprs[node].kind {
                        ExprKind::Column(..) => stats.raw_columns.push(node),
                        ExprKind::TableReference(..) => stats.tables.push(node),
                        ExprKind::Cte { .. } => stats.ctes.push(node),
                        ExprKind::Select { .. } => stats.subqueries.push(node),
                        ExprKind::ValuesClause(_, _) => stats.udtfs.push(node),
                        _ => {
                            let expr_data = &tables.exprs[node];
                            if is_derived_table(expr_data)
                                && expr_data.parent().is_some_and(|parent| {
                                    matches!(
                                        tables.exprs[parent].kind,
                                        ExprKind::From { .. } | ExprKind::Subquery(..)
                                    )
                                })
                            {
                                stats.derived_tables.push(node);
                            }
                        }
                    }
                }

                stats
            })
        })
    }

    pub(crate) fn selected_sources(
        &self,
        tables: &Tables,
    ) -> Ref<'_, IndexMap<String, (Expr, Source)>> {
        Ref::map(self.get(), |scope| {
            scope.selected_sources.get_or_init(|| {
                let references = self.references(tables);
                let mut result = IndexMap::default();

                for (name, node) in references.iter() {
                    if result.contains_key(name) {
                        panic!("Alias already used: {name}");
                    }

                    if let Some(source) = scope.sources.get(name) {
                        result.insert(name.clone(), (*node, source.clone()));
                    }
                }

                result
            })
        })
    }

    pub(crate) fn references(&self, tables: &Tables) -> Ref<'_, Vec<(String, usize)>> {
        Ref::map(self.get(), |scope| {
            scope.references.get_or_init(|| {
                let mut references = Vec::new();
                let stats = self.stats(tables);

                for &table in &stats.tables {
                    let ExprKind::TableReference(name, alias) = &tables.exprs[table].kind else {
                        unreachable!()
                    };

                    let alias_or_name = alias.as_deref().unwrap_or(name).to_owned();
                    references.push((alias_or_name, table));
                }

                for &expr in &stats.derived_tables {
                    let ExprKind::Subquery(_, alias) = &tables.exprs[expr].kind else {
                        unimplemented!()
                    };

                    let alias = alias.clone().unwrap_or_default();
                    let expr = tables.unnest(expr);

                    references.push((alias, expr));
                }

                for &expr in &stats.udtfs {
                    let ExprKind::ValuesClause(_, alias) = tables.exprs[expr].kind else {
                        unreachable!()
                    };

                    let alias = tables.alias(alias.unwrap(), true);
                    let expr = tables.unnest(expr);
                    references.push((alias, expr));
                }

                references
            })
        })
    }
}

#[derive(Default, Clone, Debug)]
pub(crate) struct Stats {
    pub(crate) tables: Vec<Expr>,
    pub(crate) raw_columns: Vec<Expr>,
    pub(crate) derived_tables: Vec<Expr>,
    pub(crate) ctes: Vec<Expr>,
    pub(crate) subqueries: Vec<Expr>,
    pub(crate) udtfs: Vec<Expr>,
}

impl Scope {
    pub(crate) fn new(expr: Expr) -> Self {
        Scope {
            inner: Rc::new(RefCell::new(ScopeInner {
                expr,
                ..Default::default()
            })),
        }
    }

    pub(crate) fn expr(&self) -> Expr {
        self.get().expr
    }

    pub(crate) fn branch(
        &self,
        tables: &Tables,
        expr: Expr,
        scope_kind: ScopeKind,
        cte_sources: Option<HashMap<String, Source>>,
    ) -> Self {
        Self::new(tables.unnest(expr)).config(|this| {
            let mut inner = this.get_mut();
            inner.kind = scope_kind;
            inner.parent = self.get().expr.into();
            inner.cte_sources = self.get().cte_sources.clone();
            if let Some(cte_sources) = cte_sources {
                inner.cte_sources.extend(cte_sources);
            }
        })
    }
}

pub(crate) fn build(tables: &mut Tables, expr: Expr) -> Scope {
    traverse(tables, expr).pop().unwrap()
}

pub(crate) fn traverse(tables: &mut Tables, expr: Expr) -> Vec<Scope> {
    traverse_scope(tables, Scope::new(expr))
}

fn traverse_scope(tables: &mut Tables, mut scope: Scope) -> Vec<Scope> {
    let mut acc = Vec::new();

    let expr = scope.get().expr;
    match tables.exprs[expr].kind {
        ExprKind::Select { .. } => traverse_select(tables, &mut scope, &mut acc),
        ExprKind::Union { .. } => {
            traverse_ctes(tables, &mut scope, &mut acc);
            traverse_union(tables, &mut scope, &mut acc);
            return acc;
        }
        ExprKind::ValuesClause(_, _) => {}
        _ => return acc,
    };

    acc.push(scope);
    acc
}

fn traverse_select(tables: &mut Tables, scope: &mut Scope, acc: &mut Vec<Scope>) {
    traverse_ctes(tables, scope, acc);
    traverse_tables(tables, scope, acc);
    traverse_subqueries(tables, scope, acc);
}

fn traverse_union(tables: &mut Tables, scope: &mut Scope, acc: &mut Vec<Scope>) {
    let ExprKind::Union { left, right } = tables.exprs[scope.get().expr].kind.clone() else {
        unimplemented!()
    };

    let mut prev_scope = None;
    let mut union_scope_stack = vec![scope.clone()];
    let mut expression_stack = vec![right, left];

    while let Some(expression) = expression_stack.pop() {
        let union_scope = union_scope_stack.last().cloned().unwrap();

        let new_scope = union_scope.branch(tables, expression, ScopeKind::Union, None);

        if let ExprKind::Union { left, right } = tables.exprs[expression].kind {
            traverse_ctes(tables, scope, acc);
            union_scope_stack.push(new_scope.clone());
            expression_stack.extend([right, left]);
            continue;
        }

        acc.extend(traverse_scope(tables, new_scope));

        let scope = acc.last().unwrap().clone();
        match std::mem::take(&mut prev_scope) {
            Some(prevscope) => {
                union_scope_stack.pop();
                union_scope.get_mut().union_scopes = Some([prevscope, scope]);
                prev_scope = Some(union_scope.clone());
                acc.push(union_scope);
            }
            None => {
                prev_scope = Some(scope);
            }
        }
    }
}

fn traverse_ctes(tables: &mut Tables, scope: &mut Scope, acc: &mut Vec<Scope>) {
    let mut sources = HashMap::new();

    let ctes = scope.stats(tables).ctes.clone();

    for cte in ctes {
        let &ExprKind::Cte { this, alias } = &tables.exprs[cte].kind else {
            unreachable!()
        };
        let cte_name = tables.stringify(alias);

        let child_scopes = traverse_scope(
            tables,
            scope.branch(tables, this, ScopeKind::Cte, Some(sources.clone())),
        );
        acc.extend(child_scopes);

        // append the final child_scope
        if let Some(child_scope) = acc.last().cloned() {
            sources.insert(cte_name, Source::Scope(child_scope.clone()));
            scope.get_mut().cte_scopes.push(child_scope);
        }
    }

    scope.get_mut().sources.extend(sources);
}

fn traverse_tables(tables: &mut Tables, scope: &mut Scope, acc: &mut Vec<Scope>) {
    let mut sources = HashMap::new();
    let mut exprs = Vec::new();

    let ExprKind::Select { from, joins, .. } = &tables.exprs[scope.get().expr].kind else {
        unreachable!()
    };

    let mut select_alias = String::new();

    if let Some(from) = *from {
        let ExprKind::From { this, alias } = &tables.exprs[from].kind else {
            unreachable!()
        };
        select_alias = alias
            .map(|alias| tables.stringify(alias))
            .unwrap_or_default();

        exprs.push(*this);

        let maybe_alias = tables.alias(*this, true);
        if !maybe_alias.is_empty() {
            select_alias = maybe_alias;
        }
    }

    for &join in joins {
        let ExprKind::Join {
            this,
            on: _,
            using: _,
        } = tables.exprs[join].kind
        else {
            unreachable!()
        };
        exprs.push(this);
    }

    for expr in exprs {
        if let ExprKind::TableReference(value, alias) = &tables.exprs[expr].kind {
            let table_name = value.clone();
            let source_name = alias.as_deref().unwrap_or(value).to_string();

            #[allow(clippy::map_entry)]
            if scope.get().sources.contains_key(&table_name) {
                sources.insert(source_name, scope.get().sources[&table_name].clone());
            } else {
                sources.insert(source_name, Source::Expr(expr));
            }

            continue;
        }

        let scope = scope.branch(tables, expr, ScopeKind::DerivedTable, None);

        let child_scopes = traverse_scope(tables, scope);
        acc.reserve(child_scopes.len());

        for child_scope in child_scopes {
            acc.push(child_scope.clone());
            sources.insert(select_alias.clone(), Source::Scope(child_scope));
        }
    }

    let mut inner = scope.get_mut();
    inner.sources.extend(sources.clone());
    inner.cte_sources.extend(sources);
}

fn traverse_subqueries(tables: &mut Tables, scope: &mut Scope, acc: &mut Vec<Scope>) {
    let subqueries = scope.stats(tables).subqueries.clone();

    #[allow(clippy::unnecessary_to_owned)] // borrow checker error
    for subquery in subqueries {
        let child_scope = scope.branch(tables, subquery, ScopeKind::Subquery, None);
        let child_scopes = traverse_scope(tables, child_scope);

        let top_scope = child_scopes.last().unwrap();
        scope.get_mut().subquery_scopes.push(top_scope.clone());

        acc.extend(child_scopes);
    }
}

fn is_derived_table(expr: &ExprData) -> bool {
    matches!(expr.kind, ExprKind::Subquery(_, _))
}

pub(crate) fn walk_in_scope(tables: &Tables, root_expr: Expr) -> Vec<Expr> {
    let mut acc = Vec::new();
    let crossed_scope_boundary = Cell::new(false);

    for node in tables.walk(
        root_expr,
        Some(|_tables: &Tables, _node| crossed_scope_boundary.get()),
    ) {
        crossed_scope_boundary.set(false);
        acc.push(node);

        if node == root_expr {
            continue;
        }

        let data = &tables.exprs[node];

        let c1 = matches!(
            data.kind,
            ExprKind::Cte { .. } | ExprKind::Select { .. } | ExprKind::Union { .. }
        );

        let c2 = data.parent.is_some_and(|parent| {
            matches!(
                tables.exprs[parent].kind,
                ExprKind::From { .. } | ExprKind::Subquery(..)
            )
        }) && is_derived_table(data);

        if c1 || c2 {
            crossed_scope_boundary.set(true);
        }
    }

    acc
}
