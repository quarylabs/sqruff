"""Custom Bazel rules for running Cargo as an opaque tool in a hermetic sandbox.

These rules install Rust from scratch using rustup, then run cargo commands
with vendored dependencies for hermetic, reproducible builds.
"""

def _cargo_vendor_impl(ctx):
    """Vendors cargo dependencies. Installs Rust toolchain on-the-fly."""
    vendor_dir = ctx.actions.declare_directory("vendor")
    cargo_config = ctx.actions.declare_file(".cargo/config.toml")

    manifest_files = ctx.files.manifests

    # Build list of source paths
    src_paths = " ".join([f.path for f in manifest_files])

    script_content = """#!/bin/bash
set -euo pipefail

# Save the original directory for outputs
EXEC_ROOT="$PWD"

# Set up isolated cargo/rustup home
export CARGO_HOME=$(mktemp -d)
export RUSTUP_HOME=$(mktemp -d)

# Download and install rustup (no-modify-path prevents writing to ~/.profile)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain {rust_version} --profile minimal

# Add cargo to PATH
export PATH="$CARGO_HOME/bin:$PATH"

# Verify installation
cargo --version
rustc --version

WORK_DIR=$(mktemp -d)

# Copy all Cargo.toml / Cargo.lock files preserving structure
for src in {srcs}; do
    mkdir -p "$WORK_DIR/$(dirname "$src")"
    cp "$src" "$WORK_DIR/$src"
done

cd "$WORK_DIR"

cargo vendor "$EXEC_ROOT/{vendor_out}" 2>&1

# Write the cargo config that points to the vendored dir
mkdir -p "$EXEC_ROOT/$(dirname {config_out})"
cat > "$EXEC_ROOT/{config_out}" <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF
""".format(
        rust_version = ctx.attr.rust_version,
        srcs = src_paths,
        vendor_out = vendor_dir.path,
        config_out = cargo_config.path,
    )

    script_file = ctx.actions.declare_file(ctx.label.name + "_vendor.sh")
    ctx.actions.write(script_file, script_content, is_executable = True)

    ctx.actions.run(
        inputs = manifest_files,
        outputs = [vendor_dir, cargo_config],
        executable = script_file,
        mnemonic = "CargoVendor",
        progress_message = "Vendoring cargo dependencies (installing Rust %s)" % ctx.attr.rust_version,
        execution_requirements = {
            "requires-network": "1",
        },
    )

    return [DefaultInfo(files = depset([vendor_dir, cargo_config]))]

cargo_vendor = rule(
    implementation = _cargo_vendor_impl,
    attrs = {
        "manifests": attr.label_list(
            allow_files = True,
            doc = "Cargo.toml and Cargo.lock files",
        ),
        "rust_version": attr.string(
            default = "1.92.0",
            doc = "Rust toolchain version to install",
        ),
    },
)

# Provider to carry vendored dependencies
CargoVendorInfo = provider(
    doc = "Carries vendored cargo dependencies",
    fields = {
        "vendor_dir": "Directory containing vendored sources",
        "cargo_config": "File with cargo config pointing to vendor",
    },
)

def _cargo_vendor_provider_impl(ctx):
    """Wrapper that provides CargoVendorInfo from cargo_vendor output."""
    vendor_files = ctx.attr.vendor[DefaultInfo].files.to_list()
    vendor_dir = [f for f in vendor_files if f.is_directory][0]
    cargo_config = [f for f in vendor_files if not f.is_directory][0]

    return [
        DefaultInfo(files = depset(vendor_files)),
        CargoVendorInfo(
            vendor_dir = vendor_dir,
            cargo_config = cargo_config,
        ),
    ]

cargo_vendor_provider = rule(
    implementation = _cargo_vendor_provider_impl,
    attrs = {
        "vendor": attr.label(
            mandatory = True,
            doc = "cargo_vendor target",
        ),
    },
)

def _cargo_test_impl(ctx):
    """Runs cargo commands as a Bazel test. Installs Rust on-the-fly."""
    vendor_info = ctx.attr.vendor[CargoVendorInfo]

    all_inputs = ctx.files.srcs + [vendor_info.vendor_dir, vendor_info.cargo_config]

    script_content = """#!/bin/bash
set -euo pipefail

# Find the runfiles directory
if [[ -n "${{RUNFILES_DIR:-}}" ]]; then
    RUNFILES="$RUNFILES_DIR"
elif [[ -d "$0.runfiles" ]]; then
    RUNFILES="$0.runfiles"
else
    RUNFILES="$PWD"
fi

VENDOR_DIR="$RUNFILES/_main/{vendor_dir}"
CARGO_CONFIG="$RUNFILES/_main/{cargo_config}"

# Set up isolated cargo/rustup home
export CARGO_HOME=$(mktemp -d)
export RUSTUP_HOME=$(mktemp -d)

# Download and install rustup (no-modify-path prevents writing to ~/.profile)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain {rust_version} --profile minimal

# Add cargo to PATH
export PATH="$CARGO_HOME/bin:$PATH"

WORK_DIR=$(mktemp -d)

# Copy source files into writable tree
for src in {srcs}; do
    SRC_PATH="$RUNFILES/_main/$src"
    mkdir -p "$WORK_DIR/$(dirname "$src")"
    cp -r "$SRC_PATH" "$WORK_DIR/$src"
done

# Copy vendored dependencies
cp -r "$VENDOR_DIR" "$WORK_DIR/vendor"

# Copy cargo config
mkdir -p "$WORK_DIR/.cargo"
cp "$CARGO_CONFIG" "$WORK_DIR/.cargo/config.toml"

cd "$WORK_DIR"

export CARGO_TARGET_DIR="$WORK_DIR/target"

# Run the user script
{script}
""".format(
        rust_version = ctx.attr.rust_version,
        vendor_dir = vendor_info.vendor_dir.short_path,
        cargo_config = vendor_info.cargo_config.short_path,
        srcs = " ".join([f.short_path for f in ctx.files.srcs]),
        script = ctx.attr.script,
    )

    executable = ctx.actions.declare_file(ctx.label.name + "_test.sh")
    ctx.actions.write(executable, script_content, is_executable = True)

    runfiles = ctx.runfiles(files = all_inputs)

    return [DefaultInfo(
        executable = executable,
        runfiles = runfiles,
    )]

cargo_test = rule(
    implementation = _cargo_test_impl,
    test = True,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "vendor": attr.label(
            mandatory = True,
            providers = [CargoVendorInfo],
            doc = "cargo_vendor_provider target",
        ),
        "rust_version": attr.string(
            default = "1.92.0",
            doc = "Rust toolchain version to install",
        ),
        "script": attr.string(mandatory = True),
    },
)
