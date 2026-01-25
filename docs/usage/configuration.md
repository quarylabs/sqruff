# Configuration

Settings for SQL dialect, indentation, capitalization, and other linting and style options are configured in a `.sqruff` file.
This file should be located in the directory where sqruff is run.

The following example highlights a few configuration points: setting the dialect to `sqlite`, turning on all rules except AM01 and AM02, and configuring some indentation settings.
For a comprehensive list of configuration options, see the [default configuration file](../../crates/lib/src/core/default_config.cfg).
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

See [sample configurations](../reference/sample-configurations.md) for more examples.
