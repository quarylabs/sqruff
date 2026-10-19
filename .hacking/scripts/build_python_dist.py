"""Build the host's release wheels and sdist using only declared Bazel inputs."""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def build(specification):
    tools = {
        name: str(Path(specification[name]).resolve())
        for name in ["cargo", "rustc", "cc", "cxx", "ar"]
    }
    python = Path(specification["tools_python"]).resolve() / "bin/python3"
    interpreters = [
        str(Path(path).resolve() / "bin/python3")
        for path in specification["interpreters"]
    ]
    vendor = str(Path(specification["vendor"]).resolve())
    wheels = str(Path(specification["wheels"]).resolve())
    sdist = str(Path(specification["sdist"]).resolve())
    with tempfile.TemporaryDirectory(prefix="sqruff-python-dist-") as directory:
        workspace = Path(directory)
        for source in specification["sources"]:
            destination = workspace / source
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
        cargo_home = workspace / ".cargo"
        cargo_home.mkdir()
        (cargo_home / "config.toml").write_text(
            '[source.crates-io]\nreplace-with = "vendored-sources"\n'
            f"[source.vendored-sources]\ndirectory = {json.dumps(vendor)}\n"
        )
        # rustc's linker defaults to `cc`, so expose the selected C toolchain
        # under the conventional names as well as Cargo's CC/CXX/AR variables.
        tool_bin = workspace / "bin"
        tool_bin.mkdir()
        for name in ["cc", "cxx", "ar"]:
            (tool_bin / ("c++" if name == "cxx" else name)).symlink_to(tools[name])
        environment = dict(
            os.environ,
            CARGO=tools["cargo"],
            RUSTC=tools["rustc"],
            CC=tools["cc"],
            CXX=tools["cxx"],
            AR=tools["ar"],
            CARGO_HOME=str(cargo_home),
            CARGO_TARGET_DIR=str(workspace / "target"),
            CARGO_NET_OFFLINE="true",
            CARGO_INCREMENTAL="0",
            CARGO_BUILD_JOBS="2",
            PYTHONNOUSERSITE="1",
            PYTHONDONTWRITEBYTECODE="1",
            PATH=os.pathsep.join(
                [str(tool_bin), str(Path(tools["cargo"]).parent), os.environ["PATH"]]
            ),
        )
        manifest = ["--manifest-path", "crates/cli-python/Cargo.toml"]
        # A single target directory shares Rust dependencies between the ABI3
        # wheel and the two free-threaded builds. maturin selects PyO3's ABI
        # for each explicitly supplied interpreter, just as the release CI does.
        subprocess.run(
            [
                str(python),
                "-I",
                "-m",
                "maturin",
                "build",
                "--release",
                "--locked",
                "--offline",
                "--out",
                wheels,
                *manifest,
                "--interpreter",
                *interpreters,
            ],
            cwd=workspace,
            env=environment,
            check=True,
        )
        subprocess.run(
            [str(python), "-I", "-m", "maturin", "sdist", "--out", sdist, *manifest],
            cwd=workspace,
            env=environment,
            check=True,
        )
        if not list(Path(wheels).glob("*.whl")) or not list(
            Path(sdist).glob("*.tar.gz")
        ):
            raise ValueError("maturin did not produce wheels and a source distribution")


if __name__ == "__main__":
    build(json.loads(Path(sys.argv[1]).read_text()))
