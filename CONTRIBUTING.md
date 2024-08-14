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
cargo publish -p sqruff-sqruff-lib
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

