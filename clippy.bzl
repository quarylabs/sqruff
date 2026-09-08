"""Cargo lint policy and an all-features configuration for native Clippy."""

load("@rules_rust//cargo:defs.bzl", "extract_cargo_lints")
load("@rules_rust//rust:defs.bzl", "rust_clippy_aspect")

_Features = provider(fields = ["entries"])

def _features_impl(target, ctx):
    return [_Features(entries = [{
        "target": str(target.label),
        "manifest": target.label.package + "/Cargo.toml",
        "features": ctx.rule.attr.crate_features,
    }])]

_features = aspect(implementation = _features_impl)

def _clippy_targets_impl(ctx):
    return [
        DefaultInfo(files = depset(transitive = [
            target[OutputGroupInfo].clippy_checks
            for target in ctx.attr.deps
        ])),
        _Features(entries = [entry for target in ctx.attr.deps for entry in target[_Features].entries]),
    ]

clippy_targets = rule(
    implementation = _clippy_targets_impl,
    attrs = {"deps": attr.label_list(aspects = [rust_clippy_aspect, _features])},
)

def cargo_lints():
    """Read this package's lints, including inheritance from the workspace."""
    extract_cargo_lints(
        name = "cargo_lints",
        manifest = "Cargo.toml",
        workspace = "//:Cargo.toml",
    )

def clippy_config(normal, all_features):
    """Select attributes for the all-features lint configuration only."""
    return select({
        "//:clippy_all_features_enabled": all_features,
        "//conditions:default": normal,
    })

def _flag_impl(_ctx):
    return []

clippy_feature_flag = rule(
    implementation = _flag_impl,
    build_setting = config.bool(flag = True),
)

def _clippy_transition_impl(settings, _attr):
    return {
        "//:clippy_all_features": True,
        # Keep warnings fatal even when extract_cargo_lints provides lint files.
        "@rules_rust//rust/settings:clippy_flags": settings["@rules_rust//rust/settings:clippy_flags"] + ["-Dwarnings"],
    }

_clippy_transition = transition(
    implementation = _clippy_transition_impl,
    inputs = ["@rules_rust//rust/settings:clippy_flags"],
    outputs = ["//:clippy_all_features", "@rules_rust//rust/settings:clippy_flags"],
)

def _workspace_clippy_impl(ctx):
    check = ctx.attr.check[0]
    inventory = ctx.actions.declare_file(ctx.label.name + ".features.json")
    ctx.actions.write(inventory, json.encode(check[_Features].entries))
    marker = ctx.actions.declare_file(ctx.label.name + ".features.ok")
    manifests = [file for file in ctx.files.manifests if file.basename == "Cargo.toml"]
    args = ctx.actions.args()
    args.add(inventory)
    args.add(marker)
    args.add_all(manifests)
    ctx.actions.run(
        executable = ctx.executable._feature_guard,
        arguments = [args],
        inputs = manifests + [inventory],
        outputs = [marker],
        mnemonic = "ClippyFeatures",
        progress_message = "Checking Clippy feature coverage against Cargo.toml",
    )
    return [DefaultInfo(files = depset([marker], transitive = [check[DefaultInfo].files]))]

workspace_clippy = rule(
    implementation = _workspace_clippy_impl,
    attrs = {
        "check": attr.label(mandatory = True, cfg = _clippy_transition),
        "manifests": attr.label_list(allow_files = True, mandatory = True),
        "_feature_guard": attr.label(
            default = "//:check_clippy_features",
            executable = True,
            cfg = "exec",
        ),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
    },
)
