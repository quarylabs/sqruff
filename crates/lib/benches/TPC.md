# TPC SQL benchmarks

This ports SQLFluff #7923's TPC-H (22 queries) and TPC-DS (99 queries) lex/parse
benchmark coverage to sqruff's existing Criterion setup. The SQL is fetched
from Apache Doris commit
[`3a2d9d55f1e8e2d74187179ef89c36c8562815fd`](https://github.com/apache/doris/tree/3a2d9d55f1e8e2d74187179ef89c36c8562815fd),
not committed to this repository. The fetch is explicit; normal builds and CI
never access the network for these fixtures.

From the repository root:

```sh
bash .hacking/scripts/fetch_tpc_queries.sh
cargo bench -p sqruff-lib --bench tpc
cargo run -p sqruff-lib --example time_tpc --release
```

The default fixture cache is `target/tpc-fixtures`. To use another directory,
pass it to the fetch script and set `SQRUFF_TPC_FIXTURES_DIR` when running the
benchmark or timing example. The timing example reports per-query and suite
parse times; Criterion measures lexing and parsing separately. Query parsing
uses the ANSI dialect, like the upstream baseline. Sqruff's parser does not
expose SQLFluff Rust parser's table-cache/pruning counters, so those
implementation-specific statistics are not reported here.

These queries are representative SQL inputs only. This is not a TPC-compliant
benchmark and makes no performance claims under the TPC specifications.
