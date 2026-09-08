"""Native Rust test compilation with an isolated Python runtime at execution."""

load("@rules_rust//rust:defs.bzl", "rust_test")
load("@rules_shell//shell:sh_test.bzl", "sh_test")

def python_rust_test(name, fixtures = [], extension = False, **kwargs):
    """Build once with rules_rust; stage only runtime files for each test."""
    rust_test(
        name = name + "_binary",
        tags = ["manual"],
        visibility = ["//visibility:private"],
        **kwargs
    )
    data = fixtures + [
        ":" + name + "_binary",
        "//:native_test_python",
        "//crates/cli-python:python_runtime_srcs",
    ]
    env = {
        "SQRUFF_NATIVE_TEST_BINARY": "$(rlocationpath :" + name + "_binary)",
        "PYTHON_RUNTIME": "$(rlocationpath //:native_test_python)",
        "TEST_CRATE": native.package_name(),
    }
    if extension:
        data.append("//crates/cli-python:sqruff-extension")
        env["SQRUFF_EXTENSION"] = "$(rlocationpath //crates/cli-python:sqruff-extension)"
    sh_test(
        name = name,
        srcs = ["//:.hacking/scripts/python_rust_test.sh"],
        data = data,
        env = env,
        size = "medium",
        visibility = ["//visibility:public"],
    )
