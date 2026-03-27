# Sqruff

Sqruff is a SQL linter and formatter written in Rust. It focuses on formatting valid SQL for specific dialects and provides fast linting and fixing.

## Key features

- Linting: Advanced, customizable SQL linting.
- Formatting: Automated, configurable formatting for SQL code consistency.
- Lineage: Column-level data lineage analysis for SQL queries.
- Speed: Fast and efficient with minimal overhead.
- Portability: Designed to integrate into workflows, including CI.

## Sqruff vs SQLFluff

Sqruff started as an exact replacement for SQLFluff but is diverging.

- Accurate dialect definitions: Sqruff targets valid SQL for each dialect and does not try to fix partially correct SQL.
- Configuration: The config format is currently similar to SQLFluff but may diverge over time.

## Playground

Try sqruff in your browser at https://playground.quary.dev.

## Get started

- Install the CLI in [Installation](getting-started/installation.md)
- Learn basic commands: [Lint and fix](usage/lint.md)
- Configure behavior in [Configuration](usage/configuration.md)

## Experimental features

- [dbt project support](experimental/dbt.md)
- [SQL Column Lineage](experimental/lineage.md)

## Credits

The sqruff project is heavily inspired by [sqlfluff](https://sqlfluff.com/) and [ruff](https://github.com/astral-sh/ruff).
