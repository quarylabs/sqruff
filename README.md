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
  <a aria-label="Quary logo" href="https://quary.dev/">
    <img src="https://img.shields.io/badge/MADE%20BY%20Quary-000000.svg?style=for-the-badge&logo=Quary&labelColor=000">
  </a>
  <a aria-label="CodSpeed" href="https://codspeed.io/quarylabs/sqruff?utm_source=badge">
    <img src="https://img.shields.io/endpoint?url=https://codspeed.io/badge.json?utm_source=badge" alt="CodSpeed Badge"/>
  </a>
</p>

`sqruff` is a SQL linter and formatter written in Rust.

- Linting: advanced, configurable SQL linting
- Formatting: automated, configurable formatting
- Speed: fast and efficient
- Portability: easy to integrate into dev workflows

Try it in the browser: https://playground.quary.dev

## Quickstart

### Install (macOS)

```bash
brew install sqruff
```

For other platforms (pip, cargo, binary downloads), see the [installation guide](https://playground.quary.dev/docs/getting-started/installation/).

### Lint a project

```bash
sqruff lint . --dialect postgres
```

### Configure a project

You can also install sqruff using [pip](https://pypi.org/project/sqruff/).

```bash
pip install sqruff
```

#### For other platforms

Either download the binary from the [releases page](https://github.com/quarylabs/sqruff/releases) with `cargo binstall` or compile it yourself with cargo:

```bash
cargo binstall sqruff
cargo install sqruff
```

#### GitHub Action

You can also use the GitHub Action to install and run sqruff in your CI/CD pipeline. Inside a GitHub Action, `sqruff` automatically outputs linting results in the GitHub format so they can be easily viewed in the PR tab.

```yaml
jobs:
  sqruff-lint:
    name: Lint with sqruff
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: quarylabs/install-sqruff-cli-action@main
      - run: sqruff lint .
```

#### Visual Studio Code Extension

In addition to the CLI installation mechanism listed above, sqruff is also released as a [Visual Studio Code extension](https://marketplace.visualstudio.com/items?itemName=Quary.sqruff).

### Usage

#### Linting

To lint a SQL file or set of files, run the following command:

```bash
sqruff lint <file>
sqruff lint <file1> <file2> <file3>
sqruff lint <directory>
```

#### Fixing

To fix a single file or a set of files, run the following command:

```bash
sqruff fix <file/paths/directory>
```

#### Configuration

Settings for SQL dialect, indentation, capitalization, and other linting/style options are configured in a `.sqruff` file. This file should be located in the directory where Sqruff is being run.

The following example highlights a few configuration points: setting the dialect to `sqlite`, turning on all rules except AM01 and AM02, and configuring some indentation settings. For a comprehensive list of configuration options, see the defaults in `crates/lib/src/core/config.rs`. You can also refer to the [rules documentation](docs/rules.md) for more information on configuring specific rules.

```ini
[sqruff]
dialect = postgres
```

Then run without the `--dialect` flag:

```bash
sqruff lint .
```

## Documentation

Full documentation: [playground.quary.dev/docs](https://playground.quary.dev/docs/)

Key entry points:

- [Installation](https://playground.quary.dev/docs/getting-started/installation/)
- [Usage](https://playground.quary.dev/docs/usage/lint/)
- [Configuration](https://playground.quary.dev/docs/usage/configuration/)
- [Rules](https://playground.quary.dev/docs/reference/rules/)
- [CLI reference](https://playground.quary.dev/docs/reference/cli/)
