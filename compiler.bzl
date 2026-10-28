"""Type-check Cargo's target inventory with native Bazel rules and check coverage."""

load("@rules_rust//rust:defs.bzl", "rust_common")

def _all_features_configuration(settings, _attr):
    # Cargo check uses the dev profile: opt-level 0 and debug assertions enabled.
    # fastbuild also omits debug information, matching CARGO_PROFILE_DEV_DEBUG=0.
    # Share this configuration with Clippy so both checks reuse compiled deps.
    flags = settings["@rules_rust//rust/settings:clippy_flags"]
    if not flags or flags[-1] != "-Dwarnings":
        flags = flags + ["-Dwarnings"]
    return {
        "//:clippy_all_features": True,
        "//command_line_option:compilation_mode": "fastbuild",
        # Cargo check validates metadata without generating unused executables.
        "@rules_rust//rust/settings:always_enable_metadata_output_groups": True,
        # Keep warnings fatal even when extract_cargo_lints provides lint files.
        "@rules_rust//rust/settings:clippy_flags": flags,
    }

all_features_transition = transition(
    implementation = _all_features_configuration,
    inputs = ["@rules_rust//rust/settings:clippy_flags"],
    outputs = [
        "//:clippy_all_features",
        "//command_line_option:compilation_mode",
        "@rules_rust//rust/settings:always_enable_metadata_output_groups",
        "@rules_rust//rust/settings:clippy_flags",
    ],
)

_Compilation = provider(fields = ["entries"])

def _inventory_impl(target, ctx):
    if rust_common.crate_info in target:
        crate = target[rust_common.crate_info]
    else:
        crate = target[rust_common.test_crate_info].crate
    return [_Compilation(entries = [{
        "target": str(target.label),
        "root": crate.root.short_path,
        "test": crate.is_test,
        "harness": getattr(ctx.rule.attr, "use_libtest_harness", False),
        "edition": crate.edition,
        "features": ctx.rule.attr.crate_features,
        "compilation_mode": ctx.var["COMPILATION_MODE"],
        "lint_config": bool(getattr(ctx.rule.attr, "lint_config", None)),
    }])]

_inventory = aspect(implementation = _inventory_impl)

def _targets_impl(ctx):
    checks = []
    for target in ctx.attr.deps:
        if OutputGroupInfo not in target or not hasattr(target[OutputGroupInfo], "build_metadata"):
            fail("Expected compiler metadata for {}".format(target.label))
        metadata = target[OutputGroupInfo].build_metadata
        if not metadata.to_list():
            fail("Empty compiler metadata for {}".format(target.label))
        checks.append(metadata)
    return [
        DefaultInfo(files = depset(transitive = checks)),
        _Compilation(entries = [entry for dep in ctx.attr.deps for entry in dep[_Compilation].entries]),
    ]

_targets = rule(
    implementation = _targets_impl,
    attrs = {"deps": attr.label_list(aspects = [_inventory])},
)

def compiler_targets(libraries):
    """Type-check all declared Rust tests, including benchmarks tagged manual."""
    _targets(
        name = "compiler_targets",
        testonly = True,
        deps = libraries + [
            ":" + name
            for name, rule in native.existing_rules().items()
            if rule["kind"] == "rust_test"
        ],
        visibility = ["//visibility:public"],
        tags = ["manual"],
    )
    native.filegroup(
        name = "compiler_sources",
        srcs = ["Cargo.toml"] + native.glob(["**/*.rs"], exclude = ["**/target/**"]),
        visibility = ["//visibility:public"],
    )

def _workspace_compile_impl(ctx):
    inventory = ctx.actions.declare_file(ctx.label.name + ".targets.json")
    ctx.actions.write(inventory, json.encode([
        entry for target in ctx.attr.targets for entry in target[_Compilation].entries
    ]))
    sources = ctx.actions.declare_file(ctx.label.name + ".sources.json")
    ctx.actions.write(sources, json.encode([file.path for file in ctx.files.srcs]))
    marker = ctx.actions.declare_file(ctx.label.name + ".coverage.ok")
    toolchain = ctx.toolchains["@rules_rust//rust:toolchain_type"]
    args = ctx.actions.args()
    args.add_all([inventory, sources, marker, toolchain.cargo, toolchain.rustc])
    ctx.actions.run(
        executable = ctx.executable._guard,
        arguments = [args],
        inputs = ctx.files.srcs + [inventory, sources],
        tools = [toolchain.all_files],
        outputs = [marker],
        mnemonic = "CargoTargetCoverage",
        progress_message = "Checking native compiler coverage against Cargo targets",
    )
    return [DefaultInfo(files = depset(
        [marker],
        transitive = [target[DefaultInfo].files for target in ctx.attr.targets],
    ))]

workspace_compile = rule(
    implementation = _workspace_compile_impl,
    attrs = {
        "targets": attr.label_list(cfg = all_features_transition),
        "srcs": attr.label_list(allow_files = True),
        "_guard": attr.label(default = "//:check_cargo_targets", executable = True, cfg = "exec"),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
    },
    toolchains = ["@rules_rust//rust:toolchain_type"],
)
