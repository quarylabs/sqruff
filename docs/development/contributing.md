# Contributing

See `CONTRIBUTING.md` for guidelines on how to run things locally and how to contribute.

## VS Code defaults

The repository includes a sample `.vscode` in `.hacking/vscode` with recommended settings.

## Docs preview and build

Install Zensical (recommended in a venv):

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install zensical
```

Preview locally:

```bash
zensical serve
```

This runs a local preview server (default localhost:8000) and auto-reloads on changes.

Build static site:

```bash
zensical build
```

Build output goes to `site_dir` (default `site`).
