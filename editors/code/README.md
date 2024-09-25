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
</p>

`sqruff` is a SQL linter and formatter written in Rust. Key features include:

- **Linting:** Advanced, customizable SQL linting capabilities to ensure query quality.
- **Formatting:** Automated, configurable formatting for SQL code consistency.
- **Speed:** Fast and efficient, with minimal overhead.
- **Portability:** Designed to be easily integrated into various development workflows like a website.

Try it out in the [playground](https://playground.quary.dev)!

## Dialects Supported

Sqruff currently supports the following SQL dialects:

- **ANSI SQL** - Standard SQL syntax - **This dialect is used by default**
- [**BigQuery**](https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax)
- [**Athena**](https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html)
- [**Clickhouse**](https://clickhouse.com/docs/en/sql-reference/)
- [**DuckDB**](https://duckdb.org/docs/sql/introduction)
- [**PostgreSQL**](https://www.postgresql.org/docs/current/sql.html)
- [**Snowflake**](https://docs.snowflake.com/en/sql-reference.html)
- [**SparkSql**](https://spark.apache.org/sql/)
- [**SQLite**](https://www.sqlite.org/lang.html)
- [**Trino**](https://trino.io/docs/current/sql.html)

While those above are the supported dialects, we are working on adding support for more dialects in the future.

## Getting Started

### Try it in your browser

Open the [playground](https://playground.quary.dev) to try out the linter and formatter online.

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

#### Pip

You can also install sqruff using [pip](https://pypi.org/project/sqruff/).

```bash
pip install sqruff
```

#### GitHub Action

You can also use the GitHub Action to install and run sqruff in your CI/CD pipeline.

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

#### For other platforms

Either download the binary from the [releases page](https://github.com/quarylabs/sqruff/releases) or compile it yourself and with cargo with the following commands.

```bash
rustup toolchain install nightly
cargo +nightly install sqruff
sqruff --help
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

To fix a single or set of files, run the following command:

```bash
sqruff fix <file/paths/directory>
```

#### Configuration

Settings for SQL dialect, indentation, capitalization, and other linting/style options are configured in a `.sqruff` file. This file should be located in the directory where Sqruff is being run.

The following example highlights a few configuration points: setting the dialect to `sqlite`, turning on all rules except AM01 and AM02, and configuring some indentation settings. For a comprehensive list of configuration options, see the [default configuration file](crates/lib/src/core/default_config.cfg). You can also refer to the [rules documentation](docs/rules.md) for more information on configuring specific rules.

```ini
[sqruff]
dialect = sqlite
exclude_rules = AM01,AM02
rules = all

[sqruff:indentation]
indent_unit = space
tab_space_size = 4
indented_joins = True
```

#### Help

To get help on the available commands and options, run the following command:

```bash
sqruff --help
```

For all the details on the CLI commands and options, see the [CLI documentation](./docs/cli.md).

## Docs

For more details about, see the documents in the [docs](./docs/) folder which contains:

- [Details on the rules](./docs/rules.md)
- [Details on the CLI](./docs/cli.md)
- [Details on the templaters](./docs/templaters.md)

## Community

Join the Quary community on [Slack](https://join.slack.com/t/quarylabs/shared_invite/zt-2dlbfnztw-dMLXJVL38NcbhqRuM5gUcw) to ask questions, suggest features, or share your projects. Also feel free to raise any issues in the repository.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines on how to run things locally and on how to contribute.

## Credits

The sqruff project wouldn't be possible without "heavy inspiration" from the [sqlfluff](https://sqlfluff.com/) and [ruff](https://github.com/astral-sh/ruff) projects! We're very grateful to their awesome work!
