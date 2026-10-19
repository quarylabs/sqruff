"""Regression coverage for build tools relocated into Bazel's sandbox."""

import sys
import tempfile
import unittest
from pathlib import Path

from build_python_dist import build


class PythonDistTest(unittest.TestCase):
    def test_build_with_unusable_python_module_launcher(self):
        with tempfile.TemporaryDirectory(prefix="python dist ") as directory:
            root = Path(directory)
            tools = root / "relocated tools"
            (tools / "bin").mkdir(parents=True)
            # Reproduce the CI environment: module discovery through this
            # interpreter fails, while the installed native tool is usable.
            python = tools / "bin/python3"
            python.write_text("#!/bin/sh\necho 'No module named maturin' >&2\nexit 1\n")
            python.chmod(0o755)
            maturin = tools / "bin/maturin"
            maturin.write_text(
                f"#!{sys.executable}\n"
                "import pathlib, sys\n"
                "output = pathlib.Path(sys.argv[sys.argv.index('--out') + 1])\n"
                "output.mkdir(parents=True)\n"
                "name = 'sqruff.whl' if sys.argv[1] == 'build' else 'sqruff.tar.gz'\n"
                "(output / name).write_text('artifact')\n"
            )
            maturin.chmod(0o755)
            build(
                {
                    "sources": [],
                    "vendor": str(root / "vendor"),
                    "tools_python": str(tools),
                    "interpreters": [],
                    "cargo": sys.executable,
                    "rustc": sys.executable,
                    "cc": sys.executable,
                    "cxx": sys.executable,
                    "ar": sys.executable,
                    "wheels": str(root / "wheels"),
                    "sdist": str(root / "sdist"),
                }
            )
            self.assertTrue((root / "wheels/sqruff.whl").is_file())
            self.assertTrue((root / "sdist/sqruff.tar.gz").is_file())


if __name__ == "__main__":
    unittest.main()
