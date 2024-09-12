# Contributing

Contributions are welcome! See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines on how to contribute.

## Running locally

The project shouldn't require anything more than the `rust-toolchain.toml` version of Rust specified which you can set up with `rustup`. Once you have that the following runs the test suite. 

```bash
cargo test
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

5. Release `sqruff` crate

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
