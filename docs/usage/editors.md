# Editors

Sqruff ships a language server, `sqruff lsp`, which provides linting diagnostics
and formatting in any editor with LSP support. Dialect and rules are read from
the project's [`.sqruff` file](configuration.md), exactly as for the CLI.

Two integrations are maintained in this repository:

| Editor  | Source                                                                                | Install                                        |
| ------- | ------------------------------------------------------------------------------------- | ---------------------------------------------- |
| VS Code | [`editors/code`](https://github.com/quarylabs/sqruff/tree/main/editors/code)           | Search for `sqruff` in the Extensions view      |
| Zed     | [`editors/zed`](https://github.com/quarylabs/sqruff/tree/main/editors/zed)             | Search for `Sqruff` in the Extensions view      |

## Zed

Zed does not ship SQL support itself, so install the **SQL** extension alongside
Sqruff — it provides the `SQL` language the server attaches to.

Zed formats SQL with Prettier by default. To lint and format with sqruff, add to
your `settings.json`:

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

The extension uses a `sqruff` binary on your `$PATH` if there is one, and
otherwise downloads the latest release. See the
[extension README](https://github.com/quarylabs/sqruff/tree/main/editors/zed)
for the full settings reference.

## Other editors

Any LSP-capable editor can run `sqruff lsp` directly. Point your client at the
`sqruff` binary with `lsp` as its only argument, and register it for SQL files.
