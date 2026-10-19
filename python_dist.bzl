"""Cache maturin release artifacts separately from installed-wheel smoke tests."""

load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load(":cargo_build.bzl", "CargoVendorInfo")

def _python_dist_impl(ctx):
    wheels = ctx.actions.declare_directory(ctx.label.name + "/wheels")
    sdist = ctx.actions.declare_directory(ctx.label.name + "/sdist")
    rust = ctx.toolchains["@rules_rust//rust:toolchain_type"]
    cc = find_cc_toolchain(ctx)
    features = cc_common.configure_features(ctx = ctx, cc_toolchain = cc)
    specification = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(specification, json.encode({
        "sources": [file.path for file in ctx.files.srcs],
        "vendor": ctx.attr.vendor[CargoVendorInfo].vendor_dir.path,
        "tools_python": ctx.file.tools_python.path,
        "interpreters": [file.path for file in ctx.files.interpreters],
        "cargo": rust.cargo.path,
        "rustc": rust.rustc.path,
        "cc": cc_common.get_tool_for_action(feature_configuration = features, action_name = ACTION_NAMES.c_compile),
        "cxx": cc_common.get_tool_for_action(feature_configuration = features, action_name = ACTION_NAMES.cpp_compile),
        "ar": cc_common.get_tool_for_action(feature_configuration = features, action_name = ACTION_NAMES.cpp_link_static_library),
        "wheels": wheels.path,
        "sdist": sdist.path,
    }))
    ctx.actions.run(
        executable = ctx.executable._build,
        arguments = [specification.path],
        inputs = ctx.files.srcs + ctx.files.interpreters + [
            specification,
            ctx.file.tools_python,
            ctx.attr.vendor[CargoVendorInfo].vendor_dir,
        ],
        tools = [rust.all_files, cc.all_files],
        outputs = [wheels, sdist],
        mnemonic = "MaturinDist",
        progress_message = "Building release Python wheels and source distribution",
        env = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
    )
    return [
        DefaultInfo(files = depset([wheels, sdist])),
        OutputGroupInfo(wheels = depset([wheels]), sdist = depset([sdist])),
    ]

python_dist = rule(
    implementation = _python_dist_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "vendor": attr.label(mandatory = True, providers = [CargoVendorInfo]),
        "tools_python": attr.label(mandatory = True, allow_single_file = True),
        "interpreters": attr.label_list(allow_files = True),
        "_build": attr.label(default = "//:build_python_dist", executable = True, cfg = "exec"),
    },
    fragments = ["cpp"],
    toolchains = ["@rules_rust//rust:toolchain_type", "@bazel_tools//tools/cpp:toolchain_type"],
)
