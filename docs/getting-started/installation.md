# Installation

## Homebrew

You can use Homebrew to install sqruff on macOS.

```bash
brew install sqruff
```

## Download the binary with a bash script

Using bash:

```bash
# Install to default location (/usr/local/bin)
curl -fsSL https://raw.githubusercontent.com/quarylabs/sqruff/main/install.sh | bash

# Install to custom directory
curl -fsSL https://raw.githubusercontent.com/quarylabs/sqruff/main/install.sh | bash -s ~/.local/bin
```

## Pip

You can also install sqruff using pip.

```bash
pip install sqruff
```

## Other platforms

Download the binary from the releases page with cargo-binstall or compile it with cargo:

```bash
cargo binstall sqruff
cargo install sqruff
```

Releases: https://github.com/quarylabs/sqruff/releases

## GitHub Action

Use the GitHub Action to install and run sqruff in CI.

```yaml
jobs:
  sqruff-lint:
    name: Lint with sqruff
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: quarylabs/install-sqruff-cli-action@main
      - run: sqruff lint .
```

## Visual Studio Code Extension

Sqruff is also released as a Visual Studio Code extension:
https://marketplace.visualstudio.com/items?itemName=Quary.sqruff
