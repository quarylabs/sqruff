import collections
import unittest
from pathlib import Path

from prepare_cargo_hack import (
    commands,
    dependency_closure,
    unchecked_directories,
    verify_matrix,
)


class CargoHackCoverageTest(unittest.TestCase):
    def test_matrix_rejects_missing_extra_and_duplicate_combinations(self):
        matrix = commands(
            "cargo check --manifest-path crates/a/Cargo.toml --no-default-features\ncargo check --manifest-path crates/a/Cargo.toml --features new\n"
        )
        verify_matrix(matrix, matrix.copy())
        for command in matrix:
            missing = matrix.copy()
            missing[command] -= 1
            with self.assertRaisesRegex(ValueError, "coverage drift"):
                verify_matrix(matrix, missing)
            duplicate = matrix.copy()
            duplicate[command] += 1
            with self.assertRaisesRegex(ValueError, "coverage drift"):
                verify_matrix(matrix, duplicate)
        extra = matrix + collections.Counter(
            {("cargo", "check", "--features", "unexpected"): 1}
        )
        with self.assertRaisesRegex(ValueError, "coverage drift"):
            verify_matrix(matrix, extra)

    def test_empty_or_unrecognized_command_output_fails_closed(self):
        for output in [
            "",
            "cargo test --manifest-path crates/a/Cargo.toml",
            "unrecognized output",
        ]:
            with self.assertRaises(ValueError):
                commands(output)

    def test_closure_includes_optional_dev_and_transitive_dependencies(self):
        packages = [
            {
                "manifest_path": "/a/Cargo.toml",
                "dependencies": [
                    {"path": "/b", "optional": True},
                    {"path": "/c", "kind": "dev"},
                ],
            },
            {"manifest_path": "/b/Cargo.toml", "dependencies": [{"path": "/d"}]},
            {"manifest_path": "/c/Cargo.toml", "dependencies": []},
            {"manifest_path": "/d/Cargo.toml", "dependencies": [{"path": "/a"}]},
            {"manifest_path": "/unrelated/Cargo.toml", "dependencies": []},
        ]
        self.assertEqual(dependency_closure(packages, "/a"), {"/a", "/b", "/c", "/d"})
        packages[1]["dependencies"] = [{"path": "/missing"}]
        with self.assertRaisesRegex(ValueError, "Unrepresented local Cargo dependency"):
            dependency_closure(packages, "/a")

    def test_only_unchecked_target_directories_are_omitted(self):
        package = {
            "manifest_path": "/a/Cargo.toml",
            "targets": [
                {"kind": ["rlib", "cdylib"], "src_path": "/a/src/lib.rs"},
                {"kind": ["bin"], "src_path": "/a/src/bin/bench.rs"},
                {"kind": ["bench"], "src_path": "/a/benches/speed.rs"},
                {"kind": ["test"], "src_path": "/a/tests/integration.rs"},
                {"kind": ["example"], "src_path": "/a/examples/demo.rs"},
            ],
        }
        self.assertEqual(
            unchecked_directories([package]),
            {Path("/a/benches"), Path("/a/tests"), Path("/a/examples")},
        )
        # A custom binary location must not be mistaken for an unchecked test.
        package["targets"].append({"kind": ["bin"], "src_path": "/a/tests/tool.rs"})
        self.assertEqual(
            unchecked_directories([package]),
            {Path("/a/benches"), Path("/a/examples")},
        )
        # Build scripts may read fixtures from any directory in their package.
        package["targets"].append({"kind": ["custom-build"], "src_path": "/a/build.rs"})
        self.assertEqual(unchecked_directories([package]), set())


if __name__ == "__main__":
    unittest.main()
