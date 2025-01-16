# Command-Line Help for `sqruff`

This document contains the help content for the `sqruff` command-line program.

**Command Overview:**

* [`sqruff`↴](#sqruff)
* [`sqruff lint`↴](#sqruff-lint)
* [`sqruff fix`↴](#sqruff-fix)
* [`sqruff lsp`↴](#sqruff-lsp)
* [`sqruff info`↴](#sqruff-info)

## `sqruff`

sqruff is a sql formatter and linter

**Usage:** `sqruff [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `lint` — Lint files
* `fix` — Fix files
* `lsp` — Run an LSP server
* `info` — Print information about sqruff and the current environment

###### **Options:**

* `--config <CONFIG>` — Path to a configuration file
* `--parsing-errors` — Show parse errors

  Default value: `false`



## `sqruff lint`

Lint files

**Usage:** `sqruff lint [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>` — Files or directories to fix. Use `-` to read from stdin

###### **Options:**

* `-f`, `--format <FORMAT>`

  Default value: `human`

  Possible values: `human`, `github-annotation-native`, `json`




## `sqruff fix`

Fix files

**Usage:** `sqruff fix [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>` — Files or directories to fix. Use `-` to read from stdin

###### **Options:**

* `--force` — Skip the confirmation prompt and go straight to applying fixes
* `-f`, `--format <FORMAT>`

  Default value: `human`

  Possible values: `human`, `github-annotation-native`, `json`




## `sqruff lsp`

Run an LSP server

**Usage:** `sqruff lsp`



## `sqruff info`

Print information about sqruff and the current environment

**Usage:** `sqruff info`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
