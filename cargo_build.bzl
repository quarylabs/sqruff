"""Custom Bazel rules for running Cargo as an opaque tool in a hermetic sandbox.

These rules install Rust from scratch using rustup, then run cargo commands
with vendored dependencies for hermetic, reproducible builds.

Also provides rules for pre-caching a Python virtual environment via uv,
so that cargo tests requiring Python (e.g. PyO3/maturin) can run fully
sandboxed without network access at test time.
"""

def _cargo_vendor_impl(ctx):
    """Vendors cargo dependencies and installs Rust toolchain with components."""
    vendor_dir = ctx.actions.declare_directory("vendor")
    cargo_config = ctx.actions.declare_file(".cargo/config.toml")
    toolchain_dir = ctx.actions.declare_directory("rust_toolchain")

    manifest_files = ctx.files.manifests

    # Build list of source paths
    src_paths = " ".join([f.path for f in manifest_files])

    # Build component install command
    install_components = ""
    if ctx.attr.components:
        install_components = "rustup component add " + " ".join(ctx.attr.components)

    script_content = """#!/bin/bash
set -euo pipefail

# Save the original directory for outputs
EXEC_ROOT="$PWD"

# Set up isolated cargo/rustup home that we'll preserve
export CARGO_HOME="$EXEC_ROOT/{toolchain_out}/cargo"
export RUSTUP_HOME="$EXEC_ROOT/{toolchain_out}/rustup"
mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"

# Download and install rustup (no-modify-path prevents writing to ~/.profile)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain {rust_version} --profile minimal

# Add cargo to PATH
export PATH="$CARGO_HOME/bin:$PATH"

# Install additional components if specified
{install_components}

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
        install_components = install_components,
        srcs = src_paths,
        vendor_out = vendor_dir.path,
        config_out = cargo_config.path,
        toolchain_out = toolchain_dir.path,
    )

    script_file = ctx.actions.declare_file(ctx.label.name + "_vendor.sh")
    ctx.actions.write(script_file, script_content, is_executable = True)

    ctx.actions.run(
        inputs = manifest_files,
        outputs = [vendor_dir, cargo_config, toolchain_dir],
        executable = script_file,
        mnemonic = "CargoVendor",
        progress_message = "Vendoring cargo dependencies (installing Rust %s)" % ctx.attr.rust_version,
        execution_requirements = {
            "requires-network": "1",
        },
    )

    return [DefaultInfo(files = depset([vendor_dir, cargo_config, toolchain_dir]))]

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
        "components": attr.string_list(
            default = [],
            doc = "Additional rustup components to install (e.g., clippy, rustfmt)",
        ),
    },
)

# Provider to carry vendored dependencies and toolchain
CargoVendorInfo = provider(
    doc = "Carries vendored cargo dependencies and Rust toolchain",
    fields = {
        "vendor_dir": "Directory containing vendored sources",
        "cargo_config": "File with cargo config pointing to vendor",
        "toolchain_dir": "Directory containing Rust toolchain installation",
    },
)

def _cargo_vendor_provider_impl(ctx):
    """Wrapper that provides CargoVendorInfo from cargo_vendor output."""
    vendor_files = ctx.attr.vendor[DefaultInfo].files.to_list()
    vendor_dir = None
    toolchain_dir = None
    cargo_config = None

    for f in vendor_files:
        if f.is_directory:
            if f.basename == "vendor":
                vendor_dir = f
            elif f.basename == "rust_toolchain":
                toolchain_dir = f
        else:
            cargo_config = f

    return [
        DefaultInfo(files = depset(vendor_files)),
        CargoVendorInfo(
            vendor_dir = vendor_dir,
            cargo_config = cargo_config,
            toolchain_dir = toolchain_dir,
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

# Provider to carry a pre-built Python environment with dependencies
PythonVenvInfo = provider(
    doc = "Carries a self-contained Python installation with pre-installed packages",
    fields = {
        "venv_dir": "Directory containing the Python installation with packages",
    },
)

def _python_venv_impl(ctx):
    """Copies the Bazel-managed Python runtime and installs dev dependencies into it.

    This runs as a cacheable action with network access. The output is a
    self-contained Python installation (bin/, lib/, include/) with all dev
    dependencies installed. Uses --prefix to guarantee scripts (like maturin)
    are installed to bin/ regardless of the base Python's sysconfig scheme.
    """
    venv_dir = ctx.actions.declare_directory("python_venv")

    uv_file = ctx.files.uv[0]
    src_files = ctx.files.srcs

    src_paths = " ".join([f.path for f in src_files])

    # Find the python3 binary from the runtime files
    python_files = ctx.files.python
    python_bin = None
    for f in python_files:
        if f.basename == "python3" and f.short_path.endswith("/bin/python3"):
            python_bin = f
            break
    if not python_bin:
        for f in python_files:
            if "bin/python3" in f.short_path and f.basename.startswith("python3"):
                python_bin = f
                break
    if not python_bin:
        fail("Could not find python3 binary in python runtime files")

    script_content = """#!/bin/bash
set -euo pipefail

EXEC_ROOT="$PWD"
UV_BIN="$EXEC_ROOT/{uv_path}"
VENV_OUT="$EXEC_ROOT/{venv_out}"
PYTHON_BIN="$EXEC_ROOT/{python_path}"

WORK_DIR=$(mktemp -d)
export UV_CACHE_DIR="$WORK_DIR/.uv-cache"

# Copy the entire Python runtime (bin, lib, include) into a writable directory
PYTHON_ROOT="$(dirname "$(dirname "$PYTHON_BIN")")"
cp -r "$PYTHON_ROOT/." "$WORK_DIR/python/"

# Install pyproject.toml for dependency resolution
for src in {srcs}; do
    mkdir -p "$WORK_DIR/$(dirname "$src")"
    cp "$src" "$WORK_DIR/$src"
done
cd "$WORK_DIR"

# Install all dev dependencies into the Python installation.
# Use --prefix to force scripts (maturin, pytest, etc.) into <prefix>/bin/
# regardless of the Python's sysconfig scheme (which may differ for
# standalone/relocated Python builds).
"$UV_BIN" pip install --python "$WORK_DIR/python/bin/python3" \
    --prefix "$WORK_DIR/python" \
    -r pyproject.toml --extra dev

# Verify key tools were installed to the expected location
test -f "$WORK_DIR/python/bin/maturin" || \
    { echo "ERROR: maturin not found in python/bin/"; ls -la "$WORK_DIR/python/bin/"; exit 1; }

# Copy the complete Python installation to the Bazel output directory
cp -r "$WORK_DIR/python/." "$VENV_OUT/"

echo "Python environment created at $VENV_OUT"
""".format(
        uv_path = uv_file.path,
        python_path = python_bin.path,
        srcs = src_paths,
        venv_out = venv_dir.path,
    )

    script_file = ctx.actions.declare_file(ctx.label.name + "_venv.sh")
    ctx.actions.write(script_file, script_content, is_executable = True)

    ctx.actions.run(
        inputs = src_files + [uv_file] + python_files,
        outputs = [venv_dir],
        executable = script_file,
        mnemonic = "PythonVenv",
        progress_message = "Creating Python environment with dependencies",
        execution_requirements = {
            "requires-network": "1",
        },
    )

    return [DefaultInfo(files = depset([venv_dir]))]

python_venv = rule(
    implementation = _python_venv_impl,
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "pyproject.toml for dependency resolution",
        ),
        "uv": attr.label_list(
            allow_files = True,
            mandatory = True,
            doc = "uv binary target",
        ),
        "python": attr.label_list(
            allow_files = True,
            mandatory = True,
            doc = "Full Python runtime files from rules_python (e.g. @python_3_12//:files)",
        ),
    },
)

def _python_venv_provider_impl(ctx):
    """Wrapper that provides PythonVenvInfo from python_venv output."""
    venv_files = ctx.attr.venv[DefaultInfo].files.to_list()
    venv_dir = None

    for f in venv_files:
        if f.is_directory and f.basename == "python_venv":
            venv_dir = f

    return [
        DefaultInfo(files = depset(venv_files)),
        PythonVenvInfo(
            venv_dir = venv_dir,
        ),
    ]

python_venv_provider = rule(
    implementation = _python_venv_provider_impl,
    attrs = {
        "venv": attr.label(
            mandatory = True,
            doc = "python_venv target",
        ),
    },
)

def _cargo_test_impl(ctx):
    """Runs cargo commands as a Bazel test using pre-installed toolchain.

    Optionally sets up a Python virtual environment (from python_venv_provider)
    so that PyO3/maturin-based tests can run fully sandboxed.
    """
    vendor_info = ctx.attr.vendor[CargoVendorInfo]

    all_inputs = ctx.files.srcs + ctx.files.tools + [vendor_info.vendor_dir, vendor_info.cargo_config, vendor_info.toolchain_dir]

    # Add python venv inputs if provided
    python_setup = ""
    if ctx.attr.python_venv:
        python_info = ctx.attr.python_venv[PythonVenvInfo]
        all_inputs = all_inputs + [python_info.venv_dir]

        python_setup = """
# Set up Python environment from pre-cached installation
PYTHON_ENV_SRC="$RUNFILES/_main/{venv_dir}"

# Copy the Python installation to the writable work directory
cp -r "$PYTHON_ENV_SRC" "$WORK_DIR/.python"

# Set up environment for PyO3 and maturin
export PYO3_PYTHON="$WORK_DIR/.python/bin/python3"
export VIRTUAL_ENV="$WORK_DIR/.python"
export PYTHONHOME="$WORK_DIR/.python"
export PATH="$WORK_DIR/.python/bin:$PATH"

# Set library paths for both compile-time linking and runtime linking
export LIBRARY_PATH="$WORK_DIR/.python/lib:${{LIBRARY_PATH:-}}"
export LD_LIBRARY_PATH="$WORK_DIR/.python/lib:${{LD_LIBRARY_PATH:-}}"
export DYLD_LIBRARY_PATH="$WORK_DIR/.python/lib:${{DYLD_LIBRARY_PATH:-}}"

# Create .venv symlink for tests that expect it at the project root
ln -s .python "$WORK_DIR/.venv"

echo "Python ready: $($PYO3_PYTHON --version)"
""".format(
            venv_dir = python_info.venv_dir.short_path,
        )

    # Generate symlink commands for additional cargo subcommand tools
    tool_setup = ""
    if ctx.files.tools:
        tool_setup = "TOOL_BINDIR=$(mktemp -d)\n"
        for f in ctx.files.tools:
            tool_setup += 'ln -s "$RUNFILES/_main/{path}" "$TOOL_BINDIR/{name}"\n'.format(
                path = f.short_path,
                name = f.basename,
            )
        tool_setup += 'export PATH="$TOOL_BINDIR:$PATH"'

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
TOOLCHAIN_DIR="$RUNFILES/_main/{toolchain_dir}"

# Use the pre-installed toolchain from cargo_vendor
export CARGO_HOME="$TOOLCHAIN_DIR/cargo"
export RUSTUP_HOME="$TOOLCHAIN_DIR/rustup"
export PATH="$CARGO_HOME/bin:$PATH"

{tool_setup}

WORK_DIR=$(mktemp -d)

# Copy source files into writable tree (heredoc handles spaces in filenames)
while IFS= read -r src; do
    [ -z "$src" ] && continue
    SRC_PATH="$RUNFILES/_main/$src"
    mkdir -p "$WORK_DIR/$(dirname "$src")"
    cp -r "$SRC_PATH" "$WORK_DIR/$src"
done << 'SRCS_EOF'
{srcs}
SRCS_EOF

# Copy vendored dependencies
cp -r "$VENDOR_DIR" "$WORK_DIR/vendor"

# Copy cargo config
mkdir -p "$WORK_DIR/.cargo"
cp "$CARGO_CONFIG" "$WORK_DIR/.cargo/config.toml"

cd "$WORK_DIR"

export CARGO_TARGET_DIR="$WORK_DIR/target"

{python_setup}

# Run the user script
{script}
""".format(
        vendor_dir = vendor_info.vendor_dir.short_path,
        cargo_config = vendor_info.cargo_config.short_path,
        toolchain_dir = vendor_info.toolchain_dir.short_path,
        srcs = "\n".join([f.short_path for f in ctx.files.srcs]),
        tool_setup = tool_setup,
        python_setup = python_setup,
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
            doc = "cargo_vendor_provider target with pre-installed toolchain",
        ),
        "python_venv": attr.label(
            default = None,
            providers = [PythonVenvInfo],
            doc = "Optional python_venv_provider target for PyO3/maturin tests",
        ),
        "tools": attr.label_list(
            allow_files = True,
            default = [],
            doc = "Additional cargo subcommand binaries (e.g. cargo-hack) to symlink into PATH",
        ),
        "script": attr.string(mandatory = True),
    },
)
