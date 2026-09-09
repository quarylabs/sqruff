"""Package-level Cargo-hack checks with a checked, automatically discovered matrix."""

load(":cargo_build.bzl", "cargo_test")

def _inputs_impl(ctx):
    trees = [ctx.actions.declare_directory(ctx.label.name + "/" + package) for package in ctx.attr.packages]
    specification = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(specification, json.encode({
        "sources": [file.path for file in ctx.files.srcs],
        "packages": dict(zip(ctx.attr.packages, [tree.path for tree in trees])),
    }))
    rust = ctx.toolchains["@rules_rust//rust:toolchain_type"]
    args = ctx.actions.args()
    args.add_all([specification, rust.cargo, rust.rustc, ctx.file.hack])
    ctx.actions.run(
        executable = ctx.executable._prepare,
        arguments = [args],
        inputs = ctx.files.srcs + [specification],
        tools = [rust.all_files, ctx.file.hack],
        outputs = trees,
        mnemonic = "CargoHackCoverage",
        progress_message = "Checking Cargo-hack matrix and preparing package sources",
    )
    return [
        DefaultInfo(files = depset(trees)),
        OutputGroupInfo(**{package: depset([tree]) for package, tree in zip(ctx.attr.packages, trees)}),
    ]

_inputs = rule(
    implementation = _inputs_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "packages": attr.string_list(),
        "hack": attr.label(default = "@cargo_hack//:cargo-hack", allow_single_file = True),
        "_prepare": attr.label(default = "//:prepare_cargo_hack", executable = True, cfg = "exec"),
    },
    toolchains = ["@rules_rust//rust:toolchain_type"],
)

def cargo_hack_packages(name, packages, srcs):
    """Generate one real test per discovered package; the matrix lives in Cargo."""
    inputs = name + "_inputs"
    _inputs(name = inputs, packages = packages, srcs = srcs)
    tests = []
    for package in packages:
        suffix = package.removeprefix("crates/")
        source = name + "_sources_" + suffix
        native.filegroup(name = source, srcs = [":" + inputs], output_group = package)
        test = "cargo_hack_" + suffix
        cargo_test(
            name = test,
            size = "large",
            srcs = [":" + source],
            vendor = ":cargo_deps",
            python_venv = ":python_runtime",
            tools = ["@cargo_hack//:cargo-hack"],
            script = """
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_INCREMENTAL=0
# Bound each independent Cargo invocation to avoid multiplying host concurrency.
export CARGO_BUILD_JOBS=2
cd %s/%s
bash run_hack.sh
""" % (inputs, package),
        )
        tests.append(":" + test)
    native.test_suite(name = name, tests = tests)
