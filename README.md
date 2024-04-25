<p align="center">
  <a href="https://quary.dev">
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

### Installation

#### macOS

You can use [brew](https://brew.sh/) to install sqruff easily on macOS. 

```bash
brew install quarylabs/quary/sqruff
```

#### Linux

Using `bash`:

```bash
curl -fsSL https://raw.githubusercontent.com/quarylabs/sqruff/main/install.sh | bash
```

#### For other platforms

Either download the binary from the [releases page](https://github.com/quarylabs.sqruff/releases) or compile it yourself and with cargo with the following commands.

```bash
rustup override set nightly
cargo install sqruff
sqruff --help
```

### Usage

#### Linting

To lint a SQL file or set of files, run the following command:

```bash
sqruff lint <file>
sqruff lint <file1> <file2> <file3>
sqruff lint <directory>
```

#### Fixing

To fix a single or or set of files, run the following command:

```bash
sqruff fix <file/paths/directory>
```

#### Help

To get help on the available commands and options, run the following command:

```bash
sqruff --help
```

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines on how to contribute.

### Releasing

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

## Community

Join the sqruff community on [GitHub Discussions](https://github.com/quarylabs/sqruff/discussions) to ask questions, suggest features, or share your projects.
