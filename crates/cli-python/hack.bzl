"""Feature variations for `cargo hack --each-feature` on this crate.

This crate declares no Cargo features, so cargo-hack emits a single plain
`cargo check`. The derived `HACK` map drives the `cargo check` target in
BUILD.bazel and is reconciled one-to-one against cargo-hack by //:hack_reconcile.
"""

load("//:cargo_build.bzl", "each_feature")

MANIFEST = "crates/cli-python/Cargo.toml"

FEATURES = []

HACK = each_feature(FEATURES)
