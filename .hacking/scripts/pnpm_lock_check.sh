#!/usr/bin/env bash
# Verify that pnpm-lock.yaml is up to date with the workspace package.json files.
#
# Runs `pnpm install --frozen-lockfile --lockfile-only`, which fails when the
# specifiers in pnpm-lock.yaml no longer match the package.json manifests (for
# example after a dependency bump that forgot to regenerate the lockfile).
#
# The workspace files are copied into a writable temp directory because Bazel
# runfiles are read-only symlinks and pnpm wants a writable tree.
set -euo pipefail

PNPM_BIN="$RUNFILES_DIR/$PNPM"

# pnpm needs a writable HOME for its config/cache and a writable workspace.
WORKDIR=$(mktemp -d)
export HOME="$WORKDIR"
# Silence the global "update available" notifier and behave like CI.
export CI=1
export npm_config_update_notifier=false

cd "$RUNFILES_DIR/_main"

# Copy only the files that determine the resolved lockfile, preserving the
# workspace directory structure declared in pnpm-workspace.yaml.
mkdir -p "$WORKDIR/editors/code" "$WORKDIR/playground"
cp package.json pnpm-workspace.yaml pnpm-lock.yaml "$WORKDIR/"
cp editors/code/package.json "$WORKDIR/editors/code/"
cp playground/package.json "$WORKDIR/playground/"

if ! "$PNPM_BIN" --dir "$WORKDIR" install --frozen-lockfile --lockfile-only; then
    echo ""
    echo "ERROR: pnpm-lock.yaml is out of date with the package.json files."
    echo "Run 'pnpm install --lockfile-only' and commit the updated pnpm-lock.yaml."
    exit 1
fi

echo "OK: pnpm-lock.yaml is up to date"
