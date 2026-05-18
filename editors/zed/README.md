# Sqruff LSP for Zed

This Zed extension wires up `sqruff lsp` as a language server for Zed’s **SQL**
language support (provided by the `SQL` extension).

## Prereqs

- Install `sqruff` so it’s available on your `$PATH` (or configure a custom path
  in Zed settings).

## Install (dev extension)

In Zed:

1. Open the Command Palette
2. Run `zed: install dev extension`
3. Select this directory: `editors/zed`

## Configure

Project settings live at `.zed/settings.json`.

Basic enablement:

```jsonc
{
  "languages": {
    "SQL": {
      "language_servers": ["sqruff", "..."],
      "formatter": { "language_server": { "name": "sqruff" } },
      "format_on_save": "on"
    }
  }
}
```

If `sqruff` isn’t on your `$PATH`, or you want to pass global flags (like dialect)
before `lsp`, set the binary settings:

```jsonc
{
  "lsp": {
    "sqruff": {
      "binary": {
        "path": "/path/to/sqruff",
        "arguments": ["--dialect", "snowflake", "lsp"]
      }
    }
  }
}
```
