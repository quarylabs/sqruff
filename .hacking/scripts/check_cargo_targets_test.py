import tempfile
import unittest
from pathlib import Path

from check_cargo_targets import check_coverage, target_modes


class CargoCoverageTest(unittest.TestCase):
    def test_library_unit_and_benchmark_modes(self):
        target = {"kind": ["rlib"], "name": "example", "test": True}
        self.assertEqual(target_modes(target, {}), [(False, False), (True, True)])
        target["test"] = False
        self.assertEqual(target_modes(target, {}), [(False, False), (True, True)])
        self.assertEqual(
            target_modes(target, {"lib": {"bench": False}}), [(False, False)]
        )

    def test_criterion_benchmark_is_compiled_in_test_mode_without_harness(self):
        target = {"kind": ["bench"], "name": "speed", "test": False}
        manifest = {"bench": [{"name": "speed", "harness": False}]}
        self.assertEqual(target_modes(target, manifest), [(True, False)])

    def test_new_build_script_requires_coverage(self):
        with self.assertRaises(ValueError):
            target_modes({"kind": ["custom-build"]}, {})

    def test_new_test_benchmark_and_example_are_not_silently_skipped(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            manifest = workspace / "Cargo.toml"
            manifest.write_text('[package]\nname = "example"\nversion = "0.1.0"\n')
            for kind in ["test", "bench", "example"]:
                target = {
                    "name": "new_target",
                    "kind": [kind],
                    "test": kind == "test",
                    "src_path": str(workspace / "new.rs"),
                    "edition": "2024",
                }
                metadata = {
                    "packages": [
                        {
                            "name": "example",
                            "manifest_path": str(manifest),
                            "features": {"new": []},
                            "targets": [target],
                        }
                    ]
                }
                errors, count = check_coverage(metadata, [], workspace)
                self.assertEqual(count, 1)
                self.assertIn("Missing Bazel compiler target", errors[0])
                entry = {
                    "root": "new.rs",
                    "compilation_mode": "fastbuild",
                    "lint_config": True,
                    "test": kind != "example",
                    "harness": True,
                    "edition": "2024",
                    "features": ["new"],
                }
                self.assertEqual(check_coverage(metadata, [entry], workspace)[0], [])
                entry["features"] = []
                self.assertIn(
                    "features, edition or harness differ",
                    check_coverage(metadata, [entry], workspace)[0][0],
                )
                entry["features"] = ["new"]
                entry["edition"] = "2021"
                self.assertTrue(check_coverage(metadata, [entry], workspace)[0])

    def test_empty_inventory_fails(self):
        self.assertTrue(check_coverage({"packages": []}, [], Path("."))[0])


if __name__ == "__main__":
    unittest.main()
