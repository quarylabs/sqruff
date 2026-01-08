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

`sqruff` is a SQL linter and formatter written in Rust. Key features include:

- **Linting:** Advanced, customizable SQL linting capabilities to ensure query quality.
- **Formatting:** Automated, configurable formatting for SQL code consistency.
- **Speed:** Fast and efficient, with minimal overhead.
- **Portability:** Designed to be easily integrated into various development workflows like a website.

Try it out in the [playground](https://playground.quary.dev)!

## Sqruff vs SQLFluff

Sqruff started its life aiming to be an exact replacement for [sqlfluff](https://sqlfluff.com/), but it is slowly diverging. Key differences include:

- **Accurate dialect definitions:** Unlike sqlfluff, sqruff aims for dialect definitions that accurately reflect the target SQL dialect. Sqruff only concerns itself with formatting valid SQL code rather than aiming to fix partially correct code.
- **Configuration:** While sqruff's configuration format is similar to sqlfluff, it will slowly diverge over time as sqruff develops its own identity.

## Dialects Supported

Sqruff currently supports the following SQL dialects:

- **ANSI SQL** - Standard SQL syntax - **This dialect is used by default**
- [**BigQuery**](https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax)
- [**Athena**](https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html)
- [**Clickhouse**](https://clickhouse.com/docs/en/sql-reference/)
- [**Databricks**](https://docs.databricks.com/en/sql/language-manual/index.html)
- [**DuckDB**](https://duckdb.org/docs/sql/introduction)
- [**Mysql**](https://dev.mysql.com/doc/)
- [**PostgreSQL**](https://www.postgresql.org/docs/current/sql.html)
- [**Redshift**](https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html)
- [**Snowflake**](https://docs.snowflake.com/en/sql-reference.html)
- [**SparkSql**](https://spark.apache.org/sql/)
- [**SQLite**](https://www.sqlite.org/lang.html)
- [**Trino**](https://trino.io/docs/current/sql.html)

While those above are the supported dialects, we are working on adding support for more dialects in the future.

## Getting Started

### Try it in your browser

Open the [playground](https://playground.quary.dev) to try out the linter and formatter online.

### Installation

#### Homebrew

You can use [brew](https://brew.sh/) to install sqruff easily on macOS.

```bash
brew install sqruff
```

#### Download the binary with a bash script

Using `bash`:

```bash
# Install to default location (/usr/local/bin)
curl -fsSL https://raw.githubusercontent.com/quarylabs/sqruff/main/install.sh | bash

# Install to custom directory
curl -fsSL https://raw.githubusercontent.com/quarylabs/sqruff/main/install.sh | bash -s ~/.local/bin
```

#### Pip

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
dialect = sqlite
exclude_rules = AM01,AM02
rules = all

[sqruff:indentation]
indent_unit = space
tab_space_size = 4
indented_joins = True
```

#### Ignoring files

Like `.ignore` files, sqruff ignores files and folders specified in a `.sqruffignore` file placed in the root of where the command is run. For example, if placed in `.sqruffignore`, the following code will ignore `.hql` files and files in any directory named temp:

```
# ignore ALL .hql files
*.hql

# ignore ALL files in ANY directory named temp
temp/
```

#### Ignoring errors

The NoQA directive is a way to disable specific rules or all rules for a specific line or range of lines. Similar to flake8’s ignore, individual lines can be ignored by adding `-- noqa` to the end of the line.

##### Ignoring single-line errors

The following example will ignore all errors on the line where it is placed:

```sql
-- Ignore all errors
SeLeCt  1 from tBl ;    -- noqa

-- Ignore rule CP02 & rule CP03
SeLeCt  1 from tBl ;    -- noqa: CP02,CP03
```

##### Ignoring multiple line errors

Similar to pylint’s “pylint directive”, ranges of lines can be ignored by adding `-- noqa:disable=<rule>[,...] | all` to the line. Following this directive, specified rules (or all rules, if “all” was specified) will be ignored until a corresponding `-– noqa:enable=<rule>[,…] | all`.

For example:

```sql
-- Ignore rule AL02 from this line forward
SELECT col_a a FROM foo -- noqa: disable=AL02

-- Ignore all rules from this line forward
SELECT col_a a FROM foo -- noqa: disable=all

-- Enforce all rules from this line forward
SELECT col_a a FROM foo -- noqa: enable=all
```

#### Help

To get help on the available commands and options, run the following command:

```bash
sqruff --help
```

For all the details on the CLI commands and options, see the [CLI documentation](./docs/cli.md).

## Docs

For more details, see the documents in the [docs](./docs/) folder, which contains:

- [Details on the rules](./docs/rules.md)
- [Details on the CLI](./docs/cli.md)
- [Details on the templaters](./docs/templaters.md)
- [Sample configurations](./docs/sample_configurations.md)

## Experimental

### dbt Project Support

Sqruff has experimental support for linting and formatting dbt projects. This feature requires the Python version of sqruff to be installed.

#### Installation

```bash
pip install sqruff[dbt]
```

#### Configuration

To use sqruff with a dbt project, create a `.sqruff` configuration file in your dbt project root:

```ini
[sqruff]
dialect = snowflake  # Set to your target database dialect
templater = dbt

[sqruff:templater:dbt]
profiles_dir = ~/.dbt  # Path to your dbt profiles directory (optional)
```

#### Configuration Options

The following options can be set under `[sqruff:templater:dbt]`:

| Option         | Description                                          | Default                                |
| -------------- | ---------------------------------------------------- | -------------------------------------- |
| `profiles_dir` | Path to the directory containing your `profiles.yml` | `~/.dbt`                               |
| `project_dir`  | Path to your dbt project directory                   | Current working directory              |
| `profile`      | The dbt profile to use                               | Profile specified in `dbt_project.yml` |
| `target`       | The dbt target to use                                | Default target in profile              |

#### Usage

Once configured, run sqruff as usual from your dbt project directory:

```bash
# Lint all SQL files in the models directory
sqruff lint models/

# Fix formatting issues
sqruff fix models/
```

Sqruff will automatically compile your dbt models using Jinja templating, resolving refs, sources, and macros before linting.

#### Limitations

- The dbt templater requires a valid dbt project setup with `dbt_project.yml` and `profiles.yml`
- Macros and disabled models are automatically skipped
- stdin input is not supported when using the dbt templater

## Community Projects

If your project isn't listed here and you would like it to be, please feel free to create a PR.

- [`cargo-sqruff`](https://github.com/gvozdvmozgu/cargo-sqruff): you can use [`cargo-sqruff`](https://github.com/gvozdvmozgu/cargo-sqruff), implemented as a `dylint` plugin, to lint your SQL queries that run through `sqlx`!

## Community

Join the Quary community on [Slack](https://join.slack.com/t/quarylabs/shared_invite/zt-2dlbfnztw-dMLXJVL38NcbhqRuM5gUcw) to ask questions, suggest features, or share your projects. Also feel free to raise any issues in the repository.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines on how to run things locally and on how to contribute.

## Credits

The sqruff project wouldn't be possible without "heavy inspiration" from the [sqlfluff](https://sqlfluff.com/) and [ruff](https://github.com/astral-sh/ruff) projects! We're very grateful to their awesome work!
