"""Guard Cargo-hack's exact matrix and stage independently cached package inputs."""

import collections
import json
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# Preserve the existing check policy, including its intentional docs exclusion.
CHECK = [
    "check",
    "--each-feature",
    "--exclude-features=codegen-docs",
    "--locked",
    "--offline",
]


def commands(output):
    result = collections.Counter()
    for line in output.splitlines():
        command = tuple(shlex.split(line))
        if command[:2] != ("cargo", "check") or "--manifest-path" not in command:
            raise ValueError(f"Unexpected Cargo-hack command: {line}")
        result[command] += 1
    if not result:
        raise ValueError("Cargo-hack did not discover any checks")
    return result


def verify_matrix(expected, actual):
    missing, extra = expected - actual, actual - expected
    if missing or extra:
        raise ValueError(
            f"Cargo-hack coverage drift: missing {dict(missing)}, extra {dict(extra)}"
        )


def dependency_closure(packages, root):
    by_path = {str(Path(p["manifest_path"]).parent.resolve()): p for p in packages}
    seen, pending = set(), [root]
    while pending:
        path = pending.pop()
        if path in seen:
            continue
        seen.add(path)
        for dependency in by_path[path]["dependencies"]:
            if "path" in dependency:
                dependency_path = str(Path(dependency["path"]).resolve())
                if dependency_path not in by_path:
                    raise ValueError(
                        f"Unrepresented local Cargo dependency: {dependency_path}"
                    )
                pending.append(dependency_path)
    return seen


def prepare(specification, cargo, rustc, hack):
    outputs = {
        package: Path(path).resolve()
        for package, path in specification["packages"].items()
    }
    with tempfile.TemporaryDirectory(prefix="cargo-hack-matrix-") as directory:
        workspace = Path(directory).resolve()
        for source in specification["sources"]:
            destination = workspace / source
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
        environment = dict(
            os.environ, CARGO_HOME=str(workspace / ".cargo"), RUSTC=rustc, CARGO=cargo
        )

        def run(argv):
            result = subprocess.run(
                argv,
                cwd=workspace,
                env=environment,
                check=False,
                capture_output=True,
                text=True,
            )
            if result.returncode:
                raise ValueError(f"{shlex.join(argv)} failed:\n{result.stderr}")
            return result.stdout

        metadata = json.loads(
            run(
                [
                    cargo,
                    "metadata",
                    "--format-version=1",
                    "--no-deps",
                    "--all-features",
                    "--locked",
                    "--offline",
                ]
            )
        )
        packages = metadata["packages"]
        by_directory = {
            str(Path(p["manifest_path"]).parent.relative_to(workspace)): p
            for p in packages
        }
        if set(by_directory) != set(outputs):
            raise ValueError(
                f"Cargo package coverage drift: discovered {sorted(by_directory)}, Bazel has {sorted(outputs)}"
            )
        expected = commands(run([hack, "hack", *CHECK, "--print-command-list"]))
        actual = collections.Counter()
        plans = {}
        for package, output in outputs.items():
            argv = ["cargo", "hack", *CHECK, "--package", by_directory[package]["name"]]
            # Check the exact same command that will run in this package's test.
            actual.update(commands(run([hack, *argv[1:], "--print-command-list"])))
            plans[package] = argv
        verify_matrix(expected, actual)

        for package, output in outputs.items():
            closure = dependency_closure(packages, str(workspace / package))
            output.mkdir(parents=True, exist_ok=True)
            for source in specification["sources"]:
                path = workspace / source
                if (
                    Path(source).parent == Path(".")
                    or Path(source).name == "Cargo.toml"
                    or any(path.is_relative_to(root) for root in closure)
                ):
                    destination = output / source
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(path, destination)
            # Cargo parses every workspace manifest, even outside the dependency
            # closure. Preserve its discovered target inventory with empty roots.
            for item in packages:
                for target in item["targets"]:
                    destination = output / Path(target["src_path"]).relative_to(
                        workspace
                    )
                    if not destination.exists():
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        destination.write_text("")
            (output / "run_hack.sh").write_text(
                "#!/bin/bash\nset -euo pipefail\nexec "
                + shlex.join(plans[package])
                + "\n"
            )
        print(
            f"Verified {sum(expected.values())} Cargo-hack combinations across {len(outputs)} package tests"
        )


def main():
    specification, cargo, rustc, hack = sys.argv[1:]
    prepare(
        json.loads(Path(specification).read_text()),
        *(str(Path(tool).resolve()) for tool in [cargo, rustc, hack]),
    )


if __name__ == "__main__":
    main()
