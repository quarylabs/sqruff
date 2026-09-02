# Configuration

Settings for SQL dialect, indentation, capitalization, and other linting and style options are configured in a `.sqruff`, `.sqruff.ini`, `sqruff.toml`, or `pyproject.toml` file.
Sqruff loads configuration files from the user's home directory and each
directory down to the directory where sqruff is run. When linting a file in a
subdirectory, it then loads configuration from each directory down to the
file. Settings found closer to the file override settings from outer
directories.

The following example highlights a few configuration points: setting the dialect to `sqlite`, turning on all rules except AM01 and AM02, and configuring some indentation settings.
For a comprehensive list of configuration options, see the [default configuration file](https://github.com/quarylabs/sqruff/blob/main/crates/lib/src/core/default_config.cfg).
You can also refer to the [rules documentation](../reference/rules.md) for more information on configuring specific rules.

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

The same configuration can be written in TOML:

```toml
[tool.sqlfluff.core]
dialect = "sqlite"
exclude_rules = ["AM01", "AM02"]
rules = "all"

[tool.sqlfluff.indentation]
indent_unit = "space"
tab_space_size = 4
indented_joins = true
```

See [sample configurations](../reference/sample-configurations.md) for more examples.
