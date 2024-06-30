# Command-Line Help for `sqruff`

This document contains the help content for the `sqruff` command-line program.

**Command Overview:**

* [`sqruff`↴](#sqruff)
* [`sqruff lint`↴](#sqruff-lint)
* [`sqruff fix`↴](#sqruff-fix)
* [`sqruff lsp`↴](#sqruff-lsp)

## `sqruff`

sqruff is a sql formatter and linter

**Usage:** `sqruff <COMMAND>`

###### **Subcommands:**

* `lint` — lint files
* `fix` — fix files
* `lsp` — Run an LSP server



## `sqruff lint`

lint files

**Usage:** `sqruff lint [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>`

###### **Options:**

* `-f`, `--format <FORMAT>`

  Default value: `human`

  Possible values: `human`, `github-annotation-native`




## `sqruff fix`

fix files

**Usage:** `sqruff fix [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>`

###### **Options:**

* `--force` — Skip the confirmation prompt and go straight to applying fixes
* `-f`, `--format <FORMAT>`

  Default value: `human`

  Possible values: `human`, `github-annotation-native`




## `sqruff lsp`

Run an LSP server

**Usage:** `sqruff lsp`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
<<<<<<< HEAD:options.md

# Command-Line Help for `sqruff`

This document contains the help content for the `sqruff` command-line program.

**Command Overview:**

* [`sqruff`↴](#sqruff)
* [`sqruff lint`↴](#sqruff-lint)
* [`sqruff fix`↴](#sqruff-fix)

## `sqruff`

sqruff is a sql formatter and linter

**Usage:** `sqruff <COMMAND>`

###### **Subcommands:**

* `lint` — lint files
* `fix` — fix files



## `sqruff lint`

lint files

**Usage:** `sqruff lint [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>`

###### **Options:**

* `-f`, `--format <FORMAT>`

  Default value: `human`

  Possible values: `human`, `github-annotation-native`




## `sqruff fix`

fix files

**Usage:** `sqruff fix [OPTIONS] [PATHS]...`

###### **Arguments:**

* `<PATHS>`

###### **Options:**

* `--force` — Skip the confirmation prompt and go straight to applying fixes

  Possible values: `true`, `false`

* `-f`, `--format <FORMAT>`

  Default value: `human`

  Possible values: `human`, `github-annotation-native`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

=======
>>>>>>> main:docs/cli.md
