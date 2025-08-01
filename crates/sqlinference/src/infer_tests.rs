use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::utils::analysis::query::Query;
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::aggregate_functions::aggregate_is_test_inferrable;
use crate::infer_tests::Source::{UnderlyingColumn, UnderlyingColumnWithOperation};
use crate::parse_sql;
use crate::test::{AcceptedValuesTest, ComparisonTest, RelationshipTest, StandardTest, Test};

// TODO Probably could make the inference reason point to tests
#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub enum InferenceReason {
    // UnderlyingTest is a test that was inferred from a parent test.
    UnderlyingTest(Test),
    // UnderlyingTestWithOperation is a test that was inferred from a parent test where the column
    // is operated on. operation with whether or not it is grouped by
    UnderlyingTestWithOperation(Test, (Operation, bool)),
    // CountStar is a test reason for a count(*).
    CountStar,
}

#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub enum Operation {
    Avg,
    Min,
    Max,
}

impl Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Operation::Avg => "avg".to_string(),
            Operation::Min => "min".to_string(),
            Operation::Max => "max".to_string(),
        };
        write!(f, "{str}")
    }
}

/// infer_tests returns test types that can be inferred from parents. It returns
/// a Hashmap of the inferred test to the parent test.
/// path_of_sql: Name to give the sql statement for the tests.
pub fn infer_tests(
    parser: &Parser,
    path_of_sql: &str,
    select_statement: &str,
    tests: &HashSet<Test>,
) -> Result<HashMap<Test, InferenceReason>, String> {
    // TODO Deal with this dialect properly
    let extracted_select = get_column_with_source(parser, select_statement)?;

    match extracted_select {
        ExtractedSelect::Extracted {
            mapped,
            count_stars,
            operated_on,
            ..
        } => {
            let mappings = mapped;
            // (reference, column) to test
            let test_map: HashMap<(String, String), Vec<Test>> =
                tests.iter().fold(HashMap::new(), |mut map, test| {
                    match test {
                        Test::NotNull(t) => {
                            map.entry((t.path.to_string(), t.column.to_string()))
                                .or_default()
                                .push(Test::NotNull(t.clone()));
                        }
                        Test::Unique(t) => {
                            map.entry((t.clone().path, t.clone().column))
                                .or_default()
                                .push(Test::Unique(t.clone()));
                        }
                        Test::AcceptedValues(t) => {
                            map.entry((t.clone().path, t.clone().column))
                                .or_default()
                                .push(Test::AcceptedValues(t.clone()));
                        }
                        Test::Relationship(t) => {
                            map.entry((t.clone().path, t.clone().column))
                                .or_default()
                                .push(Test::Relationship(t.clone()));
                        }
                        Test::GreaterThanOrEqual(t) => {
                            map.entry((t.clone().path, t.clone().column))
                                .or_default()
                                .push(Test::GreaterThanOrEqual(t.clone()));
                        }
                        Test::GreaterThan(t) => {
                            map.entry((t.clone().path, t.clone().column))
                                .or_default()
                                .push(Test::GreaterThan(t.clone()));
                        }
                        Test::LessThanOrEqual(t) => {
                            map.entry((t.clone().path, t.clone().column))
                                .or_default()
                                .push(Test::LessThanOrEqual(t.clone()));
                        }
                        Test::LessThan(t) => {
                            map.entry((t.clone().path, t.clone().column))
                                .or_default()
                                .push(Test::LessThan(t.clone()));
                        }
                    }
                    map
                });

            let mut inferred_from_tests_tests: HashMap<Test, InferenceReason> = mappings
                .iter()
                .flat_map(|(column, target)| {
                    test_map
                        .get(target)
                        .unwrap_or(&vec![])
                        .iter()
                        .map(|t| match t {
                            Test::NotNull(test) => (
                                Test::NotNull(StandardTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                }),
                                Test::NotNull(test.clone()),
                            ),
                            Test::Unique(test) => (
                                Test::Unique(StandardTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                }),
                                Test::Unique(test.clone()),
                            ),
                            Test::AcceptedValues(test) => (
                                Test::AcceptedValues(AcceptedValuesTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    values: test.values.clone(),
                                }),
                                Test::AcceptedValues(test.clone()),
                            ),
                            Test::Relationship(test) => (
                                Test::Relationship(RelationshipTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    target_reference: test.target_reference.to_string(),
                                    target_column: test.target_column.to_string(),
                                }),
                                Test::Relationship(test.clone()),
                            ),
                            Test::GreaterThanOrEqual(test) => (
                                Test::GreaterThanOrEqual(ComparisonTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    value: test.value.clone(),
                                }),
                                Test::GreaterThanOrEqual(test.clone()),
                            ),
                            Test::GreaterThan(test) => (
                                Test::GreaterThan(ComparisonTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    value: test.value.clone(),
                                }),
                                Test::GreaterThan(test.clone()),
                            ),
                            Test::LessThanOrEqual(test) => (
                                Test::LessThanOrEqual(ComparisonTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    value: test.value.clone(),
                                }),
                                Test::LessThanOrEqual(test.clone()),
                            ),
                            Test::LessThan(test) => (
                                Test::LessThan(ComparisonTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    value: test.value.clone(),
                                }),
                                Test::LessThan(test.clone()),
                            ),
                        })
                        .map(|(k, v)| (k, InferenceReason::UnderlyingTest(v)))
                        .collect::<Vec<(Test, InferenceReason)>>()
                })
                .collect();

            count_stars.iter().for_each(|value| {
                inferred_from_tests_tests.insert(
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: path_of_sql.to_string(),
                        column: value.to_string(),
                        value: "0".to_string(),
                    }),
                    InferenceReason::CountStar,
                );
                inferred_from_tests_tests.insert(
                    Test::NotNull(StandardTest {
                        path: path_of_sql.to_string(),
                        column: value.to_string(),
                    }),
                    InferenceReason::CountStar,
                );
            });

            operated_on
                .iter()
                .for_each(|(column, (operation, source))| {
                    // TODO get rid of the unwrap and just map it to an empty array
                    let empty = vec![];
                    let tests_to_map = test_map
                        .get(source)
                        .unwrap_or(&empty)
                        .iter()
                        .filter(|test| {
                            let (operation, group_by) = operation;
                            aggregate_is_test_inferrable(
                                parser.dialect().name(),
                                test,
                                operation,
                                group_by,
                            )
                        })
                        .filter_map(|test| match test {
                            Test::GreaterThanOrEqual(test) => Some((
                                Test::GreaterThanOrEqual(ComparisonTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    value: test.value.to_string(),
                                }),
                                InferenceReason::UnderlyingTestWithOperation(
                                    Test::GreaterThanOrEqual(test.clone()),
                                    operation.clone(),
                                ),
                            )),
                            Test::LessThanOrEqual(test) => Some((
                                Test::LessThanOrEqual(ComparisonTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    value: test.value.to_string(),
                                }),
                                InferenceReason::UnderlyingTestWithOperation(
                                    Test::LessThanOrEqual(test.clone()),
                                    operation.clone(),
                                ),
                            )),
                            Test::GreaterThan(test) => Some((
                                Test::GreaterThan(ComparisonTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    value: test.value.to_string(),
                                }),
                                InferenceReason::UnderlyingTestWithOperation(
                                    Test::GreaterThan(test.clone()),
                                    operation.clone(),
                                ),
                            )),
                            Test::LessThan(test) => Some((
                                Test::LessThan(ComparisonTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                    value: test.value.to_string(),
                                }),
                                InferenceReason::UnderlyingTestWithOperation(
                                    Test::LessThan(test.clone()),
                                    operation.clone(),
                                ),
                            )),
                            Test::NotNull(test) => Some((
                                Test::NotNull(StandardTest {
                                    path: path_of_sql.to_string(),
                                    column: column.to_string(),
                                }),
                                InferenceReason::UnderlyingTestWithOperation(
                                    Test::NotNull(test.clone()),
                                    operation.clone(),
                                ),
                            )),

                            _ => None,
                        });
                    tests_to_map.for_each(|(test, reason)| {
                        inferred_from_tests_tests.insert(test, reason);
                    });
                });

            Ok(inferred_from_tests_tests)
        }
        ExtractedSelect::Star(target) => Ok(tests
            .iter()
            .filter(|test| match test {
                Test::NotNull(t) => t.path == target,
                Test::Unique(t) => t.path == target,
                Test::Relationship(t) => t.path == target,
                Test::AcceptedValues(t) => t.path == target,
                Test::GreaterThanOrEqual(t) => t.path == target,
                Test::GreaterThan(t) => t.path == target,
                Test::LessThanOrEqual(t) => t.path == target,
                Test::LessThan(t) => t.path == target,
            })
            .map(|test| match test {
                Test::NotNull(t) => (
                    Test::NotNull(StandardTest {
                        path: path_of_sql.to_string(),
                        column: t.column.to_string(),
                    }),
                    test.clone(),
                ),
                Test::Unique(t) => (
                    Test::Unique(StandardTest {
                        path: path_of_sql.to_string(),
                        column: t.column.to_string(),
                    }),
                    test.clone(),
                ),
                Test::Relationship(t) => (
                    Test::Relationship(RelationshipTest {
                        path: path_of_sql.to_string(),
                        column: t.column.to_string(),
                        target_reference: t.target_reference.clone(),
                        target_column: t.target_column.clone(),
                    }),
                    test.clone(),
                ),
                Test::AcceptedValues(t) => (
                    Test::AcceptedValues(AcceptedValuesTest {
                        path: path_of_sql.to_string(),
                        column: t.column.to_string(),
                        values: t.values.clone(),
                    }),
                    test.clone(),
                ),
                Test::GreaterThanOrEqual(t) => (
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: path_of_sql.to_string(),
                        column: t.column.to_string(),
                        value: t.value.to_string(),
                    }),
                    test.clone(),
                ),
                Test::GreaterThan(t) => (
                    Test::GreaterThan(ComparisonTest {
                        path: path_of_sql.to_string(),
                        column: t.column.to_string(),
                        value: t.value.to_string(),
                    }),
                    test.clone(),
                ),
                Test::LessThanOrEqual(t) => (
                    Test::LessThanOrEqual(ComparisonTest {
                        path: path_of_sql.to_string(),
                        column: t.column.to_string(),
                        value: t.value.to_string(),
                    }),
                    test.clone(),
                ),
                Test::LessThan(t) => (
                    Test::LessThan(ComparisonTest {
                        path: path_of_sql.to_string(),
                        column: t.column.to_string(),
                        value: t.value.to_string(),
                    }),
                    test.clone(),
                ),
            })
            .map(|(k, v)| (k, InferenceReason::UnderlyingTest(v)))
            .collect::<HashMap<Test, InferenceReason>>()),
    }
}

/// get_column_with_source only returns direct sources at the moment. e.g. FROMs
/// or INNER JOIN.
///   - it supports aliasing
///   - it supports inner joins
///   - it supports ctes/withs
///
/// TODO May want to add the ability to dig multiple levels down in this by
/// parsing a map of sql. Such that columns through a * could be inferred.
/// Result is Result<(HashMap<String: final_column, (String: source_reference,
/// source_column)>, Vec<String>: unrecognized columns), String>
pub fn get_column_with_source(
    parser: &Parser,
    select_statement: &str,
) -> Result<ExtractedSelect, String> {
    let ast = parse_sql(parser, select_statement);
    let query: Query<()> = Query::from_root(&ast, parser.dialect()).unwrap();
    extract_select(&query)
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExtractedSelect {
    Extracted {
        mapped: HashMap<String, (String, String)>,
        unmapped: Vec<String>,
        count_stars: HashSet<String>,
        operated_on: OperatedOn,
    },
    Star(String),
}

// column to source with operated on with bool to specify whether it was grouped
// by
type OperatedOn = HashMap<String, ((Operation, bool), (String, String))>;

/// extract_select returns the columns and unrecognized columns from a select
/// statement. The map in the result is from the final column name to the source
/// column name and source table name. Also returns an array of unrecognized
/// columns.
fn extract_select(query: &Query<'_, ()>) -> Result<ExtractedSelect, String> {
    let with_extracted: Option<Vec<(String, ExtractedSelect)>> =
        if query.inner.borrow().ctes.is_empty() {
            None
        } else {
            query
                .inner
                .borrow()
                .ctes
                .iter()
                .rev()
                .map(|(name, query)| {
                    let select = extract_select(query)?;
                    Ok(Some((name.to_lowercase(), select)))
                })
                .collect::<Result<Option<Vec<_>>, String>>()?
        };

    let main_extracted = if let Some(from_clause) = query.inner.borrow().selectables[0]
        .selectable
        .child(const { &SyntaxSet::single(SyntaxKind::FromClause) })
    {
        let has_group_by = query.inner.borrow().selectables[0]
            .selectable
            .child(const { &SyntaxSet::single(SyntaxKind::GroupbyClause) })
            .is_some();

        let relation = from_clause
            .recursive_crawl(
                const { &SyntaxSet::single(SyntaxKind::FromExpressionElement) },
                true,
                &SyntaxSet::EMPTY,
                false,
            )
            .into_iter()
            .next()
            .unwrap();

        let select_clause = query.inner.borrow().selectables[0]
            .selectable
            .child(const { &SyntaxSet::single(SyntaxKind::SelectClause) })
            .unwrap();

        let select_clause_elements = select_clause.recursive_crawl(
            const { &SyntaxSet::single(SyntaxKind::SelectClauseElement) },
            false,
            &SyntaxSet::EMPTY,
            false,
        );

        let extracted_table = extract_table(&relation, query.inner.borrow().dialect)?;
        let mut extracted_tables = vec![extracted_table];

        let joins = from_clause.recursive_crawl(
            const { &SyntaxSet::single(SyntaxKind::JoinClause) },
            true,
            &SyntaxSet::EMPTY,
            false,
        );
        if !joins.is_empty() {
            let extracted = extract_extracted_from_joins(joins, query.inner.borrow().dialect)?;
            extracted_tables.extend(extracted);
        }

        if select_clause_elements.len() == 1
            && select_clause_elements[0].raw() == "*"
            && extracted_tables.len() == 1
        {
            match extracted_tables.first().unwrap() {
                Extracted::Star(value) => ExtractedSelect::Star(value.clone()),
                // TODO Probably turn this into a type
                Extracted::AliasedSelect(_, target) => target.clone(),
                // TODO Probably turn this into a type
                Extracted::Select(select) => select.clone(),
                Extracted::AliasedStar(_, value) => ExtractedSelect::Star(value.clone()),
                Extracted::ZeroMap(_) => {
                    return Err("Do not support zero maps for wildcard".to_string());
                }
            }
        } else {
            let mut columns: HashMap<String, (String, String)> = HashMap::new();
            let mut unnamed: Vec<String> = vec![];
            let mut count_stars: HashSet<String> = HashSet::new();
            let mut operated_on: OperatedOn = HashMap::new();

            for select_clause_element in select_clause_elements {
                if let Some(alias) = select_clause_element
                    .child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) })
                {
                    let raw_segments = alias.get_raw_segments();
                    let alias = raw_segments.iter().rev().find(|it| it.is_code()).unwrap();

                    if let Some(expr) = select_clause_element
                        .child(const { &SyntaxSet::single(SyntaxKind::Expression) })
                    {
                        if expr
                            .child(
                                const {
                                    &SyntaxSet::new(&[
                                        SyntaxKind::CastExpression,
                                        SyntaxKind::CaseExpression,
                                    ])
                                },
                            )
                            .is_some()
                        {
                            unnamed.push(alias.raw().to_string());
                            continue;
                        }

                        return Err(format!(
                            "Expected Identifier/CompoundIdentifier or Function, not {:?}",
                            expr.raw()
                        ));
                    }

                    if let Some(function) = select_clause_element
                        .child(const { &SyntaxSet::single(SyntaxKind::Function) })
                    {
                        let name = function
                            .child(const { &SyntaxSet::single(SyntaxKind::FunctionName) })
                            .unwrap()
                            .raw()
                            .to_string();
                        let args = function
                            .child(const { &SyntaxSet::single(SyntaxKind::FunctionContents) })
                            .unwrap()
                            .child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
                            .unwrap();
                        let args = fn_args(args);

                        match name.to_lowercase().as_str() {
                            "count" => {
                                if args.first().map(|it| it.raw().as_str()) == Some("*") {
                                    count_stars.insert(alias.raw().to_string());
                                };
                            }
                            "avg" => {
                                avg_min_max_function_parser(
                                    &mut operated_on,
                                    &mut extracted_tables,
                                    alias.raw().to_string(),
                                    args.first().cloned().unwrap(),
                                    Operation::Avg,
                                    has_group_by,
                                )?;
                            }
                            "min" => {
                                avg_min_max_function_parser(
                                    &mut operated_on,
                                    &mut extracted_tables,
                                    alias.raw().to_string(),
                                    args.first().cloned().unwrap(),
                                    Operation::Min,
                                    has_group_by,
                                )?;
                            }
                            "max" => {
                                avg_min_max_function_parser(
                                    &mut operated_on,
                                    &mut extracted_tables,
                                    alias.raw().to_string(),
                                    args.first().cloned().unwrap(),
                                    Operation::Max,
                                    has_group_by,
                                )?;
                            }
                            _ => {}
                        }

                        continue;
                    };

                    let it = select_clause_element
                        .segments()
                        .iter()
                        .find(|it| it.is_code())
                        .unwrap();
                    let out = extracted_tables.get_source(it.raw().as_ref())?;

                    match out {
                        Source::None => {}
                        Source::CountStar => _ = count_stars.insert(alias.raw().to_string()),
                        UnderlyingColumn(out) => {
                            columns.insert(alias.raw().to_string(), out);
                        }
                        UnderlyingColumnWithOperation(_, _) => todo!(),
                    }

                    continue;
                }

                for mut segment in select_clause_element.segments() {
                    match segment.get_type() {
                        SyntaxKind::ColumnReference => {
                            let out = extracted_tables.get_source(segment.raw().as_ref())?;

                            if segment.raw().contains('.') {
                                segment = segment.segments().last().unwrap();
                            }

                            match out {
                                Source::None => {}
                                Source::CountStar => todo!(),
                                UnderlyingColumn(out) => {
                                    columns.insert(segment.raw().to_string(), out);
                                }
                                UnderlyingColumnWithOperation(out, operation) => {
                                    operated_on.insert(segment.raw().to_string(), (operation, out));
                                }
                            }
                        }
                        SyntaxKind::WildcardExpression => {}
                        _ => todo!("{} {:?}", segment.raw(), segment.get_type()),
                    }
                }
            }

            ExtractedSelect::Extracted {
                mapped: columns,
                unmapped: unnamed,
                count_stars,
                operated_on,
            }
        }
    } else {
        return Err("Not a select".to_string());
    };

    if let Some(withs) = with_extracted {
        withs
            .iter()
            .try_fold(main_extracted, |acc, (with_alias, with)| {
                match acc {
                    ExtractedSelect::Extracted {
                        mapped,
                        unmapped,
                        count_stars,
                        operated_on: _,
                    } => {
                        let extracted_mapped = mapped;
                        let extracted_unmapped = unmapped;
                        let extracted_count_stars = count_stars;
                        let operated_on: OperatedOn = HashMap::new();

                        let mut columns_map: HashMap<String, (String, String)> =
                            extracted_mapped.clone();
                        let mut count_stars_set: HashSet<String> = extracted_count_stars.clone();

                        for (name, extracted) in &withs {
                            match extracted {
                                ExtractedSelect::Star(star) => {
                                    for (x, _) in columns_map.values_mut() {
                                        if x == with_alias {
                                            *x = star.clone();
                                        }
                                    }
                                }
                                ExtractedSelect::Extracted {
                                    mapped,
                                    count_stars,
                                    ..
                                } => {
                                    let sub_columns = mapped.clone();
                                    let sub_columns_star = count_stars.clone();

                                    let mut sub_column_star_found: HashSet<String> = HashSet::new();
                                    for (_, (int_table, int_key)) in columns_map.iter_mut() {
                                        if int_table == name {
                                            if sub_columns_star.contains(int_key) {
                                                sub_column_star_found.insert(int_key.clone());
                                            } else {
                                                let (target_table, target_key) =
                                                    sub_columns.get(int_key).ok_or(format!(
                                                        "Could not find {int_key} in {sub_columns:?}"
                                                    ))?;
                                                int_table.clone_from(target_table);
                                                int_key.clone_from(target_key);
                                            }
                                        }
                                    }

                                    // TODO This can definitely be cleaned up
                                    for found in sub_column_star_found {
                                        columns_map.remove(found.as_str());
                                        count_stars_set.insert(found.clone());
                                    }

                                    // TODO deal with alias
                                }
                            }
                        }

                        Ok(ExtractedSelect::Extracted {
                            mapped: columns_map,
                            unmapped: extracted_unmapped,
                            count_stars: count_stars_set,
                            operated_on,
                        })
                    }
                    ExtractedSelect::Star(value) => {
                        if *with_alias == value {
                            Ok(with.clone())
                        } else {
                            Ok(ExtractedSelect::Star(value))
                        }
                    }
                }
            })

    // TODO Need to fix this
    } else {
        Ok(main_extracted)
    }
}

fn avg_min_max_function_parser(
    operated_on: &mut OperatedOn,
    extracted_tables: &mut Vec<Extracted>,
    alias: String,
    first_arg: ErasedSegment,
    operation: Operation,
    group_by: bool,
) -> Result<(), String> {
    if let Some(column_reference) =
        first_arg.child(const { &SyntaxSet::single(SyntaxKind::ColumnReference) })
    {
        let out = extracted_tables.get_source(column_reference.raw().as_ref())?;
        if let UnderlyingColumn((source, column)) = out {
            operated_on.insert(alias.clone(), ((operation, group_by), (source, column)));
        }
    }

    Ok(())
}

fn join_operator(join: ErasedSegment) -> String {
    let keywords = Segments::new(join, None).children(None).select(
        Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Keyword)),
        Some(|it: &ErasedSegment| {
            !it.raw().eq_ignore_ascii_case("join") && it.is_type(SyntaxKind::Keyword)
                || !it.is_code()
        }),
        None,
        None,
    );

    let keywords = keywords
        .iter()
        .map(|it| it.raw().to_string())
        .collect::<Vec<_>>()
        .join(" ");

    if keywords.is_empty() {
        return "inner".to_string();
    }

    if keywords.eq_ignore_ascii_case("left") {
        return "left outer".to_string();
    }

    keywords
}

fn extract_extracted_from_joins(
    joins: Vec<ErasedSegment>,
    dialect: &Dialect,
) -> Result<Vec<Extracted>, String> {
    let mut extracted = vec![];

    if joins
        .iter()
        .cloned()
        .all(|it| join_operator(it).eq_ignore_ascii_case("left outer"))
    {
        for join in joins {
            let relation = join
                .child(const { &SyntaxSet::single(SyntaxKind::FromExpressionElement) })
                .unwrap();
            let extracted_table = extract_table(&relation, dialect)?;
            match extracted_table {
                Extracted::AliasedStar(alias, _) => {
                    extracted.push(Extracted::ZeroMap(alias));
                }
                Extracted::AliasedSelect(alias, _) => {
                    extracted.push(Extracted::ZeroMap(alias));
                }
                _ => {
                    return Err(
                        "Cannot support left outer joins with non-aliased tables".to_string()
                    );
                }
            }
        }
        return Ok(extracted);
    }

    for join in joins {
        let join_operator = join_operator(join.clone());
        if join_operator.eq_ignore_ascii_case("inner") {
            let relation = join
                .child(const { &SyntaxSet::single(SyntaxKind::FromExpressionElement) })
                .unwrap();
            let extracted_table = extract_table(&relation, dialect)?;
            extracted.push(extracted_table);
        } else {
            return Err(format!("Cannot support joins yet: {:?}", join.raw()));
        }
    }

    Ok(extracted)
}

#[derive(Clone, Debug)]
enum Extracted {
    // A star mapping is essentially a select * to a particular reference.
    // WITH SELECT * FROM table AS alias
    Star(String),
    // An aliased star mapping is a select * to a particular reference with a particular alias.
    // WITH SELECT * FROM table AS alias SELECT * FROM alias AS alias2
    AliasedStar(String, String),
    // A Select mapping is a select of a reference but with particular columns selected and ones
    // they refer to. WITH SELECT column1, column2 FROM table AS alias SELECT column1, column2
    // FROM alias
    Select(ExtractedSelect),
    // An Aliased Select is a mapping to a reference with select but with a particular alias. The
    // first string is the alias. WITH SELECT column1, column2 FROM table AS alias SELECT
    // a.column1, a.column2 FROM alias a
    AliasedSelect(String, ExtractedSelect),
    // ZeroMap is just a placeholder such that left outer joins can be joined on but not used to
    // generate tests. It is just the alias to know the target.
    ZeroMap(String),
}

pub trait ExtractedFunc: Sized {
    fn count_non_aliased(&self) -> (usize, Self);

    fn find_alias_and_target(
        &self,
        alias: &str,
        target: &str,
    ) -> Result<Option<(String, String)>, String>;

    fn get_source(&self, value: &str) -> Result<Source, String>;
}

#[derive(Clone, Debug)]
pub enum Source {
    None,
    CountStar,
    UnderlyingColumn((String, String)),
    UnderlyingColumnWithOperation((String, String), (Operation, bool)),
}

impl ExtractedFunc for Vec<Extracted> {
    fn count_non_aliased(&self) -> (usize, Self) {
        let mut non_aliased: Self = Vec::new();
        for extract in self {
            match extract {
                Extracted::Star(_) => non_aliased.push(extract.clone()),
                Extracted::Select(_) => non_aliased.push(extract.clone()),
                _ => {}
            };
        }
        (non_aliased.len(), non_aliased)
    }

    fn find_alias_and_target(
        &self,
        alias: &str,
        target: &str,
    ) -> Result<Option<(String, String)>, String> {
        for extract in self {
            match extract {
                Extracted::AliasedSelect(a, reference) => match reference {
                    // TODO Figure this out
                    ExtractedSelect::Star(_) => return Err("Not yet implemented".to_string()),
                    ExtractedSelect::Extracted { mapped, .. } => {
                        if a == alias {
                            if let Some(value) = mapped.get(target) {
                                return Ok(Some(value.clone()));
                            }
                            return Err(format!(
                                "In find alias, could not find {target} in {reference:?}"
                            ));
                        }
                    }
                },
                Extracted::AliasedStar(a, reference) => {
                    if a == alias {
                        return Ok(Some((reference.clone(), target.to_string())));
                    }
                }
                Extracted::ZeroMap(a) => {
                    if a == alias {
                        return Ok(None);
                    }
                }
                _ => {}
            }
        }
        Err(format!("Could not find {target} in {self:?}"))
    }

    fn get_source(&self, value: &str) -> Result<Source, String> {
        let sections: Vec<&str> = value.split('.').collect();
        let (non_aliased_count, non_aliased) = self.count_non_aliased();
        match (&self[..], &sections[..]) {
            ([self_part], [_]) => {
                match self_part {
                    Extracted::Star(s) => Ok(UnderlyingColumn((s.to_string(), value.to_string()))),
                    Extracted::Select(m) => match m {
                        // TODO Figure this out
                        ExtractedSelect::Star(_) => Err("Not yet implemented".to_string()),
                        ExtractedSelect::Extracted {
                            mapped,
                            count_stars,
                            operated_on,
                            ..
                        } => {
                            if let Some(v) = mapped.get(value) {
                                Ok(UnderlyingColumn(v.clone()))
                            } else if count_stars.get(value).is_some() {
                                Ok(Source::CountStar)
                            } else if let Some((operation, (source, column))) =
                                operated_on.get(value)
                            {
                                Ok(UnderlyingColumnWithOperation(
                                    (source.clone(), column.clone()),
                                    operation.clone(),
                                ))
                            } else {
                                Err(format!("In getsource, Could not find {value} in {m:?}"))
                            }
                        }
                    },
                    // TODO Add Test so that this gets covered by count star as well
                    Extracted::AliasedSelect(_, select) => match select {
                        ExtractedSelect::Star(_) => Err("Not yet implemented".to_string()),
                        ExtractedSelect::Extracted { mapped, .. } => {
                            let underlying_column = mapped
                                .get(value)
                                .ok_or(format!("In mapped, could not find {value} in {select:?}"))?
                                .clone();
                            Ok(UnderlyingColumn(underlying_column))
                        }
                    },
                    Extracted::AliasedStar(_, s) => {
                        Ok(UnderlyingColumn((s.to_string(), value.to_string())))
                    }
                    _ => Err("Should have been caught by valid".to_string()),
                }
            }
            (_, [section]) => {
                if non_aliased_count == 1 {
                    match &non_aliased.first() {
                        Some(Extracted::Star(s)) => {
                            Ok(UnderlyingColumn((s.to_string(), section.to_string())))
                        }
                        Some(Extracted::Select(select)) => match select {
                            // TODO Figure this out
                            ExtractedSelect::Star(_) => Err("Not yet implemented".to_string()),
                            ExtractedSelect::Extracted { mapped, .. } => {
                                let v = mapped
                                    .get(value)
                                    .ok_or(format!(
                                        "In mapped, could not find {value} in {select:?}"
                                    ))?
                                    .clone();
                                Ok(UnderlyingColumn(v))
                            }
                        },
                        _ => Err("Should have been caught by valid".to_string()),
                    }
                } else {
                    Err("Not yet implemented".to_string())
                }
            }
            (_, [alias, key]) => {
                if non_aliased_count > 1 {
                    return Err(
                        "Impossible to match where non_aliased count is greater than 1".to_string(),
                    );
                }
                match self.find_alias_and_target(alias, key) {
                    Ok(Some(a)) => Ok(UnderlyingColumn(a)),
                    Ok(None) => Ok(Source::None),
                    Err(e) => Err(e),
                }
            }
            _ => Err("Not yet implemented".to_string()),
        }
    }
}

fn extract_table(table_factor: &ErasedSegment, dialect: &Dialect) -> Result<Extracted, String> {
    if let Some(table_expression) =
        table_factor.child(const { &SyntaxSet::single(SyntaxKind::TableExpression) })
    {
        if table_expression.segments()[0].get_type() == SyntaxKind::Bracketed {
            let subquery = table_expression
                .recursive_crawl(
                    const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                )
                .into_iter()
                .next()
                .unwrap();

            let subquery = Query::from_segment(&subquery, dialect, None);
            let selected = extract_select(&subquery)?;

            if let Some(alias) = table_factor
                .segments()
                .iter()
                .find(|it| it.is_type(SyntaxKind::AliasExpression))
            {
                let raw_segments = alias.get_raw_segments();
                let alias = raw_segments.iter().rev().find(|it| it.is_code()).unwrap();
                return Ok(Extracted::AliasedSelect(alias.raw().to_string(), selected));
            }

            return Ok(Extracted::Select(selected));
        }
        let name = table_expression.segments()[0].raw().to_string();

        if let Some(alias) = table_factor
            .recursive_crawl(
                const { &SyntaxSet::single(SyntaxKind::AliasExpression) },
                true,
                &SyntaxSet::EMPTY,
                false,
            )
            .into_iter()
            .next()
        {
            let raw_segments = alias.get_raw_segments();
            let alias = raw_segments.iter().rev().find(|it| it.is_code()).unwrap();
            return Ok(Extracted::AliasedStar(alias.raw().to_string(), name));
        }

        return Ok(Extracted::Star(name));
    }
    todo!()
}

fn fn_args(root: ErasedSegment) -> Vec<ErasedSegment> {
    Segments::new(root, None)
        .children(Some(|it: &ErasedSegment| {
            !it.is_meta()
                && !matches!(
                    it.get_type(),
                    SyntaxKind::StartBracket
                        | SyntaxKind::EndBracket
                        | SyntaxKind::Whitespace
                        | SyntaxKind::Newline
                        | SyntaxKind::CastingOperator
                        | SyntaxKind::Comma
                        | SyntaxKind::Keyword
                )
        }))
        .into_vec()
}

#[cfg(test)]
mod tests {
    use sqruff_lib_dialects::ansi;

    use super::*;

    struct TestStructure {
        sql: &'static str,
        tests: Vec<Test>,
        tests_want: HashMap<Test, InferenceReason>,
    }

    #[test]
    fn test_infer_tests() {
        let test_model_path = "test_path".to_string();

        let tests: Vec<TestStructure> = vec![
            TestStructure {
                sql: "SELECT a FROM q.model_b;",
                tests: vec![
                    Test::NotNull(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                    }),
                    Test::Unique(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                    }),
                    Test::AcceptedValues(AcceptedValuesTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                        values: ["1", "2"].iter().map(|s| s.to_string()).collect(),
                    }),
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                        value: "1".to_string(),
                    }),
                    Test::LessThanOrEqual(ComparisonTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                        value: "1".to_string(),
                    }),
                    Test::GreaterThan(ComparisonTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                        value: "1".to_string(),
                    }),
                    Test::LessThan(ComparisonTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                        value: "1".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                        })),
                    ),
                    (
                        Test::Unique(StandardTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::Unique(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                        })),
                    ),
                    (
                        Test::AcceptedValues(AcceptedValuesTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                            values: ["1", "2"].iter().map(|s| s.to_string()).collect(),
                        }),
                        InferenceReason::UnderlyingTest(Test::AcceptedValues(AcceptedValuesTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                            values: ["1", "2"].iter().map(|s| s.to_string()).collect(),
                        })),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                            value: "1".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::GreaterThanOrEqual(ComparisonTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                            value: "1".to_string(),
                        })),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                            value: "1".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::LessThanOrEqual(ComparisonTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                            value: "1".to_string(),
                        })),
                    ),
                    (
                        Test::GreaterThan(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                            value: "1".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::GreaterThan(ComparisonTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                            value: "1".to_string(),
                        })),
                    ),
                    (
                        Test::LessThan(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                            value: "1".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::LessThan(ComparisonTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                            value: "1".to_string(),
                        })),
                    ),
                ]),
            },
            TestStructure {
                sql: "SELECT a FROM model_b;",
                tests: vec![
                    Test::NotNull(StandardTest {
                        path: "model_b".to_string(),
                        column: "a".to_string(),
                    }),
                    Test::AcceptedValues(AcceptedValuesTest {
                        path: "model_b".to_string(),
                        column: "a".to_string(),
                        values: ["1", "2"].iter().map(|s| s.to_string()).collect(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                            path: "model_b".to_string(),
                            column: "a".to_string(),
                        })),
                    ),
                    (
                        Test::AcceptedValues(AcceptedValuesTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                            values: ["1", "2"].iter().map(|s| s.to_string()).collect(),
                        }),
                        InferenceReason::UnderlyingTest(Test::AcceptedValues(AcceptedValuesTest {
                            path: "model_b".to_string(),
                            column: "a".to_string(),
                            values: ["1", "2"].iter().map(|s| s.to_string()).collect(),
                        })),
                    ),
                ]),
            },
            TestStructure {
                sql: "SELECT employee_id,
                        strftime('%Y-%m', shift_date) AS shift_month,
                         COUNT(*)                     AS total_shifts
                FROM q.model_b
                GROUP BY employee_id, shift_month;",
                tests: vec![
                    Test::NotNull(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "employee_id".to_string(),
                    }),
                    Test::Unique(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "employee_id".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "employee_id".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "employee_id".to_string(),
                        })),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "total_shifts".to_string(),
                            value: "0".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "total_shifts".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                    (
                        Test::Unique(StandardTest {
                            path: test_model_path.clone(),
                            column: "employee_id".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::Unique(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "employee_id".to_string(),
                        })),
                    ),
                ]),
            },
            TestStructure {
                sql: "WITH
              min_shifts AS (
                SELECT
                  employee_id,
                  MIN(shift_start) AS shift_start
                FROM
                  q.model_b
                GROUP BY
                  employee_id
              )
            SELECT
              x.employee_id AS employee_id,
              x.shift_start AS shift_start,
              x.shift_end AS shift_end
            FROM
              q.model_b x
              INNER JOIN min_shifts y ON y.employee_id = x.employee_id
              AND y.shift_start = x.shift_start
            GROUP BY
              x.employee_id,
              x.shift_start
            ",
                tests: vec![
                    Test::NotNull(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "employee_id".to_string(),
                    }),
                    Test::Unique(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "employee_id".to_string(),
                    }),
                    Test::Relationship(RelationshipTest {
                        path: "q.model_b".to_string(),
                        column: "employee_id".to_string(),
                        target_reference: "q.model_c".to_string(),
                        target_column: "employee_id".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.to_string(),
                            column: "employee_id".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "employee_id".to_string(),
                        })),
                    ),
                    (
                        Test::Unique(StandardTest {
                            path: test_model_path.to_string(),
                            column: "employee_id".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::Unique(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "employee_id".to_string(),
                        })),
                    ),
                    (
                        Test::Relationship(RelationshipTest {
                            path: test_model_path.to_string(),
                            column: "employee_id".to_string(),
                            target_reference: "q.model_c".to_string(),
                            target_column: "employee_id".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::Relationship(RelationshipTest {
                            path: "q.model_b".to_string(),
                            column: "employee_id".to_string(),
                            target_reference: "q.model_c".to_string(),
                            target_column: "employee_id".to_string(),
                        })),
                    ),
                ]),
            },
            TestStructure {
                sql: "SELECT a AS b FROM q.model_b;",
                tests: vec![
                    Test::NotNull(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                    }),
                    Test::Unique(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.to_string(),
                            column: "b".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                        })),
                    ),
                    (
                        Test::Unique(StandardTest {
                            path: test_model_path.to_string(),
                            column: "b".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::Unique(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                        })),
                    ),
                ]),
            },
            TestStructure {
                sql: "SELECT * FROM q.model_b;",
                tests: vec![
                    Test::NotNull(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                    }),
                    Test::Unique(StandardTest {
                        path: "q.model_b".to_string(),
                        column: "a".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                        })),
                    ),
                    (
                        Test::Unique(StandardTest {
                            path: test_model_path.clone(),
                            column: "a".to_string(),
                        }),
                        InferenceReason::UnderlyingTest(Test::Unique(StandardTest {
                            path: "q.model_b".to_string(),
                            column: "a".to_string(),
                        })),
                    ),
                ]),
            },
        ];

        let dialect = ansi::dialect();
        let parser = Parser::from(&dialect);

        for test in tests {
            let inferred_tests = infer_tests(
                &parser,
                "test_path",
                test.sql,
                &test.tests.into_iter().collect(),
            )
            .unwrap();

            assert_eq!(
                test.tests_want.len(),
                inferred_tests.len(),
                "SQL: {}",
                test.sql
            );
            assert_eq!(test.tests_want, inferred_tests, "SQL: {}", test.sql);
        }
    }

    #[test]
    fn test_infer_tests_count_star() {
        let test_model_path = "test_path".to_string();

        let tests: Vec<TestStructure> = vec![
            TestStructure {
                sql: "
            SELECT COUNT(*) AS count
            FROM q.stg_employees e",
                tests: vec![],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "count".to_string(),
                            value: "0".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "count".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                ]),
            },
            TestStructure {
                sql: "
            SELECT count(*) AS count
            FROM q.stg_employees e",
                tests: vec![],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "count".to_string(),
                            value: "0".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "count".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                ]),
            },
            TestStructure {
                sql: "
            SELECT Count(*) AS count
            FROM q.stg_employees e",
                tests: vec![],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "count".to_string(),
                            value: "0".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "count".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                ]),
            },
            TestStructure {
                sql: "
            WITH cte AS (SELECT count(*) AS count FROM q.stg_employees e) SELECT count FROM cte",
                tests: vec![],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "count".to_string(),
                            value: "0".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                    (
                        Test::NotNull(StandardTest {
                            path: test_model_path.clone(),
                            column: "count".to_string(),
                        }),
                        InferenceReason::CountStar,
                    ),
                ]),
            },
        ];

        let dialect = ansi::dialect();
        let parser = Parser::from(&dialect);

        for test in tests {
            let inferred_tests = infer_tests(
                &parser,
                "test_path",
                test.sql,
                &test.tests.into_iter().collect(),
            )
            .unwrap();

            assert_eq!(inferred_tests.len(), test.tests_want.len());
            assert_eq!(test.tests_want, inferred_tests);
        }
    }

    #[test]
    fn test_infer_tests_avg_min_max() {
        let test_model_path = "test_path".to_string();

        // TODO ADD Tests for GROUP BY

        let tests: Vec<TestStructure> = vec![
            // lower case plus not null/greater than or equal and less than or equal;
            TestStructure {
                sql: "
SELECT
    avg(employee_age) AS average,
    min(employee_age) AS minimum,
    max(employee_age) AS maximum
FROM q.stg_employees",
                tests: vec![
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "18".to_string(),
                    }),
                    Test::LessThanOrEqual(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "100".to_string(),
                    }),
                    Test::NotNull(StandardTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                ]),
            },
            // less/greater than rather than grater/less than or Equal
            TestStructure {
                sql: "
SELECT
    avg(employee_age) AS average,
    min(employee_age) AS minimum,
    max(employee_age) AS maximum
FROM q.stg_employees",
                tests: vec![
                    Test::GreaterThan(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "18".to_string(),
                    }),
                    Test::LessThan(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "100".to_string(),
                    }),
                    Test::NotNull(StandardTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThan(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThan(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::LessThan(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThan(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::GreaterThan(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThan(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::LessThan(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThan(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::GreaterThan(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThan(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                    (
                        Test::LessThan(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThan(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                ]),
            },
            // capitalised casing
            TestStructure {
                sql: "
            SELECT
                AVG(employee_age) AS average,
                MIN(employee_age) AS minimum,
                MAX(employee_age) AS maximum
            FROM q.stg_employees",
                tests: vec![
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "18".to_string(),
                    }),
                    Test::LessThanOrEqual(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "100".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                ]),
            },
            // TODO Add subquery with star and subquery with just as is.
            // subquery
            TestStructure {
                sql: "
SELECT average, minimum, maximum FROM (SELECT
    AVG(e.employee_age) AS average,
    MIN(e.employee_age) AS minimum,
    MAX(e.employee_age) AS maximum
FROM q.stg_employees e)",
                tests: vec![
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "18".to_string(),
                    }),
                    Test::LessThanOrEqual(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "100".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                ]),
            },
            // with statement
            // TODO Add with star and subquery with just as is.
            TestStructure {
                sql: "
WITH data AS (SELECT
    AVG(e.employee_age) AS average,
    MIN(e.employee_age) AS minimum,
    MAX(e.employee_age) AS maximum
FROM q.stg_employees e) SELECT * FROM data",
                tests: vec![
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "18".to_string(),
                    }),
                    Test::LessThanOrEqual(ComparisonTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_age".to_string(),
                        value: "100".to_string(),
                    }),
                ],
                tests_want: HashMap::from([
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "average".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Avg, false),
                        ),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "minimum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Min, false),
                        ),
                    ),
                    (
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "18".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::GreaterThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "18".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                    (
                        Test::LessThanOrEqual(ComparisonTest {
                            path: test_model_path.clone(),
                            column: "maximum".to_string(),
                            value: "100".to_string(),
                        }),
                        InferenceReason::UnderlyingTestWithOperation(
                            Test::LessThanOrEqual(ComparisonTest {
                                path: "q.stg_employees".to_string(),
                                column: "employee_age".to_string(),
                                value: "100".to_string(),
                            }),
                            (Operation::Max, false),
                        ),
                    ),
                ]),
            },
        ];

        let dialect = ansi::dialect();
        let parser = Parser::from(&dialect);

        for test in tests {
            let inferred_tests = infer_tests(
                &parser,
                "test_path",
                test.sql,
                &test.tests.into_iter().collect(),
            )
            .unwrap();

            assert_eq!(inferred_tests.len(), test.tests_want.len(), "{}", test.sql);
            assert_eq!(test.tests_want, inferred_tests, "{}", test.sql);
        }
    }

    #[test]
    fn test_infer_tests_avg_min_max_with_group_by() {
        let test_model_path = "test_path".to_string();

        let tests: Vec<TestStructure> = vec![TestStructure {
            sql: "
SELECT
    avg(employee_age) AS average,
    min(employee_age) AS minimum,
    max(employee_age) AS maximum,
    department
FROM q.stg_employees
GROUP BY department",
            tests: vec![
                Test::GreaterThanOrEqual(ComparisonTest {
                    path: "q.stg_employees".to_string(),
                    column: "employee_age".to_string(),
                    value: "18".to_string(),
                }),
                Test::LessThanOrEqual(ComparisonTest {
                    path: "q.stg_employees".to_string(),
                    column: "employee_age".to_string(),
                    value: "100".to_string(),
                }),
                Test::NotNull(StandardTest {
                    path: "q.stg_employees".to_string(),
                    column: "employee_age".to_string(),
                }),
            ],
            tests_want: HashMap::from([
                (
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: test_model_path.clone(),
                        column: "average".to_string(),
                        value: "18".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                            value: "18".to_string(),
                        }),
                        (Operation::Avg, true),
                    ),
                ),
                (
                    Test::LessThanOrEqual(ComparisonTest {
                        path: test_model_path.clone(),
                        column: "average".to_string(),
                        value: "100".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::LessThanOrEqual(ComparisonTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                            value: "100".to_string(),
                        }),
                        (Operation::Avg, true),
                    ),
                ),
                (
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: test_model_path.clone(),
                        column: "minimum".to_string(),
                        value: "18".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                            value: "18".to_string(),
                        }),
                        (Operation::Min, true),
                    ),
                ),
                (
                    Test::LessThanOrEqual(ComparisonTest {
                        path: test_model_path.clone(),
                        column: "minimum".to_string(),
                        value: "100".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::LessThanOrEqual(ComparisonTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                            value: "100".to_string(),
                        }),
                        (Operation::Min, true),
                    ),
                ),
                (
                    Test::GreaterThanOrEqual(ComparisonTest {
                        path: test_model_path.clone(),
                        column: "maximum".to_string(),
                        value: "18".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::GreaterThanOrEqual(ComparisonTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                            value: "18".to_string(),
                        }),
                        (Operation::Max, true),
                    ),
                ),
                (
                    Test::LessThanOrEqual(ComparisonTest {
                        path: test_model_path.clone(),
                        column: "maximum".to_string(),
                        value: "100".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::LessThanOrEqual(ComparisonTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                            value: "100".to_string(),
                        }),
                        (Operation::Max, true),
                    ),
                ),
                (
                    Test::NotNull(StandardTest {
                        path: test_model_path.clone(),
                        column: "maximum".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::NotNull(StandardTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                        }),
                        (Operation::Max, true),
                    ),
                ),
                (
                    Test::NotNull(StandardTest {
                        path: test_model_path.clone(),
                        column: "average".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::NotNull(StandardTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                        }),
                        (Operation::Avg, true),
                    ),
                ),
                (
                    Test::NotNull(StandardTest {
                        path: test_model_path.clone(),
                        column: "minimum".to_string(),
                    }),
                    InferenceReason::UnderlyingTestWithOperation(
                        Test::NotNull(StandardTest {
                            path: "q.stg_employees".to_string(),
                            column: "employee_age".to_string(),
                        }),
                        (Operation::Min, true),
                    ),
                ),
            ]),
        }];

        let dialect = ansi::dialect();
        let parser = Parser::from(&dialect);

        for test in tests {
            let inferred_tests = infer_tests(
                &parser,
                "test_path",
                test.sql,
                &test.tests.into_iter().collect(),
            )
            .unwrap();

            assert_eq!(inferred_tests.len(), test.tests_want.len(), "{}", test.sql);
            assert_eq!(test.tests_want, inferred_tests, "{}", test.sql);
        }
    }

    #[test]
    fn test_infer_tests_multiple_left_join() {
        let test_model_path = "test_path".to_string();

        let tests: Vec<TestStructure> = vec![TestStructure {
            sql: "
SELECT e.employee_id,
       e.first_name,
       e.last_name AS last_name,
       sf.shift_start AS first_shift,
       sl.shift_start AS last_shift
FROM q.stg_employees e
LEFT JOIN q.shift_first sf
    ON e.employee_id = sf.employee_id
LEFT JOIN q.shift_last sl
    ON e.employee_id = sl.employee_id",
            tests: vec![
                Test::NotNull(StandardTest {
                    path: "q.stg_employees".to_string(),
                    column: "employee_id".to_string(),
                }),
                Test::Unique(StandardTest {
                    path: "q.stg_employees".to_string(),
                    column: "employee_id".to_string(),
                }),
                Test::NotNull(StandardTest {
                    path: "q.stg_employees".to_string(),
                    column: "first_name".to_string(),
                }),
                Test::NotNull(StandardTest {
                    path: "q.stg_employees".to_string(),
                    column: "last_name".to_string(),
                }),
            ],
            tests_want: HashMap::from([
                (
                    Test::NotNull(StandardTest {
                        path: test_model_path.to_string(),
                        column: "employee_id".to_string(),
                    }),
                    InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_id".to_string(),
                    })),
                ),
                (
                    Test::NotNull(StandardTest {
                        path: test_model_path.to_string(),
                        column: "first_name".to_string(),
                    }),
                    InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                        path: "q.stg_employees".to_string(),
                        column: "first_name".to_string(),
                    })),
                ),
                (
                    Test::NotNull(StandardTest {
                        path: test_model_path.to_string(),
                        column: "last_name".to_string(),
                    }),
                    InferenceReason::UnderlyingTest(Test::NotNull(StandardTest {
                        path: "q.stg_employees".to_string(),
                        column: "last_name".to_string(),
                    })),
                ),
                (
                    Test::Unique(StandardTest {
                        path: test_model_path.to_string(),
                        column: "employee_id".to_string(),
                    }),
                    InferenceReason::UnderlyingTest(Test::Unique(StandardTest {
                        path: "q.stg_employees".to_string(),
                        column: "employee_id".to_string(),
                    })),
                ),
            ]),
        }];

        let dialect = ansi::dialect();
        let parser = Parser::from(&dialect);

        for test in tests {
            let inferred_tests = infer_tests(
                &parser,
                "test_path",
                test.sql,
                &test.tests.into_iter().collect(),
            )
            .unwrap();

            assert_eq!(inferred_tests.len(), test.tests_want.len());
            assert_eq!(test.tests_want, inferred_tests);
        }
    }

    // TODO Need to test mixes of stars to not stars and vice-versa
    #[test]
    fn test_get_column_with_star() {
        let tests = &[
            ("SELECT * FROM q.model_a", "q.model_a"),
            ("SELECT * FROM (SELECT * FROM q.model_a)", "q.model_a"),
            (
                "WITH intermediary AS (SELECT * FROM q.table_a) SELECT * FROM intermediary",
                "q.table_a",
            ),
            (
                "WITH intermediary AS (SELECT * FROM q.model_a), ignored as (SELECT * FROM \
                 q_table_b)  SELECT * FROM intermediary",
                "q.model_a",
            ),
            (
                "WITH ignored AS (SELECT * FROM q_model_b), intermediary as (SELECT * FROM \
                 q.model_a)  SELECT * FROM intermediary",
                "q.model_a",
            ),
            (
                "WITH intermediary_1 AS (SELECT * FROM q.table_a), intermediary_2 as (SELECT * \
                 FROM intermediary_1)  SELECT * FROM intermediary_2",
                "q.table_a",
            ),
        ];

        let dialect = ansi::dialect();
        let parser = Parser::from(&dialect);

        for (sql, want) in tests {
            let selected = get_column_with_source(&parser, sql).unwrap();
            assert_eq!(ExtractedSelect::Star(want.to_string()), selected, "{}", sql)
        }
    }

    #[test]
    fn test_get_column_with_source() {
        let tests: Vec<(&str, Vec<(&str, (&str, &str))>, Vec<&str>, Vec<&str>)> = vec![
            (
                "SELECT a FROM q.model_a",
                vec![("a", ("q.model_a", "a"))],
                vec![],
                vec![],
            ),
            (
                "SELECT a AS b FROM q.model_a",
                vec![("b", ("q.model_a", "a"))],
                vec![],
                vec![],
            ),
            (
                "SELECT a, b AS c FROM q.model_a",
                vec![("a", ("q.model_a", "a")), ("c", ("q.model_a", "b"))],
                vec![],
                vec![],
            ),
            (
                "SELECT b.a FROM q.model_a b",
                vec![("a", ("q.model_a", "a"))],
                vec![],
                vec![],
            ),
            (
                "SELECT a FROM q.model_a b",
                vec![("a", ("q.model_a", "a"))],
                vec![],
                vec![],
            ),
            (
                "SELECT b.c AS a FROM q.model_a b",
                vec![("a", ("q.model_a", "c"))],
                vec![],
                vec![],
            ),
            (
                "SELECT alias_a.a AS c, alias_b.b FROM q.model_a alias_a INNER JOIN q.model_b \
                 alias_b ON alias_a.a=alias_b.a;",
                vec![("c", ("q.model_a", "a")), ("b", ("q.model_b", "b"))],
                vec![],
                vec![],
            ),
            (
                "SELECT alias_a.a AS c, alias_b.b FROM q.model_a alias_a JOIN q.model_b alias_b \
                 ON alias_a.a=alias_b.a;",
                vec![("c", ("q.model_a", "a")), ("b", ("q.model_b", "b"))],
                vec![],
                vec![],
            ),
            (
                "WITH a AS (SELECT b, c AS d FROM q.table_c) SELECT b, d AS e FROM a",
                vec![("b", ("q.table_c", "b")), ("e", ("q.table_c", "c"))],
                vec![],
                vec![],
            ),
            (
                "WITH a AS (SELECT b FROM q.table_c), q AS (SELECT b AS v FROM a) SELECT v AS e \
                 FROM q",
                vec![("e", ("q.table_c", "b"))],
                vec![],
                vec![],
            ),
            (
                "SELECT a FROM (SELECT a FROM q.table_a)",
                vec![("a", ("q.table_a", "a"))],
                vec![],
                vec![],
            ),
            (
                "SELECT c FROM (SELECT a AS c FROM q.table_a)",
                vec![("c", ("q.table_a", "a"))],
                vec![],
                vec![],
            ),
            (
                "SELECT a AS b FROM (SELECT c AS a FROM q.table_a)",
                vec![("b", ("q.table_a", "c"))],
                vec![],
                vec![],
            ),
            (
                "SELECT e.a AS b, g.b FROM (SELECT d.c AS a FROM q.table_a d) e INNER JOIN \
                 (SELECT b FROM q.table_b) g ON g.b=e.a",
                vec![("b", ("q.table_a", "c")), ("b", ("q.table_b", "b"))],
                vec![],
                vec![],
            ),
            (
                "SELECT COUNT(*) AS b FROM q.table_a",
                vec![],
                vec![],
                vec!["b"],
            ),
            (
                "SELECT count(*) AS b FROM (SELECT a.b AS c FROM q.table_a a)",
                vec![],
                vec![],
                vec!["b"],
            ),
            (
                "SELECT c AS b FROM (SELECT count(*) AS c FROM q.table_a a)",
                vec![],
                vec![],
                vec!["b"],
            ),
            (
                "WITH b AS (SELECT count(*) AS c FROM q.table_a a) SELECT c FROM b",
                vec![],
                vec![],
                vec!["c"],
            ),
            (
                "WITH bc AS (SELECT b AS c FROM q.table_a a) SELECT * FROM bc",
                vec![("c", ("q.table_a", "b"))],
                vec![],
                vec![],
            ),
            (
                "WITH
base AS (
    SELECT *
    FROM root_table
),
final AS (
    SELECT column_a
    FROM base
)
SELECT * FROM final",
                vec![("column_a", ("root_table", "column_a"))],
                vec![],
                vec![],
            ),
            (
                "With
base as (
    select *
    from root_table
),
final as (
    select column_a AS column_b
    from base
)
select *
from final",
                vec![("column_b", ("root_table", "column_a"))],
                vec![],
                vec![],
            ),
            // TODO Be smarter about type casting
            (
                "SELECT date::date as cost_date FROM q.table_a",
                vec![],
                vec!["cost_date"],
                vec![],
            ),
            // TODO Be smarter about casting, here could do one of
            (
                "SELECT CASE when market != 'THING' or receive_market != 'THING' then 1 when \
                 channel = 'THING' then 0 else 0 end as caq from q.caq",
                vec![],
                vec!["caq"],
                vec![],
            ),
        ];

        let dialect = ansi::dialect();
        let parser = Parser::from(&dialect);

        for (sql, expected_map_entries, expected_not_parseable, expected_count) in tests {
            let selected = get_column_with_source(&parser, sql).unwrap();

            let mut expected_map: HashMap<String, (String, String)> = HashMap::new();
            for (k, (v1, v2)) in expected_map_entries {
                expected_map.insert(k.to_string(), (v1.to_string(), v2.to_string()));
            }

            match selected {
                ExtractedSelect::Extracted {
                    mapped,
                    count_stars,
                    unmapped,
                    operated_on,
                } => {
                    assert_eq!(mapped, expected_map, "mapped sql: {}", sql);
                    assert_eq!(unmapped, expected_not_parseable, "unmapped sql: {}", sql);
                    assert_eq!(operated_on, HashMap::new(), "operated on: {}", sql);
                    assert_eq!(
                        count_stars,
                        expected_count.into_iter().map(|s| s.to_string()).collect(),
                        "stars sql: {}",
                        sql
                    );
                }
                ExtractedSelect::Star(_) => panic!("not right"),
            }
        }
    }
}
