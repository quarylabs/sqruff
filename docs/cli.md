# Command-Line Help for `sqruff`

This document contains the help content for the `sqruff` command-line program.

**Command Overview:**

* [`sqruff`↴](#sqruff)
* [`sqruff lint`↴](#sqruff-lint)
* [`sqruff fix`↴](#sqruff-fix)
* [`sqruff lsp`↴](#sqruff-lsp)
* [`sqruff info`↴](#sqruff-info)
* [`sqruff rules`↴](#sqruff-rules)
* [`sqruff parse`↴](#sqruff-parse)

## `sqruff`

sqruff is a sql formatter and linter

**Usage:** `sqruff [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `lint` — Lint SQL files via passing a list of files or using stdin
* `fix` — Fix SQL files via passing a list of files or using stdin
* `lsp` — Run an LSP server
* `info` — Print information about sqruff and the current environment
* `rules` — Explain the available rules
* `parse` — Parse SQL and output the parse tree for debugging

###### **Options:**

* `--config <CONFIG>` — Path to a configuration file
* `--parsing-errors` — Show parse errors

  Default value: `false`



## `sqruff lint`

Lint SQL files via passing a list of files or using stdin

**Usage:** `sqruff lint [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>` — Files or directories to fix. Use `-` to read from stdin

###### **Options:**

* `-f`, `--format <FORMAT>`

  Default value: `human`

  Possible values: `human`, `github-annotation-native`, `json`




## `sqruff fix`

Fix SQL files via passing a list of files or using stdin

**Usage:** `sqruff fix [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>` — Files or directories to fix. Use `-` to read from stdin

###### **Options:**

* `-f`, `--format <FORMAT>` — The output format for the results

  Default value: `human`

  Possible values: `human`, `github-annotation-native`, `json`




## `sqruff lsp`

Run an LSP server

**Usage:** `sqruff lsp`



## `sqruff info`

Print information about sqruff and the current environment

**Usage:** `sqruff info`



## `sqruff rules`

Explain the available rules

**Usage:** `sqruff rules`



## `sqruff parse`

Parse SQL and output the parse tree for debugging

**Usage:** `sqruff parse [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>` — Files or directories to parse. Use `-` to read from stdin

###### **Options:**

* `-f`, `--format <FORMAT>` — The output format for the parse tree

  Default value: `pretty`

  Possible values: `json`, `pretty`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
