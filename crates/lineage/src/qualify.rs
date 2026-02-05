use hashbrown::hash_map::Entry;
use hashbrown::{HashMap, HashSet};
use std::cell::{OnceCell, RefCell};

use indexmap::IndexMap;

use crate::ir::{Expr, ExprKind, Tables};
use crate::schema::Schema;
use crate::scope::{Scope, Source};

pub(crate) fn qualify(tables: &mut Tables, schema: &Schema, expr: Expr) {
    qualify_tables(tables, expr);
    qualify_columns(tables, schema, expr);
}

fn qualify_tables(tables: &mut Tables, expr: Expr) {
    let next_alias_name = name_sequence("_q_");

    for scope in crate::scope::traverse(tables, expr) {
        for &derived_table in &scope.stats(tables).derived_tables {
            if let ExprKind::Subquery(_, slot @ None) = &mut tables.exprs[derived_table].kind {
                let alias = next_alias_name();
                *slot = Some(alias);
            }
        }

        for (name, source) in &scope.get().sources {
            match source {
                &Source::Expr(expr) => {
                    if let ExprKind::TableReference(_, slot @ None) = &mut tables.exprs[expr].kind {
                        *slot = Some(name.clone());
                    }
                }
                Source::Scope(_) => {
                    for node in crate::scope::walk_in_scope(tables, scope.expr()) {
                        let node_data = &tables.exprs[node];

                        if node_data.parent.is_some_and(|parent| {
                            matches!(&tables.exprs[parent].kind, ExprKind::From { .. })
                        }) && let ExprKind::TableReference(_, slot @ None) =
                            &mut tables.exprs[node].kind
                        {
                            *slot = Some(name.clone());
                        }
                    }
                }
            }
        }
    }
}

fn qualify_columns(tables: &mut Tables, schema: &Schema, expr: Expr) {
    let infer_schema = schema.is_empty();

    for scope in crate::scope::traverse(tables, expr) {
        let mut resolver = Resolver::new(&scope, schema);
        resolver.infer_schema = infer_schema;

        qualify_columns0(tables, &scope, &resolver);
        expand_stars(tables, &scope, &resolver);
        qualify_outputs(tables, &scope);
    }
}

fn qualify_columns0(tables: &mut Tables, scope: &Scope, resolver: &Resolver) {
    for &column in &scope.stats(tables).raw_columns {
        let s = tables.stringify(column);

        let mut iter = s.split(".");
        let column_name = iter.next().unwrap();
        let table = iter.next();

        if table.is_none()
            && let Some(column_table) = resolver.table(tables, column_name)
        {
            let column_table = tables.stringify(column_table);
            tables.exprs[column].kind = ExprKind::Column(format!("{column_table}.{column_name}"));
        }
    }
}

fn expand_stars(tables: &mut Tables, scope: &Scope, resolver: &Resolver) {
    let mut new_selections = Vec::new();

    let projections = tables.selects(scope.expr());

    for projection in projections {
        let mut tables_ = Vec::new();

        if let ExprKind::Star = tables.exprs[projection].kind {
            let sources = scope.selected_sources(tables);
            tables_.extend(sources.keys().cloned());
        }

        if tables_.is_empty() {
            new_selections.push(projection);
            continue;
        }

        for table in tables_ {
            if !scope.get().sources.contains_key(&table) {
                unreachable!("Unknown table: {table}")
            }

            let columns = resolver.source_columns(tables, &table, true);

            for name in columns {
                let alias = name.clone();
                let mut selection_expr =
                    tables.alloc_expr(ExprKind::Column(format!("{table}.{name}")), None);

                if name != alias {
                    let alias = tables.alloc_expr(ExprKind::Alias(alias, None), None);
                    selection_expr = tables.alloc_expr(
                        ExprKind::Table {
                            this: selection_expr,
                            alias,
                        },
                        None,
                    );
                }

                new_selections.push(selection_expr);
            }
        }
    }

    if !new_selections.is_empty()
        && matches!(tables.exprs[scope.expr()].kind, ExprKind::Select { .. })
    {
        *tables.selects_mut(scope.expr()) = new_selections;
    }
}

fn qualify_outputs(tables: &mut Tables, scope: &Scope) {
    let mut new_projections = Vec::new();
    let projections = tables.selects(scope.expr());

    for mut projection in projections {
        match tables.exprs[projection].kind {
            ExprKind::Subquery(_, _) => {}
            ExprKind::Column(_) => {
                let s = tables.stringify(projection);
                let name = s.split(".").last().unwrap();

                projection =
                    tables.alloc_expr(ExprKind::Alias(s.clone(), name.to_owned().into()), None);
            }
            _ => {}
        }

        new_projections.push(projection);
    }

    if !new_projections.is_empty()
        && matches!(tables.exprs[scope.expr()].kind, ExprKind::Select { .. })
    {
        *tables.selects_mut(scope.expr()) = new_projections;
    }
}

struct Resolver<'scope, 'schema> {
    scope: &'scope Scope,
    schema: &'schema Schema,
    infer_schema: bool,
    unambiguous_columns: OnceCell<HashMap<String, String>>,
    source_columns: OnceCell<IndexMap<String, Vec<String>>>,
    source_columns_cache: RefCell<HashMap<(String, bool), Vec<String>>>,
}

impl<'scope, 'schema> Resolver<'scope, 'schema> {
    fn new(scope: &'scope Scope, schema: &'schema Schema) -> Self {
        Self {
            scope,
            schema,
            infer_schema: true,
            unambiguous_columns: OnceCell::new(),
            source_columns: OnceCell::new(),
            source_columns_cache: RefCell::new(HashMap::new()),
        }
    }

    fn unambiguous_columns(&self, tables: &Tables) -> &HashMap<String, String> {
        self.unambiguous_columns.get_or_init(|| {
            let source_columns = self.all_source_columns(tables);

            if source_columns.is_empty() {
                return HashMap::new();
            }

            let mut source_columns_iter = source_columns.iter();
            let (first_table, first_columns) = source_columns_iter.next().unwrap();

            if source_columns.len() == 1 {
                // Performance optimization - avoid copying if there's only one table
                return first_columns
                    .iter()
                    .map(|column| (column.clone(), first_table.clone()))
                    .collect();
            }

            let mut unambiguous_columns: HashMap<String, String> = first_columns
                .iter()
                .map(|col| (col.clone(), first_table.clone()))
                .collect();
            let mut all_columns: HashSet<String> = first_columns.iter().cloned().collect();

            for (table, columns) in source_columns_iter {
                let unique: HashSet<String> = columns.iter().cloned().collect();
                let ambiguous: HashSet<String> =
                    all_columns.intersection(&unique).cloned().collect();
                all_columns.extend(unique.iter().cloned());

                for column in &ambiguous {
                    unambiguous_columns.remove(column);
                }
                for column in unique.difference(&ambiguous) {
                    unambiguous_columns.insert(column.clone(), table.clone());
                }
            }

            unambiguous_columns
        })
    }

    fn source_columns(&self, tables: &Tables, name: &str, only_visible: bool) -> Vec<String> {
        let cache_key = (name.to_owned(), only_visible);

        match self.source_columns_cache.borrow_mut().entry(cache_key) {
            Entry::Occupied(occupied) => occupied.get().clone(),
            Entry::Vacant(vacant) => {
                let source = &self.scope.get().sources[name];

                let columns = match source {
                    &Source::Expr(table) => self.schema.column_names(tables, table, only_visible),
                    Source::Scope(scope) => {
                        let projections = tables.selects(scope.expr());
                        projections
                            .iter()
                            .filter_map(|&projection| match &tables.exprs[projection].kind {
                                ExprKind::TableReference(v, alias) => {
                                    Some(alias.clone().unwrap_or(v.clone()))
                                }
                                ExprKind::Column(v) => Some(v.clone()),
                                ExprKind::Alias(a, alias) => {
                                    Some(alias.clone().unwrap_or(a.clone()))
                                }
                                ExprKind::Star => Some("*".to_owned()),
                                _ => None,
                            })
                            .collect()
                    }
                };

                vacant.insert(columns).clone()
            }
        }
    }

    fn table(&self, tables: &Tables, column_name: &str) -> Option<Expr> {
        let mut table_name = self.unambiguous_columns(tables).get(column_name);

        if table_name.is_none() && self.infer_schema {
            let mut sources_without_schema: Vec<_> = self
                .all_source_columns(tables)
                .iter()
                .filter(|(_source, columns)| {
                    columns.is_empty() || columns.contains(&"*".to_owned())
                })
                .map(|(source, _)| source)
                .collect();

            if sources_without_schema.len() == 1 {
                table_name = Some(sources_without_schema.pop().unwrap());
            }
        }

        let table_name = table_name?;

        match self.scope.selected_sources(tables).get(table_name) {
            Some(&(mut node, _)) => {
                if let ExprKind::Select { .. } = &tables.exprs[node].kind {
                    while &tables.alias(node, false) != table_name {
                        if let Some(parent) = tables.exprs[node].parent {
                            node = parent;
                        } else {
                            break;
                        }
                    }
                }

                let node_alias = tables.alias(node, true);

                let this = if !node_alias.is_empty() {
                    node_alias
                } else {
                    table_name.clone()
                };
                Some(tables.alloc_expr(ExprKind::Ident(this), None))
            }
            None => todo!(),
        }
    }

    fn all_source_columns(&self, tables: &Tables) -> &IndexMap<String, Vec<String>> {
        self.source_columns.get_or_init(|| {
            self.scope
                .selected_sources(tables)
                .keys()
                .map(|source_name| {
                    (
                        source_name.clone(),
                        self.source_columns(tables, source_name, false),
                    )
                })
                .collect()
        })
    }
}

fn name_sequence(prefix: &'static str) -> impl Fn() -> String {
    let sequence = std::cell::Cell::new(0);

    move || {
        let current = sequence.get();
        sequence.set(current + 1);
        format!("{prefix}{current}")
    }
}
