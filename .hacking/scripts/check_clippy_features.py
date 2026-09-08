"""Verify configured Bazel Clippy features against Cargo's all-features set."""

import json
import sys
from pathlib import Path

import tomllib


def all_features(manifest, workspace):
    features = manifest.get("features", {})
    names = set(features)
    explicit_deps = {
        feature.removeprefix("dep:")
        for values in features.values()
        for feature in values
        if feature.startswith("dep:")
    }
    sections = [manifest, *manifest.get("target", {}).values()]
    for section in sections:
        for kind in ("dependencies", "build-dependencies"):
            for name, dependency in section.get(kind, {}).items():
                if not isinstance(dependency, dict):
                    continue
                if dependency.get("workspace"):
                    inherited = workspace.get("dependencies", {}).get(name, {})
                    if isinstance(inherited, dict):
                        dependency = inherited | dependency
                if dependency.get("optional") and name not in explicit_deps:
                    names.add(name)
    return names


def check(entries, manifests):
    workspace = manifests["Cargo.toml"].get("workspace", {})
    errors = []
    for entry in entries:
        expected = all_features(manifests[entry["manifest"]], workspace)
        actual = set(entry["features"])
        if actual != expected:
            errors.append(
                f"{entry['target']} differs from {entry['manifest']}: "
                f"missing features {sorted(expected - actual)}, "
                f"unexpected features {sorted(actual - expected)}. "
                "Update the Bazel all-features Clippy configuration."
            )
    if not entries:
        errors.append("No Clippy targets found to verify")
    return errors


def main():
    inventory, marker, *paths = sys.argv[1:]
    manifests = {path: tomllib.loads(Path(path).read_text()) for path in paths}
    errors = check(json.loads(Path(inventory).read_text()), manifests)
    if errors:
        sys.exit("\n".join(errors))
    Path(marker).touch()


if __name__ == "__main__":
    main()
