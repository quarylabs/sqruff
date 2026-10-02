//! Separate ANSI lex and parse benchmarks over TPC-H and TPC-DS queries.
//! Fetch the pinned SQL corpus first; see `benches/TPC.md`.

mod tpc_support;

use criterion::{Criterion, criterion_group, criterion_main};
use sqruff_lib::core::test_functions::fresh_ansi_dialect;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use std::hint::black_box;
use std::time::Duration;
use tpc_support::{TPCDS_QUERIES, TPCH_QUERIES, load_suite};

fn benchmark_suite(c: &mut Criterion, suite: &str, count: u32) {
    let sqls = load_suite(suite, count);
    let dialect = fresh_ansi_dialect();
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);
    let tables = Tables::default();
    // Tokenize before timing parsing, so parse measurements exclude lexing.
    let token_sets: Vec<_> = sqls
        .iter()
        .map(|(_, sql)| {
            let (tokens, errors) = lexer.lex(&tables, sql.as_str());
            assert!(errors.is_empty(), "Could not lex {suite} query");
            tokens
        })
        .collect();

    let mut group = c.benchmark_group(suite);
    group.sample_size(30).warm_up_time(Duration::from_secs(3));
    group.bench_function(format!("lex_{suite}_{count}"), |b| {
        b.iter(|| {
            for (_, sql) in &sqls {
                black_box(lexer.lex(&tables, black_box(sql.as_str())));
            }
        });
    });
    group.bench_function(format!("parse_{suite}_{count}"), |b| {
        b.iter(|| {
            for tokens in &token_sets {
                black_box(
                    parser
                        .parse(&tables, black_box(tokens))
                        .expect("TPC parse failed"),
                );
            }
        });
    });
    group.finish();
}

fn tpch(c: &mut Criterion) {
    benchmark_suite(c, "tpc-h", TPCH_QUERIES);
}

fn tpcds(c: &mut Criterion) {
    benchmark_suite(c, "tpc-ds", TPCDS_QUERIES);
}

criterion_group!(tpc_benches, tpch, tpcds);
criterion_main!(tpc_benches);
