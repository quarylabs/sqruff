"""Feature variations for `cargo hack --each-feature` on this crate.

This crate declares no Cargo features, so cargo-hack emits a single plain
`cargo check`. The derived `HACK` map drives the `cargo check` target in
BUILD.bazel and is reconciled one-to-one against cargo-hack by //:hack_reconcile.
"""

load("//:cargo_build.bzl", "each_feature")

MANIFEST = "crates/sqlinference/Cargo.toml"

FEATURES = []

HACK = each_feature(FEATURES)

# This crate plus its transitive in-workspace dependencies. Scopes the Bazel
# action inputs (and the in-sandbox workspace) so edits to unrelated crates
# remain cache hits. See cargo_hack_suite.
CLOSURE = [
    "crates/sqlinference",
    "crates/lib-core",
    "crates/lib-dialects",
]
