"""Feature variations for `cargo hack --each-feature` on this crate.

`FEATURES` lists the features declared in Cargo.toml that cargo-hack checks in
isolation (excluding `codegen-docs`, which is excluded from the sweep). The
derived `HACK` map drives the per-feature `cargo check` targets in BUILD.bazel
and is reconciled one-to-one against cargo-hack by //:hack_reconcile.
"""

load("//:cargo_build.bzl", "each_feature")

MANIFEST = "crates/cli-lib/Cargo.toml"

FEATURES = [
    "parser",
    "python",
]

HACK = each_feature(FEATURES)
