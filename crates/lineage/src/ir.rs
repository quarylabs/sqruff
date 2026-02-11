use append_only_vec::AppendOnlyVec;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::{Node, NodeData};

pub struct Tables {
    pub exprs: AppendOnlyVec<ExprData>,
    pub nodes: AppendOnlyVec<NodeData>,
}

impl Default for Tables {
    fn default() -> Self {
        let exprs = AppendOnlyVec::new();
        exprs.push(ExprData {
            kind: ExprKind::Placeholder,
            parent: None,
            comments: Vec::new(),
        });

        Self {
            exprs,
            nodes: AppendOnlyVec::new(),
        }
    }
}

impl Tables {
    pub(crate) fn alloc_expr(&self, kind: ExprKind, parent: Option<Expr>) -> Expr {
        self.exprs.push(ExprData {
            kind,
            parent,
            comments: Vec::new(),
        })
    }

    pub(crate) fn selects(&self, expr: Expr) -> Vec<Expr> {
        match &self.exprs[expr].kind {
            ExprKind::Select { projections, .. } => projections.clone(),
            &ExprKind::Union { left, right: _ } => self.selects(self.unnest(left)),
            &ExprKind::ValuesClause(_, Some(alias)) => {
                let ExprKind::TableAlias(_, alias) = self.exprs[alias].kind else {
                    unimplemented!()
                };

                vec![alias.unwrap()]
            }
            _ => vec![],
        }
    }

    #[track_caller]
    pub(crate) fn selects_mut(&mut self, expr: Expr) -> &mut Vec<Expr> {
        // Check variant with immutable borrow first to avoid borrow checker issues
        let is_select = matches!(&self.exprs[expr].kind, ExprKind::Select { .. });

        if is_select {
            let ExprKind::Select { projections, .. } = &mut self.exprs[expr].kind else {
                unreachable!()
            };
            return projections;
        }

        if let ExprKind::Union { left, .. } = self.exprs[expr].kind {
            let select = self.unnest(left);
            return self.selects_mut(select);
        }

        unimplemented!("{:?}", self.stringify(expr))
    }

    pub(crate) fn is_star(&self, expr: Expr) -> bool {
        match &self.exprs[expr].kind {
            ExprKind::Select { projections, .. } => projections
                .iter()
                .any(|&projection| self.is_star(projection)),
            ExprKind::Star => true,
            _ => false,
        }
    }

    pub(crate) fn alloc_node(&self, node: NodeData) -> Node {
        self.nodes.push(node)
    }

    pub(crate) fn alias(&self, expr: Expr, return_this: bool) -> String {
        match &self.exprs[expr].kind {
            ExprKind::TableReference(_, alias) => alias.clone().unwrap_or_default(),
            ExprKind::Alias(_, alias) => alias.clone().unwrap_or_default(),
            ExprKind::Subquery(_, alias) => alias.clone().unwrap_or_default(),
            ExprKind::TableAlias(this, alias) => {
                if return_this {
                    return this.clone();
                }

                alias.map_or(String::new(), |alias| self.stringify(alias))
            }
            ExprKind::ValuesClause(_, alias) => {
                alias.map_or(String::new(), |alias| self.alias(alias, return_this))
            }
            _ => String::new(),
        }
    }

    pub(crate) fn alias_or_name(&self, expr: Expr) -> String {
        match &self.exprs[expr].kind {
            ExprKind::TableReference(name, alias) => alias.clone().unwrap_or_else(|| name.clone()),
            ExprKind::Alias(name, alias) => alias.clone().unwrap_or_else(|| name.clone()),
            &ExprKind::Alias0(_, alias) => self.stringify(alias),
            ExprKind::Subquery(_, alias) => alias.clone().unwrap_or_default(),
            _ => self.stringify(expr),
        }
    }

    fn alloc_dummy_expr(&self, parent: Option<Expr>) -> Expr {
        self.alloc_expr(ExprKind::Dummy, parent)
    }

    fn with_alloc_dummy_expr(
        &mut self,
        parent: Option<Expr>,
        f: impl FnOnce(&mut Tables, Expr) -> ExprKind,
    ) -> Expr {
        let dummy = self.alloc_dummy_expr(parent);
        self.exprs[dummy].kind = f(self, dummy);
        dummy
    }

    pub fn stringify(&self, expr: Expr) -> String {
        match &self.exprs[expr].kind {
            ExprKind::Function(callee, args) => {
                let args = args
                    .iter()
                    .map(|&arg| self.stringify(arg))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{callee}({args})")
            }
            &ExprKind::Join { this, on, using: _ } => {
                let this = self.stringify(this);
                let on = on.map_or(String::new(), |on| self.stringify(on));
                format!("join {this} on {on}")
            }
            ExprKind::Select {
                with,
                projections,
                from,
                joins,
            } => {
                let with = with.map_or(String::new(), |with| self.stringify(with) + " ");
                let projections = projections
                    .iter()
                    .map(|&projection| self.stringify(projection))
                    .collect::<Vec<_>>()
                    .join(", ");

                let from = from
                    .map(|from| " ".to_string() + &self.stringify(from))
                    .unwrap_or_default();

                let joins = joins
                    .iter()
                    .map(|&join| " ".to_string() + &self.stringify(join))
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("{with}select {projections}{from}{joins}")
            }
            &ExprKind::Subquery(subquery, ref alias) => {
                let alias = alias
                    .as_ref()
                    .map_or(String::new(), |alias| format!(" as {alias}"));
                let subquery = self.stringify(subquery);
                format!("({subquery}){alias}")
            }
            &ExprKind::From { this, alias } => {
                let this = self.stringify(this);
                let alias = alias
                    .map(|alias| " as ".to_string() + &self.stringify(alias))
                    .unwrap_or_default();

                format!("from {this}{alias}")
            }
            ExprKind::Table { .. } => todo!(),
            ExprKind::TableReference(reference, alias) => {
                let alias = alias
                    .as_deref()
                    .map_or(String::new(), |alias| format!(" as {alias}"));
                reference.clone() + &alias
            }
            ExprKind::Column(s) | ExprKind::Ident(s) | ExprKind::Wildcard(s) => s.clone(),
            ExprKind::TableAlias(s, alias) => alias.map_or_else(
                || s.clone(),
                |alias| format!("{s} as ({})", self.stringify(alias)),
            ),
            ExprKind::Star => "*".to_owned(),
            ExprKind::Dummy => todo!(),
            ExprKind::Alias(name, alias) => {
                let alias = alias
                    .as_ref()
                    .map(|alias| " as ".to_string() + alias)
                    .unwrap_or_default();
                format!("{name}{alias}")
            }
            ExprKind::Placeholder => todo!(),
            ExprKind::With { ctes } => {
                let ctes = ctes
                    .iter()
                    .map(|&cte| self.stringify(cte))
                    .collect::<Vec<_>>()
                    .join("");
                format!("with {ctes}")
            }
            &ExprKind::Cte { alias, this } => {
                let alias = self.stringify(alias);
                let this = self.stringify(this);
                format!("{alias} as {this}")
            }
            &ExprKind::Alias0(this, alias) => {
                let alias = self.stringify(alias);
                let this = self.stringify(this);
                format!("{this} as {alias}")
            }
            &ExprKind::Union { left, right } => {
                let left = self.stringify(left);
                let right = self.stringify(right);

                format!("{left} union {right}")
            }
            ExprKind::Unknown(segment) => segment.raw().to_string(),
            ExprKind::ValuesClause(segment, alias) => {
                segment.raw().to_string()
                    + &alias.map_or(String::new(), |alias| {
                        " ".to_owned() + &self.stringify(alias)
                    })
            }
            &ExprKind::Add(lhs, rhs) => {
                let lhs = self.stringify(lhs);
                let rhs = self.stringify(rhs);

                format!("{lhs} + {rhs}")
            }
            &ExprKind::Sub(lhs, rhs) => {
                let lhs = self.stringify(lhs);
                let rhs = self.stringify(rhs);

                format!("{lhs} - {rhs}")
            }
        }
    }

    pub(crate) fn unnest(&self, mut expr: Expr) -> Expr {
        while let &ExprKind::Subquery(subquery, _) = &self.exprs[expr].kind {
            expr = subquery;
        }
        expr
    }

    pub(crate) fn walk<'a, F: FnMut(&Tables, Expr) -> bool + 'a>(
        &'a self,
        expr: Expr,
        mut prune: Option<F>,
    ) -> impl Iterator<Item = Expr> + 'a {
        let mut queue = vec![expr];
        let mut last = None;

        std::iter::from_fn(move || {
            if let Some(last_node) = last.take()
                && prune.as_mut().is_none_or(|prune| !prune(self, last_node))
            {
                match &self.exprs[last_node].kind {
                    ExprKind::Select {
                        with,
                        projections,
                        from,
                        joins,
                    } => {
                        queue.extend(joins.iter().rev());
                        queue.extend(from);
                        queue.extend(projections.iter().rev());
                        queue.extend(with);
                    }
                    &ExprKind::Subquery(subquery, _) => {
                        queue.push(subquery);
                    }
                    ExprKind::With { ctes } => {
                        queue.extend(ctes.iter().rev());
                    }
                    &ExprKind::Cte { alias, this } => {
                        queue.push(this);
                        queue.push(alias);
                    }
                    &ExprKind::Alias0(this, alias) => {
                        queue.push(alias);
                        queue.push(this);
                    }
                    ExprKind::Function(_, args) => {
                        queue.extend(args.iter().rev());
                    }
                    ExprKind::TableAlias(_, alias) => {
                        queue.extend(alias);
                    }
                    &ExprKind::From { this, alias } => {
                        queue.extend(alias);
                        queue.push(this);
                    }
                    &ExprKind::Join { this, on, using } => {
                        queue.extend(using);
                        queue.extend(on);
                        queue.push(this);
                    }
                    &ExprKind::Add(lhs, rhs) | &ExprKind::Sub(lhs, rhs) => {
                        queue.extend([rhs, lhs]);
                    }
                    _ => {}
                };
            }

            last = queue.pop();
            last
        })
    }
}

pub(crate) type Expr = usize;

pub struct ExprData {
    pub(crate) kind: ExprKind,
    pub(crate) parent: Option<Expr>,
    #[allow(dead_code)]
    pub(crate) comments: Vec<String>,
}

impl ExprData {
    pub(crate) fn parent(&self) -> Option<Expr> {
        self.parent
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ExprKind {
    With {
        ctes: Vec<Expr>,
    },
    Select {
        with: Option<Expr>,
        projections: Vec<Expr>,
        from: Option<Expr>,
        joins: Vec<Expr>,
    },
    Cte {
        alias: Expr,
        this: Expr,
    },
    Subquery(Expr, Option<String>),
    From {
        this: Expr,
        alias: Option<Expr>,
    },
    #[allow(dead_code)]
    Table {
        this: Expr,
        alias: Expr,
    },
    TableAlias(String, Option<Expr>),
    Column(String),
    TableReference(String, Option<String>),
    Alias(String, Option<String>),
    Alias0(Expr, Expr),
    Star,
    Wildcard(String),
    Ident(String),
    Function(String, Vec<Expr>),
    Dummy,
    Placeholder,
    Union {
        left: Expr,
        right: Expr,
    },
    #[allow(dead_code)]
    Join {
        this: Expr,
        on: Option<Expr>,
        using: Option<Expr>,
    },
    ValuesClause(ErasedSegment, Option<Expr>),
    Add(Expr, Expr),
    Sub(Expr, Expr),
    Unknown(ErasedSegment),
}

pub(crate) fn lower(segment: ErasedSegment) -> (Tables, Expr) {
    let mut tables = Tables::default();

    let mut stmts = specific_statement_segment(segment);
    let stmt = stmts.pop().unwrap();

    let root_expr = lower_inner(&mut tables, stmt, None);
    (tables, root_expr)
}

pub(crate) fn lower_inner(
    tables: &mut Tables,
    segment: ErasedSegment,
    parent: Option<Expr>,
) -> Expr {
    let syntax_kind = segment.get_type();

    match syntax_kind {
        SyntaxKind::SelectStatement => {
            let projections = segment.recursive_crawl(
                const { &SyntaxSet::single(SyntaxKind::SelectClauseElement) },
                true,
                const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
                false,
            );

            tables.with_alloc_dummy_expr(parent, |tables, select| {
                let projections: Vec<_> = projections
                    .into_iter()
                    .map(|projection| {
                        let this = first_code(projection.clone());

                        let alias = projection
                            .child(const { &SyntaxSet::single(SyntaxKind::AliasExpression) })
                            .map(|alias| {
                                let raw_segments = alias.get_raw_segments();
                                let alias = raw_segments
                                    .iter()
                                    .rev()
                                    .find(|it| it.is_code())
                                    .unwrap()
                                    .clone();

                                lower_inner(tables, alias, select.into())
                            });

                        let this = lower_inner(tables, this, Some(alias.unwrap_or(select)));
                        if let Some(alias) = alias {
                            tables.alloc_expr(ExprKind::Alias0(this, alias), select.into())
                        } else {
                            this
                        }
                    })
                    .collect();

                let from = segment.recursive_crawl(
                    const { &SyntaxSet::single(SyntaxKind::FromExpressionElement) },
                    true,
                    const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
                    false,
                );

                let from = from.into_iter().next().map(|from_expression| {
                    let from = tables.alloc_expr(ExprKind::Dummy, parent);

                    let this = from_expression
                        .recursive_crawl(
                            const { &SyntaxSet::single(SyntaxKind::TableExpression) },
                            false,
                            &SyntaxSet::EMPTY,
                            false,
                        )
                        .into_iter()
                        .next()
                        .unwrap();

                    let this = first_code(this);

                    let this = lower_inner(tables, this, from.into());

                    let mut alias = from_expression
                        .child(const { &SyntaxSet::single(SyntaxKind::AliasExpression) })
                        .map(|alias| {
                            if let Some(bracketed) =
                                alias.child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
                            {
                                let naked_identifier = alias
                                    .child(
                                        const { &SyntaxSet::single(SyntaxKind::NakedIdentifier) },
                                    )
                                    .unwrap()
                                    .raw()
                                    .to_string();
                                let alias = bracketed
                                    .raw()
                                    .trim_start_matches('(')
                                    .trim_end_matches(')')
                                    .to_string();

                                let alias =
                                    tables.alloc_expr(ExprKind::Column(alias), select.into());

                                tables.alloc_expr(
                                    ExprKind::TableAlias(naked_identifier, alias.into()),
                                    select.into(),
                                )
                            } else {
                                let raw_segments = alias.get_raw_segments();
                                let alias = raw_segments
                                    .iter()
                                    .rev()
                                    .find(|it| it.is_code())
                                    .unwrap()
                                    .clone();

                                lower_inner(tables, alias, from.into())
                            }
                        });

                    if let ExprKind::ValuesClause(_, slot) = &mut tables.exprs[this].kind {
                        *slot = alias;
                        alias = None;
                    }

                    let mut alias_ = alias.map(|this| tables.stringify(this));
                    if let ExprKind::Subquery(_, slot) = &mut tables.exprs[this].kind {
                        *slot = std::mem::take(&mut alias_);
                        alias = None;
                    }

                    if let ExprKind::TableReference(_, slot) = &mut tables.exprs[this].kind {
                        *slot = std::mem::take(&mut alias_);
                        alias = None;
                    }

                    tables.exprs[from].kind = ExprKind::From { this, alias };

                    from
                });

                let join_clauses = segment.recursive_crawl(
                    const { &SyntaxSet::single(SyntaxKind::JoinClause) },
                    true,
                    const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
                    false,
                );

                let joins: Vec<_> = join_clauses
                    .into_iter()
                    .map(|join_clause| {
                        let mut cursor = Cursor::new(join_clause.segments());

                        cursor.skip_if(&[
                            "ANTI",
                            "CROSS",
                            "INNER",
                            "OUTER",
                            "SEMI",
                            "STRAIGHT_JOIN",
                        ]);

                        cursor.skip_if(&["JOIN"]);

                        let this = cursor.next().unwrap().clone();

                        let this = lower_inner(tables, this, parent);

                        let on = if let Some(join_on_condition) =
                            cursor.next_if(SyntaxKind::JoinOnCondition)
                        {
                            let mut cursor = Cursor::new(join_on_condition.segments());

                            debug_assert_eq!(cursor.next().unwrap().raw(), "ON");
                            let value = cursor.next().unwrap().clone();

                            Some(lower_inner(tables, value, parent))
                        } else {
                            None
                        };

                        tables.alloc_expr(
                            ExprKind::Join {
                                this,
                                on,
                                using: None,
                            },
                            parent,
                        )
                    })
                    .collect();

                ExprKind::Select {
                    with: None,
                    projections,
                    from,
                    joins,
                }
            })
        }
        SyntaxKind::Bracketed => tables.with_alloc_dummy_expr(parent, |tables, subquery| {
            let segment = segment
                .segments()
                .iter()
                .find(|it| !it.segments().is_empty())
                .unwrap();
            let select = lower_inner(tables, segment.clone(), subquery.into());
            ExprKind::Subquery(select, None)
        }),
        SyntaxKind::ColumnReference => {
            tables.alloc_expr(ExprKind::Column(segment.raw().to_string()), parent)
        }
        SyntaxKind::WildcardExpression => {
            let id = segment.raw().to_string();

            tables.alloc_expr(
                if id == "*" {
                    ExprKind::Star
                } else {
                    ExprKind::Wildcard(id)
                },
                parent,
            )
        }
        SyntaxKind::FromExpressionElement => {
            let mut cursor = Cursor::new(segment.segments());

            let mut this = cursor.next().unwrap().clone();

            if this.get_type() == SyntaxKind::TableExpression {
                this = first_code(this);
            }

            let this = lower_inner(tables, this, parent);

            if let Some(maybe_alias) = cursor.next()
                && maybe_alias.get_type() == SyntaxKind::AliasExpression
            {
                let maybe_alias = maybe_alias.clone();
                let raw_segments = maybe_alias.get_raw_segments();
                let alias = raw_segments
                    .iter()
                    .rev()
                    .find(|it| it.is_code())
                    .unwrap()
                    .clone();

                if let ExprKind::TableReference(_, slot) = &mut tables.exprs[this].kind {
                    *slot = Some(alias.raw().to_string());
                }
            }

            this
        }
        SyntaxKind::TableReference => tables.alloc_expr(
            ExprKind::TableReference(segment.raw().to_string(), None),
            parent,
        ),
        SyntaxKind::ObjectReference => tables.alloc_expr(
            ExprKind::TableReference(segment.raw().to_string(), None),
            parent,
        ),
        SyntaxKind::NakedIdentifier => {
            tables.alloc_expr(ExprKind::Ident(segment.raw().to_string()), parent)
        }
        SyntaxKind::WithCompoundStatement => {
            let select = segment
                .child(const { &SyntaxSet::single(SyntaxKind::SelectStatement) })
                .unwrap();
            let select = lower_inner(tables, select, parent);

            let cte = segment
                .child(&const { SyntaxSet::single(SyntaxKind::CommonTableExpression) })
                .unwrap();
            let cte = lower_inner(tables, cte, select.into());

            let with = tables.alloc_expr(ExprKind::With { ctes: vec![cte] }, parent);

            let ExprKind::Select { with: slot, .. } = &mut tables.exprs[select].kind else {
                unreachable!()
            };

            *slot = Some(with);

            select
        }
        SyntaxKind::CommonTableExpression => {
            let (alias, rest) = segment.segments().split_first().unwrap();
            let alias =
                tables.alloc_expr(ExprKind::TableAlias(alias.raw().to_string(), None), parent);

            let this = rest
                .iter()
                .find(|segment| !segment.segments().is_empty())
                .unwrap();
            let this = lower_inner(tables, this.clone(), parent);

            tables.alloc_expr(ExprKind::Cte { alias, this }, parent)
        }
        SyntaxKind::Expression => {
            let mut segments = Cursor::new(segment.segments());

            let lhs = segments.next().unwrap();
            let mut lhs = lower_inner(tables, lhs.clone(), parent);

            while let Some(op) = segments.next() {
                let op = match op.raw().as_ref() {
                    "+" => ExprKind::Add,
                    "=" => ExprKind::Sub,
                    _ => unimplemented!(),
                };

                let rhs = segments.next().unwrap();
                let rhs = lower_inner(tables, rhs.clone(), parent);

                lhs = tables.alloc_expr(op(lhs, rhs), parent);
            }

            lhs
        }
        SyntaxKind::Function => {
            let name = segment
                .child(const { &SyntaxSet::single(SyntaxKind::FunctionName) })
                .unwrap()
                .raw()
                .to_string();

            let args = segment
                .child(const { &SyntaxSet::single(SyntaxKind::FunctionContents) })
                .unwrap()
                .child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
                .unwrap();
            let args: Vec<_> = args
                .segments()
                .iter()
                .filter(|it| !it.segments().is_empty())
                .map(|arg| lower_inner(tables, arg.clone(), parent))
                .collect();

            tables.alloc_expr(ExprKind::Function(name, args), parent)
        }
        SyntaxKind::SetExpression => {
            let mut selects = Vec::new();
            let mut _operator = None;

            let mut this = None;

            for child in segment.segments() {
                if child.is_type(SyntaxKind::SetOperator) {
                    _operator = Some(child.raw().to_string());
                }

                if child.is_type(SyntaxKind::SelectStatement) {
                    selects.push(lower_inner(tables, child.clone(), parent));
                }

                if selects.len() == 2 {
                    let right = selects.pop().unwrap();
                    let left = selects.pop().unwrap();

                    this = Some(tables.alloc_expr(ExprKind::Union { left, right }, parent));
                }
            }

            if let Some(tail) = selects.pop() {
                let &ExprKind::Union { left, right } = &tables.exprs[this.unwrap()].kind else {
                    unimplemented!()
                };

                let new_left = tables.alloc_expr(ExprKind::Union { left, right }, parent);

                let ExprKind::Union { left, right: slot } = &mut tables.exprs[this.unwrap()].kind
                else {
                    unimplemented!()
                };

                *left = new_left;
                *slot = tail;
            }

            this.unwrap()
        }
        SyntaxKind::ValuesClause => {
            tables.alloc_expr(ExprKind::ValuesClause(segment, None), parent)
        }
        _ => tables.alloc_expr(ExprKind::Unknown(segment), parent),
    }
}

fn first_code(segment: ErasedSegment) -> ErasedSegment {
    segment
        .segments()
        .iter()
        .find(|it| it.is_code())
        .unwrap()
        .clone()
}

pub(crate) fn specific_statement_segment(parsed: ErasedSegment) -> Vec<ErasedSegment> {
    let mut segments = Vec::new();

    for top_segment in parsed.segments() {
        match top_segment.get_type() {
            SyntaxKind::Statement => {
                segments.push(top_segment.segments()[0].clone());
            }
            SyntaxKind::Batch => unimplemented!(),
            _ => {}
        }
    }

    segments
}

struct Cursor<'me> {
    iter: std::iter::Peekable<std::slice::Iter<'me, ErasedSegment>>,
}

impl<'me> Cursor<'me> {
    fn new(segments: &'me [ErasedSegment]) -> Self {
        Self {
            iter: segments.iter().peekable(),
        }
    }

    fn peek(&mut self) -> Option<&ErasedSegment> {
        let peeked = *self.iter.peek()?;
        if peeked.is_code() {
            Some(peeked)
        } else {
            self.iter.next();
            self.peek()
        }
    }

    fn next(&mut self) -> Option<&ErasedSegment> {
        let next = self.iter.next()?;
        if next.is_code() {
            Some(next)
        } else {
            self.next()
        }
    }

    fn next_if(&mut self, syntax_kind: SyntaxKind) -> Option<ErasedSegment> {
        let peeked = self.peek()?;
        if peeked.get_type() == syntax_kind {
            Some(peeked.clone())
        } else {
            None
        }
    }

    fn skip_if(&mut self, raws: &[&str]) -> bool {
        if let Some(peeked) = self.peek()
            && raws.contains(&peeked.raw().to_uppercase().as_str())
        {
            self.next();
            return true;
        }

        false
    }
}
