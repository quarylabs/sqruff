//! Per-query and whole-suite ANSI parser timing over the pinned TPC corpus.

#[path = "../benches/tpc_support/mod.rs"]
mod tpc_support;

use sqruff_lib::core::test_functions::fresh_ansi_dialect;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};
use std::hint::black_box;
use std::time::{Duration, Instant};
use tpc_support::{TPCDS_QUERIES, TPCH_QUERIES, load_suite};

const TIMED_RUNS: usize = 5;

fn parse_once(parser: &Parser<'_>, tables: &Tables, tokens: &[ErasedSegment]) -> Duration {
    let start = Instant::now();
    black_box(
        parser
            .parse(tables, black_box(tokens))
            .expect("TPC parse failed"),
    );
    start.elapsed()
}

fn run_suite(suite: &str, count: u32) {
    let sqls = load_suite(suite, count);
    let dialect = fresh_ansi_dialect();
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);
    let tables = Tables::default();
    let token_sets: Vec<_> = sqls
        .iter()
        .map(|(_, sql)| {
            let (tokens, errors) = lexer.lex(&tables, sql.as_str());
            assert!(errors.is_empty(), "Could not lex {suite} query");
            tokens
        })
        .collect();

    println!("{suite}: {count} queries (ANSI)");
    println!(
        "  {:<8} {:>8} {:>8} {:>12}",
        "Query", "Bytes", "Tokens", "Mean (ms)"
    );
    for ((n, sql), tokens) in sqls.iter().zip(&token_sets) {
        parse_once(&parser, &tables, tokens); // warm up
        let mean_ms = (0..TIMED_RUNS)
            .map(|_| parse_once(&parser, &tables, tokens).as_secs_f64() * 1000.0)
            .sum::<f64>()
            / TIMED_RUNS as f64;
        println!(
            "  Q{n:<7} {:>8} {:>8} {:>12.2}",
            sql.len(),
            tokens.len(),
            mean_ms
        );
    }

    for tokens in &token_sets {
        parse_once(&parser, &tables, tokens);
    }
    let suite_ms = (0..TIMED_RUNS)
        .map(|_| {
            let start = Instant::now();
            for tokens in &token_sets {
                black_box(
                    parser
                        .parse(&tables, black_box(tokens))
                        .expect("TPC parse failed"),
                );
            }
            start.elapsed().as_secs_f64() * 1000.0
        })
        .sum::<f64>()
        / TIMED_RUNS as f64;
    println!("  Full suite mean: {suite_ms:.2} ms\n");
}

fn main() {
    run_suite("tpc-h", TPCH_QUERIES);
    run_suite("tpc-ds", TPCDS_QUERIES);
}
