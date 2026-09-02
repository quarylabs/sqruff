"""Linter aspects declared with aspect_rules_lint.

Linters have to be declared as Bazel aspects because aspect attributes may only
be `bool`, `int` or `string` - the factory functions below close over the
label-typed arguments. See https://bazel.build/extending/aspects.

Usage:
  bazel test //:keep_sorted_check      # fails on unsorted blocks
  bazel build --config=lint //...      # reports violations across the repo
"""

load("@aspect_rules_lint//lint:keep_sorted.bzl", "lint_keep_sorted_aspect")
load("@aspect_rules_lint//lint:lint_test.bzl", "lint_test")

# keep-sorted (https://github.com/google/keep-sorted) is a language-agnostic
# linter downloaded as a platform-specific release binary by //:tools.bzl.
keep_sorted = lint_keep_sorted_aspect(
    binary = Label("@keep_sorted//:keep-sorted"),
)

keep_sorted_test = lint_test(aspect = keep_sorted)
