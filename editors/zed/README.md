# Sqruff for Zed

Runs [`sqruff`](https://github.com/quarylabs/sqruff) as a language server in
Zed, giving you SQL linting diagnostics and formatting.

## Requirements

Zed does not ship SQL support itself, so install the **SQL** extension as well —
it provides the `SQL` language this extension attaches to. Zed usually offers to
install it the first time you open a `.sql` file.

You do **not** need to install `sqruff` separately. The extension uses, in order:

1. the binary at `lsp.sqruff.binary.path`, if you set one;
2. `sqruff` on your `$PATH`;
3. otherwise, the latest release downloaded from GitHub.

## Configuration

Zed formats SQL with Prettier by default. To lint and format with sqruff, add to
your `settings.json` (or a project's `.zed/settings.json`):

```jsonc
{
  "languages": {
    "SQL": {
      "language_servers": ["sqruff"],
      "formatter": { "language_server": { "name": "sqruff" } },
      "format_on_save": "on",
    },
  },
}
```

Rules and dialect are configured with a `.sqruff` file in your project, exactly
as they are for the CLI:

```ini
[sqruff]
dialect = snowflake
exclude_rules = AM01,AM02
```

### Overriding the binary

To run a specific binary rather than one found on `$PATH` or downloaded:

```jsonc
{
  "lsp": {
    "sqruff": {
      "binary": {
        "path": "/path/to/sqruff",
      },
    },
  },
}
```

`arguments` is also honoured, but it replaces the default `["lsp"]` wholesale,
so `lsp` must remain the last argument. Note that `sqruff lsp` reads its dialect
and rules only from `.sqruff` — global flags such as `--dialect` are accepted by
the CLI but ignored by the language server, so configure them in `.sqruff`.

## Development

To try changes without publishing, open the Command Palette in Zed, run
`zed: install dev extension`, and select this directory (`editors/zed`).
