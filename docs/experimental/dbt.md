# dbt project support

Sqruff has experimental support for linting and formatting dbt projects. This feature requires the Python version of sqruff to be installed.

## Installation

```bash
pip install sqruff[dbt]
```

## Configuration

To use sqruff with a dbt project, create a `.sqruff` configuration file in your dbt project root:

```ini
[sqruff]
dialect = snowflake  # Set to your target database dialect
templater = dbt

[sqruff:templater:dbt]
profiles_dir = ~/.dbt  # Path to your dbt profiles directory (optional)
```

## Configuration options

The following options can be set under `[sqruff:templater:dbt]`:

| Option         | Description                                          | Default                                |
| -------------- | ---------------------------------------------------- | -------------------------------------- |
| `profiles_dir` | Path to the directory containing your `profiles.yml` | `~/.dbt`                               |
| `project_dir`  | Path to your dbt project directory                   | Current working directory              |
| `profile`      | The dbt profile to use                               | Profile specified in `dbt_project.yml` |
| `target`       | The dbt target to use                                | Default target in profile              |

## Usage

Once configured, run sqruff as usual from your dbt project directory:

```bash
# Lint all SQL files in the models directory
sqruff lint models/

# Fix formatting issues
sqruff fix models/
```

Sqruff will automatically compile your dbt models using Jinja templating, resolving refs, sources, and macros before linting.

## Limitations

- The dbt templater requires a valid dbt project setup with `dbt_project.yml` and `profiles.yml`
- Macros and disabled models are automatically skipped
- stdin input is not supported when using the dbt templater
