"""Check Rust source filegroups without compiling crates or preparing Cargo."""

_RUSTFMT_TOOLCHAIN = "@rules_rust//rust/rustfmt:toolchain_type"

def _shell_quote(value):
    return "'" + value.replace("'", "'\\''") + "'"

def _rustfmt_sources_test_impl(ctx):
    toolchain = ctx.toolchains[_RUSTFMT_TOOLCHAIN]
    srcs = [src for src in ctx.files.srcs if src.extension == "rs"]
    if not srcs:
        fail("Expected at least one Rust source file")

    # The project's empty config preserves the Cargo wrapper's defaults and
    # prevents rustfmt from discovering configuration outside the sandbox.
    config = ctx.file.config
    args = [toolchain.rustfmt.short_path, "--edition", ctx.attr.edition, "--check", "--config-path", config.short_path]
    args += [src.short_path for src in srcs]
    executable = ctx.actions.declare_file(ctx.label.name + "_test.sh")
    ctx.actions.write(
        executable,
        """#!/usr/bin/env bash
set -euo pipefail
cd "${RUNFILES_DIR:-$0.runfiles}/%s"
exec %s
""" % (ctx.workspace_name, " ".join([_shell_quote(arg) for arg in args])),
        is_executable = True,
    )
    return [DefaultInfo(
        executable = executable,
        runfiles = ctx.runfiles(
            files = srcs + [config],
            transitive_files = toolchain.all_files,
        ),
    )]

rustfmt_sources_test = rule(
    implementation = _rustfmt_sources_test_impl,
    test = True,
    attrs = {
        "srcs": attr.label_list(allow_files = True, mandatory = True),
        "edition": attr.string(default = "2024"),
        "config": attr.label(
            mandatory = True,
            allow_single_file = True,
        ),
    },
    toolchains = [_RUSTFMT_TOOLCHAIN],
)
