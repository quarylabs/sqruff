"""Compare native Bazel compilation with Cargo's discovered workspace targets."""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import tomllib


def target_modes(target, manifest):
    """Return compilation modes, including unit/benchmark harnesses on libraries."""
    kind = target["kind"][0]
    if kind in ("lib", "rlib", "cdylib", "staticlib", "dylib", "proc-macro"):
        settings = manifest.get("lib", {})
        modes = [(False, False)]
    elif kind == "bin":
        settings = next(
            (
                item
                for item in manifest.get("bin", [])
                if item["name"] == target["name"]
            ),
            {},
        )
        modes = [(False, False)]
    elif kind in ("test", "bench", "example"):
        settings = next(
            (item for item in manifest.get(kind, []) if item["name"] == target["name"]),
            {},
        )
        if kind == "example" and not target["test"]:
            return [(False, False)]
        return [(True, settings.get("harness", True))]
    else:
        raise ValueError(
            f"Cargo target kind {kind!r} needs explicit native coverage support"
        )
    if target["test"] or settings.get("bench", True):
        modes.append((True, settings.get("harness", True)))
    return modes


def check_coverage(metadata, entries, workspace):
    errors = []
    checked = 0
    for package in metadata["packages"]:
        manifest_path = Path(package["manifest_path"])
        manifest = tomllib.loads(manifest_path.read_text())
        features = set(package["features"])
        for target in package["targets"]:
            root = str(Path(target["src_path"]).relative_to(workspace))
            try:
                modes = target_modes(target, manifest)
            except ValueError as error:
                errors.append(f"{package['name']}/{target['name']}: {error}")
                continue
            for test_mode, harness in modes:
                checked += 1
                candidates = [
                    entry
                    for entry in entries
                    if entry["root"] == root and entry["test"] == test_mode
                ]
                description = f"{package['name']}/{target['name']} ({'test' if test_mode else 'normal'} mode, {root})"
                if not candidates:
                    errors.append(f"Missing Bazel compiler target for {description}")
                elif not any(
                    set(entry["features"]) == features
                    and entry["edition"] == target["edition"]
                    and entry["compilation_mode"] == "fastbuild"
                    and entry["lint_config"]
                    and (not test_mode or entry["harness"] == harness)
                    for entry in candidates
                ):
                    errors.append(
                        f"Bazel features, edition or harness differ for {description} "
                        "(requires fastbuild and Cargo lint policy): "
                        f"expected features {sorted(features)}, edition {target['edition']}, harness {harness}; "
                        f"got {candidates}"
                    )
    if not checked:
        errors.append("Cargo did not discover any compiler targets")
    return errors, checked


def main():
    inventory, sources, marker, cargo, rustc = sys.argv[1:]
    entries = json.loads(Path(inventory).read_text())
    cargo, rustc = str(Path(cargo).resolve()), str(Path(rustc).resolve())
    with tempfile.TemporaryDirectory(prefix="cargo-target-coverage-") as directory:
        workspace = Path(directory).resolve()
        for path in json.loads(Path(sources).read_text()):
            destination = workspace / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, destination)
        environment = dict(
            os.environ, CARGO_HOME=str(workspace / ".cargo"), RUSTC=rustc
        )
        result = subprocess.run(
            [
                cargo,
                "metadata",
                "--format-version=1",
                "--no-deps",
                "--all-features",
                "--locked",
                "--offline",
            ],
            cwd=workspace,
            env=environment,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode:
            sys.exit(result.stderr)
        errors, checked = check_coverage(json.loads(result.stdout), entries, workspace)
        if errors:
            sys.exit(
                "\n".join(errors)
                + "\nAdd or update native compiler targets before removing Cargo coverage."
            )
    Path(marker).write_text(f"Verified {checked} Cargo target/mode combinations\n")


if __name__ == "__main__":
    main()
