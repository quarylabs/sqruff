# Contributing

Contributions are welcome! See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines on how to contribute.

## Running locally

The project requires Rust version specified in `rust-toolchain.toml`which you can set up with `rustup`, as well as a Python environment setup as shown below.

### Setting Up Python

You'll also need to set up a python virtualenv and install dependencies as follows:

```bash
virtualenv .venv
source .venv/bin/activate
make python_sync
```

### Running Tests

Once you have that the following runs the test suite.

#### Rust Tests

```bash
make rust_test
```

#### Python Tests

```bash
make python_test
```

#### Python release packaging with Bazel

Build release wheels and a source distribution for the current platform:

```bash
bazel build //:python_dist
```

Artifacts are written to `bazel-bin/python_dist/wheels` and
`bazel-bin/python_dist/sdist`. Check wheel installation, extension imports, and
the installed CLI on Python 3.14, 3.14t, 3.15, and 3.15t:

```bash
bazel test //:python_wheel_check
```

Bazel downloads the Python runtimes and pinned maturin build tools, and caches
the release build separately from the installation tests. The build uses
vendored Cargo dependencies; the wheel installation tests run offline with no
source fallback. Both targets run as part of the normal `//...` CI checks.
The Python release workflow also builds the full platform matrix and publishes
the release artifacts.

## VS Code Setup

Run the following command to set up your VS Code environment using the recommended settings:

```bash
make load_vscode_settings
```

## Releasing

1. Bump the versions in `sqruff-lib/Cargo.toml` and `sqruff/Cargo.toml`.
2. Commit the changes.
3. Push the changes.
4. Tag the commit with the new version
5. Release `sqruff-lib` crate

```bash
cargo publish -p sqruff-lib
```

6. Release `sqruff` crate

```bash
cargo publish -p sqruff
```

## Running extension locally in browser

To run the extension locally, install npm in the `editors/code` directory and run the following commands:

```bash
npm run build:wasm_lsp && npm run compile && npm run run-in-browser
```

## Updating the fixtures for the dialect tests (the yaml files)

One of the big set of tests that exist in the codebase are those to ensure the parsing of the SQL dialects is correct. These tests are stored in `crates/lib/test/fixtures/dialects` and there is a folder for each dialect. In each folder there is an a set of sql files and an accompanying yaml file that contains the expected output of parsing the sql file. To update the yaml files, run the tests with the `UPDATE_EXPECT` environment variable set to `1`.

```bash
env UPDATE_EXPECT=1 cargo test
```
