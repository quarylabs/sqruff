"""Custom Bazel rules for running Cargo as an opaque tool in a hermetic sandbox.

These rules use the Rust toolchain registered with rules_rust, then run Cargo
commands with vendored dependencies for hermetic, reproducible builds.

Also provides rules for pre-caching a Python virtual environment via uv,
so that cargo tests requiring Python (e.g. PyO3/maturin) can run fully
sandboxed without network access at test time.
"""

load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

_RUST_TOOLCHAIN_TYPE = "@rules_rust//rust:toolchain_type"
_RUSTFMT_TOOLCHAIN_TYPE = "@rules_rust//rust/rustfmt:toolchain_type"
_CC_TOOLCHAIN_TYPE = "@bazel_tools//tools/cpp:toolchain_type"

def _cargo_vendor_impl(ctx):
    """Vendors Cargo dependencies with the registered rules_rust toolchain."""
    vendor_dir = ctx.actions.declare_directory("vendor")
    cargo_config = ctx.actions.declare_file(".cargo/config.toml")

    manifest_files = ctx.files.manifests
    rust_toolchain = ctx.toolchains[_RUST_TOOLCHAIN_TYPE]
    toolchain_files = rust_toolchain.all_files.to_list()

    # Build list of source paths
    src_paths = " ".join([f.path for f in manifest_files])

    script_content = """#!/bin/bash
set -euo pipefail

# Save the original directory for outputs
EXEC_ROOT="$PWD"

CARGO_BIN="$EXEC_ROOT/{cargo_path}"
RUSTC_BIN="$EXEC_ROOT/{rustc_path}"
export PATH="$(dirname "$CARGO_BIN"):$PATH"
export CARGO="$CARGO_BIN"
export RUSTC="$RUSTC_BIN"
unset RUSTUP_HOME RUSTUP_TOOLCHAIN

"$CARGO_BIN" --version
"$RUSTC_BIN" --version

WORK_DIR=$(mktemp -d)
export CARGO_HOME="$WORK_DIR/.cargo"

# Copy all Cargo.toml / Cargo.lock files preserving structure
for src in {srcs}; do
    mkdir -p "$WORK_DIR/$(dirname "$src")"
    cp "$src" "$WORK_DIR/$src"
done

# Cargo requires each workspace package to have at least one target before it
# will resolve the workspace. Vendoring only depends on the manifests and
# lockfile, so create disposable targets instead of declaring the real Rust
# sources as inputs to this action.
for manifest in {srcs}; do
    case "$manifest" in
        */Cargo.toml)
            package_dir="$WORK_DIR/$(dirname "$manifest")"
            mkdir -p "$package_dir/src/bin"
            touch \
                "$package_dir/src/lib.rs" \
                "$package_dir/src/main.rs" \
                "$package_dir/src/bin/bench.rs"
            ;;
    esac
done

cd "$WORK_DIR"

"$CARGO_BIN" vendor "$EXEC_ROOT/{vendor_out}" 2>&1

# Write the cargo config that points to the vendored dir
mkdir -p "$EXEC_ROOT/$(dirname {config_out})"
cat > "$EXEC_ROOT/{config_out}" <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF
""".format(
        cargo_path = rust_toolchain.cargo.path,
        rustc_path = rust_toolchain.rustc.path,
        srcs = src_paths,
        vendor_out = vendor_dir.path,
        config_out = cargo_config.path,
    )

    script_file = ctx.actions.declare_file(ctx.label.name + "_vendor.sh")
    ctx.actions.write(script_file, script_content, is_executable = True)

    ctx.actions.run(
        inputs = manifest_files + toolchain_files,
        outputs = [vendor_dir, cargo_config],
        executable = script_file,
        mnemonic = "CargoVendor",
        progress_message = "Vendoring Cargo dependencies with the Bazel-managed Rust toolchain",
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
    },
    toolchains = [_RUST_TOOLCHAIN_TYPE],
)

# Provider to carry vendored dependencies
CargoVendorInfo = provider(
    doc = "Carries vendored Cargo dependencies",
    fields = {
        "vendor_dir": "Directory containing vendored sources",
        "cargo_config": "File with cargo config pointing to vendor",
    },
)

def _cargo_vendor_provider_impl(ctx):
    """Wrapper that provides CargoVendorInfo from cargo_vendor output."""
    vendor_files = ctx.attr.vendor[DefaultInfo].files.to_list()
    vendor_dir = None
    cargo_config = None

    for f in vendor_files:
        if f.is_directory:
            if f.basename == "vendor":
                vendor_dir = f
        else:
            cargo_config = f

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

# Install test and Cargo build dependencies into the Python installation.
# Keep Cargo-only tools such as maturin out of the version-matrix pytest
# environments, while avoiding development tools such as ruff that Bazel
# provides separately.
# Use --prefix to force scripts (maturin, pytest, etc.) into <prefix>/bin/
# regardless of the Python's sysconfig scheme (which may differ for
# standalone/relocated Python builds).
"$UV_BIN" pip install --python "$WORK_DIR/python/bin/python3" \
    --prefix "$WORK_DIR/python" \
    -r pyproject.toml --extra test --extra cargo-test

# Verify key tools were installed to the expected location
test -f "$WORK_DIR/python/bin/maturin" || \
    {{ echo "ERROR: maturin not found in python/bin/"; ls -la "$WORK_DIR/python/bin/"; exit 1; }}

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
    """Wraps a single Python environment directory in PythonVenvInfo."""
    venv_files = ctx.attr.venv[DefaultInfo].files.to_list()
    venv_dirs = [f for f in venv_files if f.is_directory]
    if len(venv_dirs) != 1:
        fail("Expected exactly one Python environment directory")
    venv_dir = venv_dirs[0]

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
            doc = "Target providing one Python environment directory",
        ),
    },
)

def _uv_python_install_impl(ctx):
    """Downloads and caches a Python version via uv.

    Produces a directory containing the installed Python runtime.
    This is cached by Bazel and only re-runs when the Python version changes.
    """
    python_dir = ctx.actions.declare_directory(ctx.label.name + "_python")
    uv_file = ctx.files.uv[0]
    version = ctx.attr.python_version

    script_content = """#!/bin/bash
set -euo pipefail

EXEC_ROOT="$PWD"
UV_BIN="$EXEC_ROOT/{uv_path}"
PYTHON_OUT="$EXEC_ROOT/{python_out}"

WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT
export UV_CACHE_DIR="$WORK_DIR/.uv-cache"
export UV_PYTHON_INSTALL_DIR="$WORK_DIR/.uv-python"

"$UV_BIN" python install "{version}"

# Find the installed Python and copy it to the output directory
PYTHON_BIN=$("$UV_BIN" python find "{version}")
PYTHON_ROOT="$(dirname "$(dirname "$PYTHON_BIN")")"
cp -r "$PYTHON_ROOT/." "$PYTHON_OUT/"

echo "Python {version} installed at $PYTHON_OUT"
""".format(
        uv_path = uv_file.path,
        python_out = python_dir.path,
        version = version,
    )

    script_file = ctx.actions.declare_file(ctx.label.name + "_install.sh")
    ctx.actions.write(script_file, script_content, is_executable = True)

    ctx.actions.run(
        inputs = [uv_file],
        outputs = [python_dir],
        executable = script_file,
        mnemonic = "UvPythonInstall",
        progress_message = "Installing Python %s via uv" % version,
        execution_requirements = {
            "requires-network": "1",
        },
    )

    return [DefaultInfo(files = depset([python_dir]))]

uv_python_install = rule(
    implementation = _uv_python_install_impl,
    attrs = {
        "python_version": attr.string(
            mandatory = True,
            doc = "Python version to install (e.g. '3.12')",
        ),
        "uv": attr.label_list(
            allow_files = True,
            mandatory = True,
            doc = "uv binary target",
        ),
    },
)

def _uv_python_venv_impl(ctx):
    """Syncs project dependencies into a venv using an already-installed Python.

    Takes the output of uv_python_install and project files, produces a venv
    directory with all test dependencies installed. Cached by Bazel.
    """
    venv_dir = ctx.actions.declare_directory(ctx.label.name + "_venv")
    uv_file = ctx.files.uv[0]
    python_dir = ctx.files.python[0]
    src_files = ctx.files.srcs

    src_paths = " ".join([f.path for f in src_files])

    script_content = """#!/bin/bash
set -euo pipefail

EXEC_ROOT="$PWD"
UV_BIN="$EXEC_ROOT/{uv_path}"
PYTHON_DIR="$EXEC_ROOT/{python_dir}"
VENV_OUT="$EXEC_ROOT/{venv_out}"

WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT
export UV_CACHE_DIR="$WORK_DIR/.uv-cache"

# Copy project files for dependency resolution
for src in {srcs}; do
    mkdir -p "$WORK_DIR/$(dirname "$src")"
    cp "$src" "$WORK_DIR/$src"
done

# Copy the Python installation to a writable location
cp -r "$PYTHON_DIR/." "$WORK_DIR/python/"

cd "$WORK_DIR"

# Find the python3 binary
PYTHON_BIN="$WORK_DIR/python/bin/python3"

# Install test dependencies
"$UV_BIN" pip install --python "$PYTHON_BIN" \
    --prefix "$WORK_DIR/python" \
    -r pyproject.toml --extra test

# Copy the complete environment to the Bazel output directory
cp -r "$WORK_DIR/python/." "$VENV_OUT/"

echo "Python venv created at $VENV_OUT"
""".format(
        uv_path = uv_file.path,
        python_dir = python_dir.path,
        srcs = src_paths,
        venv_out = venv_dir.path,
    )

    script_file = ctx.actions.declare_file(ctx.label.name + "_sync.sh")
    ctx.actions.write(script_file, script_content, is_executable = True)

    ctx.actions.run(
        inputs = src_files + [uv_file, python_dir],
        outputs = [venv_dir],
        executable = script_file,
        mnemonic = "UvPythonVenv",
        progress_message = "Syncing Python dependencies for %s" % ctx.label.name,
        execution_requirements = {
            "requires-network": "1",
        },
    )

    return [DefaultInfo(files = depset([venv_dir]))]

uv_python_venv = rule(
    implementation = _uv_python_venv_impl,
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "pyproject.toml and uv.lock for dependency resolution",
        ),
        "python": attr.label_list(
            allow_files = True,
            mandatory = True,
            doc = "uv_python_install target providing the Python runtime",
        ),
        "uv": attr.label_list(
            allow_files = True,
            mandatory = True,
            doc = "uv binary target",
        ),
    },
)

def _cargo_test_impl(ctx):
    """Runs Cargo commands using the registered rules_rust toolchain.

    Optionally sets up a Python virtual environment (from python_venv_provider)
    so that PyO3/maturin-based tests can run fully sandboxed.
    """
    vendor_info = ctx.attr.vendor[CargoVendorInfo]
    rust_toolchain = ctx.toolchains[_RUST_TOOLCHAIN_TYPE]
    rustfmt_toolchain = ctx.toolchains[_RUSTFMT_TOOLCHAIN_TYPE]
    cc_toolchain = find_cc_toolchain(ctx)
    cc_features = cc_common.configure_features(ctx = ctx, cc_toolchain = cc_toolchain)
    cc_path = cc_common.get_tool_for_action(feature_configuration = cc_features, action_name = ACTION_NAMES.c_compile)
    cxx_path = cc_common.get_tool_for_action(feature_configuration = cc_features, action_name = ACTION_NAMES.cpp_compile)
    ar_path = cc_common.get_tool_for_action(feature_configuration = cc_features, action_name = ACTION_NAMES.cpp_link_static_library)
    toolchain_files = depset(
        transitive = [
            rust_toolchain.all_files,
            rustfmt_toolchain.all_files,
            cc_toolchain.all_files,
        ],
    ).to_list()

    all_inputs = ctx.files.srcs + ctx.files.tools + [vendor_info.vendor_dir, vendor_info.cargo_config] + toolchain_files

    # Add python venv inputs if provided
    python_setup = ""
    if ctx.attr.python_venv:
        python_info = ctx.attr.python_venv[PythonVenvInfo]
        all_inputs = all_inputs + [python_info.venv_dir]

        python_setup = """
# Set up Python environment from pre-cached installation
PYTHON_ENV_SRC="$RUNFILES/_main/{venv_dir}"

# Copy the Python installation to the writable work directory
cp -rL "$PYTHON_ENV_SRC" "$WORK_DIR/.python"

# python-build-standalone ships only the versioned shared library. PyO3 asks
# the linker for -lpython3.12, which requires the conventional unversioned
# linker name in the relocated writable copy.
chmod u+w "$WORK_DIR/.python/lib"
ln -sf libpython3.12.so.1.0 "$WORK_DIR/.python/lib/libpython3.12.so"

# Set up environment for PyO3 and maturin
export PYO3_PYTHON="$WORK_DIR/.python/bin/python3"
export VIRTUAL_ENV="$WORK_DIR/.python"
export PYTHONHOME="$WORK_DIR/.python"
export PATH="$WORK_DIR/.python/bin:$PATH"

# Set library paths for both compile-time linking and runtime linking
export LIBRARY_PATH="$WORK_DIR/.python/lib:${{LIBRARY_PATH:-}}"
export LD_LIBRARY_PATH="$WORK_DIR/.python/lib:${{LD_LIBRARY_PATH:-}}"
export DYLD_LIBRARY_PATH="$WORK_DIR/.python/lib:${{DYLD_LIBRARY_PATH:-}}"
# PyO3's relocated standalone interpreter reports its original /install/lib
# prefix. Add the actual copied library directory for final link actions.
export RUSTFLAGS="${{RUSTFLAGS:-}} -L native=$WORK_DIR/.python/lib"

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
CARGO_BIN="$RUNFILES/_main/{cargo_path}"
RUSTC_BIN="$RUNFILES/_main/{rustc_path}"
RUSTFMT_BIN="$RUNFILES/_main/{rustfmt_path}"

# Local C toolchains commonly return absolute system paths, while downloaded
# toolchains return exec-root-relative paths that live under runfiles.
resolve_cc_tool() {{
    case "$1" in
        /*) echo "$1" ;;
        *) echo "$RUNFILES/_main/$1" ;;
    esac
}}
CC_BIN=$(resolve_cc_tool "{cc_path}")
CXX_BIN=$(resolve_cc_tool "{cxx_path}")
AR_BIN=$(resolve_cc_tool "{ar_path}")

# Use the pinned toolchain registered by rules_rust.
CC_BINDIR=$(mktemp -d)
ln -s "$CC_BIN" "$CC_BINDIR/cc"
ln -s "$CXX_BIN" "$CC_BINDIR/c++"
ln -s "$AR_BIN" "$CC_BINDIR/ar"
export PATH="$CC_BINDIR:$(dirname "$CARGO_BIN"):$PATH"
export CARGO="$CARGO_BIN"
export RUSTC="$RUSTC_BIN"
export RUSTFMT="$RUSTFMT_BIN"
export CC="$CC_BIN"
export CXX="$CXX_BIN"
export AR="$AR_BIN"
unset RUSTUP_HOME RUSTUP_TOOLCHAIN

{tool_setup}

WORK_DIR=$(mktemp -d)

# Copy source files into writable tree (heredoc handles spaces in filenames)
while IFS= read -r src; do
    [ -z "$src" ] && continue
    SRC_PATH="$RUNFILES/_main/$src"
    mkdir -p "$WORK_DIR/$(dirname "$src")"
    cp -rL "$SRC_PATH" "$WORK_DIR/$src"
done << 'SRCS_EOF'
{srcs}
SRCS_EOF

# Point cargo at the read-only vendored dependencies in runfiles. The sources are
# immutable, so copying the full vendor tree into every test sandbox is wasted I/O.
mkdir -p "$WORK_DIR/.cargo"
cat > "$WORK_DIR/.cargo/config.toml" <<EOF
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "$VENDOR_DIR"
EOF

cd "$WORK_DIR"

export CARGO_TARGET_DIR="$WORK_DIR/target"

{python_setup}

# Run the user script
{script}
""".format(
        vendor_dir = vendor_info.vendor_dir.short_path,
        cargo_config = vendor_info.cargo_config.short_path,
        cargo_path = rust_toolchain.cargo.short_path,
        rustc_path = rust_toolchain.rustc.short_path,
        rustfmt_path = rustfmt_toolchain.rustfmt.short_path,
        cc_path = cc_path,
        cxx_path = cxx_path,
        ar_path = ar_path,
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

_cargo_attrs = {
    "srcs": attr.label_list(allow_files = True),
    "vendor": attr.label(
        mandatory = True,
        providers = [CargoVendorInfo],
        doc = "cargo_vendor_provider target with vendored dependencies",
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
}

cargo_test = rule(
    implementation = _cargo_test_impl,
    test = True,
    attrs = _cargo_attrs,
    fragments = ["cpp"],
    toolchains = [
        _RUST_TOOLCHAIN_TYPE,
        _RUSTFMT_TOOLCHAIN_TYPE,
        _CC_TOOLCHAIN_TYPE,
    ],
)
