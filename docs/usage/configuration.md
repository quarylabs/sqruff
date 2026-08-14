# Configuration

Settings for SQL dialect, indentation, capitalization, and other linting and style options are configured in a `.sqruff`, `.sqruff.ini`, `sqruff.toml`, or `pyproject.toml` file.
This file should be located in the directory where sqruff is run.

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

## Rule configuration

Rules are configured in a section named after the rule, for example
`[sqruff:rules:capitalisation.keywords]`. Every option a rule accepts — its type,
its default, and the values it accepts — is listed with that rule in the
[rules documentation](../reference/rules.md).

```ini
[sqruff:rules:capitalisation.keywords]
capitalisation_policy = upper
ignore_words = pi, e
```

Options are typed and validated when the configuration is loaded, so a value of
the wrong type or one outside the accepted set is reported rather than silently
ignored:

```console
$ sqruff lint query.sql
Error in configuration for rule CP01 (capitalisation.keywords): Invalid value for
`capitalisation_policy`: expected one of [consistent, upper, lower, capitalise],
got `uppercase`
```

A few options are shared by several rules and can be set once directly under
`[sqruff:rules]`. Rules that accept them inherit the shared value, and a rule's
own section always wins:

```ini
[sqruff:rules]
# Applies to every rule that accepts it, e.g. capitalisation.identifiers
unquoted_identifiers_policy = all

[sqruff:rules:references.keywords]
# ...except here, where the rule's own value is used
unquoted_identifiers_policy = aliases
```
