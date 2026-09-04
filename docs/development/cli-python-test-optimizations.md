# CLI and Python test optimisation notes

This note records follow-up opportunities discovered after moving the Rust CLI
integration tests to native Bazel targets. It is intentionally an investigation
and implementation plan rather than a completed migration.

## Recommended next change: migrate `cli-python` integration tests

`crates/cli-python` has the same test-list drift and opaque-build problems that
the CLI crate had before its migration:

- Cargo manually lists eleven harnessless integration tests in
  `crates/cli-python/Cargo.toml`.
- Bazel does not have a native Rust or PyO3 target for the crate.
- `//:cargo_workspace_tests` copies a complete Python environment into a fresh
  temporary directory, runs `maturin develop`, and then asks Cargo to compile
  and execute the remaining workspace tests.
- Because that target takes the complete `cargo_srcs` filegroup, a change to
  any Rust crate invalidates the whole opaque Cargo action.

The repository already depends on and registers `rules_rust_pyo3`. Its
`pyo3_extension` rule produces a Python dependency that can be consumed by a
`py_binary` or `py_test`, so no additional Bazel ruleset should be necessary.

A proposed target graph is:

```text
sqruff-lib-python
    -> sqruff-cli-lib-python
        -> pyo3_extension(name = "_lib_name")
            -> py_binary(name = "sqruff-python")
                -> rust_test_suite(name = "cli_python_tests")
```

The integration tests should follow the CLI migration pattern:

1. Remove the explicit `[[test]]` entries from the Cargo manifest.
2. Convert each harnessless `fn main()` entry point into a normal `#[test]`.
3. Add a shared helper that resolves the command from a Bazel-provided
   `SQRUFF_BIN`, falling back to the Cargo/Maturin `.venv/bin/sqruff` path.
4. Use `rust_test_suite(srcs = glob(["tests/*.rs"]))`, so Cargo and Bazel
   discover the same source files automatically.
5. Build the Python extension and command once with Bazel and share it among
   the tests, instead of invoking Maturin inside the workspace test action.
6. Remove `sqruff-cli-python` from `//:cargo_workspace_tests` after both Cargo
   and Bazel test paths pass.

Care is needed when placing `_lib_name.so` in the runfiles tree because Python
imports it as `sqruff._lib_name`. The test extension should use the current
Bazel compilation mode; `pyo3_extension` otherwise defaults to an optimised
build.

## Avoid duplicating the ordinary CLI suite

Seven of the eleven `cli-python` integration tests duplicate tests in
`crates/cli`: `config_not_found`, `configure_rule`, `fix_parse_errors`,
`fix_return_code`, `ui`, `ui_github`, and `ui_json`. Their fixture directories
are also identical apart from newer cases that exist only in the Rust CLI
suite.

The Python extension's public entry point is a small adapter that forwards its
arguments to `sqruff_cli_lib::run_with_args`. Re-running the entire generic CLI
behaviour suite through that adapter offers little extra coverage. A smaller
and clearer Python distribution suite would contain:

- one end-to-end bridge test covering arguments, output, and exit status;
- `ui_with_python`;
- `ui_with_jinja`;
- `library_path`;
- `ui_with_dbt`.

If running every generic test through both entry points is considered valuable,
the test cases and fixtures should be shared and parameterised by the command
path rather than copied between crates.

The Python-specific CLI fixtures use ANSI except for the dbt sample, which uses
DuckDB. After dialects are split into individual crates, the fast Python test
binary therefore needs only ANSI plus DuckDB and its Postgres dependency. A
separate full-registry smoke test should continue to exercise the production
extension.

## Make the pytest matrix more precise

The four Python-version pytest targets have several low-risk opportunities:

- They are all tagged `exclusive`, although every invocation creates and uses
  its own temporary directory. Removing that tag should allow Python 3.10,
  3.11, 3.12, and 3.13 to run concurrently.
- Each target depends on every non-Rust file under `crates/cli-python/tests`.
  The pure Python tests only use `tests/dbt_sample`; changing an unrelated CLI
  golden file currently invalidates the entire Python matrix. Add a dedicated
  `dbt_test_fixtures` filegroup and use that instead.
- `.hacking/scripts/pytest_uv.sh` copies the `crates` runfiles tree solely to
  stabilise dbt paths. The non-dbt tests can run directly from runfiles, while
  the dbt tests alone can retain the temporary copy.
- Coverage is enabled for all four compatibility versions. Coverage can run
  once on the primary version, with the other versions providing compatibility
  checks only.
- Splitting the dbt tests from the pure Python/Jinja tests would let the latter
  use a much smaller environment. Whether dbt itself must be tested on every
  Python version is a policy choice.

## Remove duplicate Python environments

There are currently two independently generated Python 3.12 environments with
the same test dependencies:

- `//:python_venv`, used by the Cargo workspace tests;
- `//:pytest_py312_venv`, used by pytest.

In the local Bazel output they occupy approximately 348 MB and 347 MB
respectively. `python_deps` can wrap `pytest_py312_venv`, eliminating one
networked setup action and one large cached tree. Migrating `cli-python` to a
native PyO3 target may remove the Cargo test environment entirely.

The custom environment rule declares `uv.lock` as an input but installs from
`pyproject.toml` using `uv pip install`; it does not actually resolve from the
lock. The implementation should consume the frozen lock (for example via a
locked export) so the cache key and installed contents describe the same input.

## Finish removing the opaque workspace test

After `cli-python`, the crates still covered by `//:cargo_workspace_tests` are
primarily `lib`, `lib-dialects`, `cli-lib`, `lsp`, and `lib-wasm`. Adding native
unit and integration test targets for those crates would:

- give Bazel per-crate invalidation and parallel scheduling;
- reuse native Bazel compilation outputs;
- avoid rebuilding the workspace from scratch in an isolated Cargo target
  directory;
- eventually allow the enormous opaque Cargo test to be removed from the
  normal Bazel test graph.

`lib/tests/rules.rs`, `lib/tests/templaters.rs`, and
`lib-dialects/tests/dialects.rs` are still manually declared harnessless tests
and are natural follow-ups. The dialect fixture runner could ultimately be
partitioned by dialect, while retaining one registry-completeness test.

## Other dependency-graph opportunities

- `sqruff-cli-lib` unconditionally depends on `sqruff-lsp`, so an LSP-only
  change invalidates both CLI distributions and all their tests. Feature-gating
  the LSP command, with a full production variant and a smaller ordinary-test
  variant, would narrow that graph.
- `.bazelrc` globally selects `--compilation_mode=opt`. A `fastbuild` test
  configuration may reduce cold Rust compile time, with `opt` reserved for
  release and benchmark targets. This needs benchmarking because parser debug
  assertions add work during fixture tests.
- Once `cli-python` has a native Bazel target it can be included in the native
  rustfmt and clippy target lists; it is currently absent from `RUST_TARGETS`.

## Small correctness issues found during the investigation

- `crates/cli-python/Cargo.toml` places `crate-type = ["cdylib"]` under
  `[package]`, which Cargo reports as an unused manifest key. It belongs under
  `[lib]`.
- The root development environment pins Maturin 1.14.0 while
  `crates/cli-python/pyproject.toml` pins 1.14.1. `maturin develop` currently
  emits a warning about the mismatch; the pins should have one source of truth
  or at least a consistency check.

