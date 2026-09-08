import unittest

from check_clippy_features import all_features, check


class FeatureCoverageTest(unittest.TestCase):
    def test_optional_dependencies_and_dep_suppression(self):
        manifest = {
            "features": {"default": ["python"], "python": ["dep:pyo3"]},
            "dependencies": {
                "pyo3": {"optional": True},
                "yaml": {"package": "serde_yaml", "optional": True},
                "normal": "1",
            },
        }
        self.assertEqual(all_features(manifest, {}), {"default", "python", "yaml"})

    def test_platform_and_build_dependencies(self):
        manifest = {
            "target": {
                "cfg(windows)": {
                    "dependencies": {"allocator": {"workspace": True, "optional": True}}
                }
            },
            "build-dependencies": {"generator": {"optional": True}},
        }
        workspace = {"dependencies": {"allocator": {"version": "1"}}}
        self.assertEqual(all_features(manifest, workspace), {"allocator", "generator"})

    def test_explicit_feature_survives_dep_suppression(self):
        self.assertEqual(
            all_features(
                {
                    "features": {"foo": ["dep:foo"]},
                    "dependencies": {"foo": {"optional": True}},
                },
                {},
            ),
            {"foo"},
        )

    def test_detects_added_and_removed_features(self):
        entries = [
            {
                "target": "//crate:lib",
                "manifest": "crate/Cargo.toml",
                "features": ["old"],
            }
        ]
        manifests = {"Cargo.toml": {}, "crate/Cargo.toml": {"features": {"new": []}}}
        (error,) = check(entries, manifests)
        self.assertIn("missing features ['new']", error)
        self.assertIn("unexpected features ['old']", error)
        entries[0]["features"] = ["new"]
        self.assertEqual(check(entries, manifests), [])

    def test_rejects_empty_inventory(self):
        self.assertTrue(check([], {"Cargo.toml": {}}))


if __name__ == "__main__":
    unittest.main()
