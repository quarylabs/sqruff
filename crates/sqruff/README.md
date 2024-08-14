<p align="center">
  <a href="https://quary.dev/">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://utfs.io/f/30765a8e-3dd9-4dc3-b905-11de822e71e4-yajpew.png">
      <img src="https://utfs.io/f/30765a8e-3dd9-4dc3-b905-11de822e71e4-yajpew.png" height="128">
    </picture>
    <h1 align="center">sqruff</h1>
  </a>
</p>

<p align="center">
  <a aria-label="Quary logo" href="https://quary.io/">
    <img src="https://img.shields.io/badge/MADE%20BY%20Quary-000000.svg?style=for-the-badge&logo=Quary&labelColor=000">
  </a>
</p>

SQRUFF is an innovative SQL linter and formatter for modern development environments, written in Rust. Key features include:

- **Linting:** Advanced, customizable SQL linting capabilities to ensure query quality.
- **Formatting:** Automated, configurable formatting for SQL code consistency.
- **Portability:** Designed to be easily integrated into various development workflows.

## Getting Started

Visit [sqruff's official site](https://www.quary.dev) to learn more about installation and usage.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines on how to contribute.

## Releasing

1. Bump the versions in `sqruff-lib/Cargo.toml` and `sqruff/Cargo.toml`.
2. Commit the changes.
3. Push the changes.
4. Tag the commit with the new version
5. Release `sqruff-lib` crate

```
cargo publish -p sqruff-lib
```

5. Release `sqruff` crate

```
cargo publish -p sqruff
```

## Installation

```
rustup override set nightly
```

```
cargo install sqruff
```

```
sqruff --help
```

## Community

Join the sqruff community on [GitHub Discussions](https://github.com/quarylabs/sqruff/discussions) to ask questions, suggest features, or share your projects.
