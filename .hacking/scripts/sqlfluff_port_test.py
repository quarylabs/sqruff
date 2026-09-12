"""Offline regression tests: real temporary Git repos, mocked network/build tools."""

import argparse
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "sqlfluff_port", Path(__file__).with_name("sqlfluff_port.py")
)
port = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(port)


def pr(number, head, base="main", body="", title="SQLFluff port"):
    return {
        "number": number,
        "head": {"ref": head},
        "base": {"ref": base},
        "body": body,
        "title": title,
        "html_url": f"https://github.com/quarylabs/sqruff/pull/{number}",
    }


class GraphTests(unittest.TestCase):
    def test_selects_tip_not_parent_and_prefers_advanced_fork(self):
        prs = [pr(1, "a"), pr(2, "b", "a"), pr(3, "old")]
        cursors = {2: "new", 3: "old"}
        self.assertEqual(
            port.select_tip(prs, lambda p: cursors[p["number"]], {"new": 0, "old": 5})[
                "number"
            ],
            2,
        )

    def test_tied_and_unknown_watermarks_fail_closed(self):
        prs = [pr(1, "a"), pr(2, "b")]
        for history in [{"same": 0}, {}]:
            with self.assertRaises(port.PortError):
                port.select_tip(prs, lambda _: "same", history)

    def test_cycle_fails_closed(self):
        with self.assertRaises(port.PortError):
            port.select_tip(
                [pr(1, "a", "b"), pr(2, "b", "a")], lambda _: "sha", {"sha": 0}
            )

    def test_duplicate_identity_not_numeric_prefix(self):
        state = {
            "branch": "port/sqlfluff-123",
            "next_sha": "a" * 40,
            "upstream_pr": 123,
        }
        self.assertTrue(
            port.exact_port(
                pr(1, "x", body="https://github.com/sqlfluff/sqlfluff/pull/123"), state
            )
        )
        self.assertFalse(
            port.exact_port(
                pr(1, "x", body="https://github.com/sqlfluff/sqlfluff/pull/1234"), state
            )
        )
        self.assertTrue(port.exact_port(pr(1, "port/sqlfluff-123"), state))
        self.assertFalse(
            port.exact_port(pr(1, "x", title="Unrelated issue 123"), state)
        )

    def test_open_ports_includes_all_pages_and_body_identification(self):
        with patch.object(
            port,
            "pages",
            return_value=[
                [pr(1, "port/sqlfluff-1")],
                [pr(2, "legacy", body="Ported from SQLFluff")],
                [pr(3, "other", title="Unrelated")],
            ],
        ):
            self.assertEqual([p["number"] for p in port.open_ports()], [1, 2])

    def test_incomplete_duplicate_search_fails_closed(self):
        state = {
            "branch": "port/sqlfluff-123",
            "next_sha": "a" * 40,
            "upstream_pr": None,
        }
        with (
            patch.object(
                port, "pages", return_value=[{"total_count": 1001, "items": []}]
            ),
            self.assertRaises(port.PortError),
        ):
            port.duplicates(state)

    def test_changed_remote_base_prevents_publication(self):
        state = {
            "root": "/tmp",
            "limit": 50,
            "base_sha": "old",
            "base_branch": "branch",
            "cursor": "cursor",
            "next_sha": "next",
        }
        with (
            patch.object(port, "snapshot", return_value={**state, "base_sha": "new"}),
            patch.object(port, "duplicates", return_value=[]),
            self.assertRaisesRegex(port.PortError, "Stack changed"),
        ):
            port.recheck(state)


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "repo"
        self.root.mkdir()
        self.checkpoint = Path(self.temporary.name) / "state.json"
        self.summary = Path(self.temporary.name) / "summary.md"
        port.git(self.root, "init", "-q")
        port.git(self.root, "config", "user.email", "test@example.invalid")
        port.git(self.root, "config", "user.name", "Test")
        port.git(self.root, "config", "commit.gpgsign", "false")
        (self.root / ".sqlfluff-sha").write_text("a" * 40 + "\n")
        port.git(self.root, "add", ".sqlfluff-sha")
        port.git(self.root, "commit", "-qm", "base")
        self.base = port.git(self.root, "rev-parse", "HEAD")
        port.git(self.root, "switch", "-qc", "port/sqlfluff-123")
        (self.root / ".sqlfluff-sha").write_text("b" * 40 + "\n")
        port.git(self.root, "add", ".sqlfluff-sha")
        self.state = {
            "root": str(self.root),
            "branch": "port/sqlfluff-123",
            "next_sha": "b" * 40,
            "base_sha": self.base,
            "base_branch": "main",
            "base_pr": None,
            "limit": 50,
            "upstream_pr": 123,
            "chain": [],
            "status": "ready",
        }
        self.args = argparse.Namespace(
            state=str(self.checkpoint), summary=str(self.summary)
        )
        self.summary.write_text("- Literal `cargo test` and $(do-not-execute)\n")
        self.real_run = subprocess.run

    def builds(self, fail=None):
        def execute(command, **kwargs):
            if command[0] in ("cargo", "bazel"):
                return subprocess.CompletedProcess(command, 1 if command == fail else 0)
            return self.real_run(command, **kwargs)

        return patch.object(port.subprocess, "run", side_effect=execute)

    def validated_commit(self):
        with self.builds():
            port.verify(self.args, self.state)
        port.git(self.root, "commit", "-qm", "chore: record upstream (123)")

    def test_success_records_all_checks_and_matches_committed_tree(self):
        self.validated_commit()
        saved = json.loads(self.checkpoint.read_text())
        self.assertEqual(
            saved["validated_tree"], port.git(self.root, "rev-parse", "HEAD^{tree}")
        )
        self.assertEqual(
            [t["command"] for t in saved["tests"]],
            [
                ["cargo", "fmt", "--all", "--", "--check"],
                ["cargo", "build"],
                ["cargo", "test"],
                ["bazel", "test", "//..."],
            ],
        )

    def test_failed_test_invalidates_previous_validation(self):
        self.state["validated_tree"] = "stale"
        with self.builds(["cargo", "test"]), self.assertRaises(port.PortError):
            port.verify(self.args, self.state)
        saved = json.loads(self.checkpoint.read_text())
        self.assertNotIn("validated_tree", saved)
        self.assertEqual(saved["tests"][-1]["exit_code"], 1)

    def test_untracked_work_refused(self):
        (self.root / "unrelated.txt").write_text("preserve me")
        with self.assertRaises(port.PortError):
            port.staged_tree(self.state)

    def test_changed_tree_cannot_publish(self):
        self.validated_commit()
        (self.root / "extra").write_text("not tested")
        port.git(self.root, "add", "extra")
        port.git(self.root, "commit", "--amend", "--no-edit", "-q")
        with (
            patch.object(port, "recheck") as check,
            self.assertRaisesRegex(port.PortError, "not validated"),
        ):
            port.publish(self.args, self.state)
        check.assert_not_called()

    def test_publish_uses_literal_body_file_and_checkpoints_success(self):
        self.validated_commit()
        calls = []
        real = port.run

        def execute(*args, **kwargs):
            if args[:2] == ("git", "ls-remote"):
                return ""
            if args[:2] == ("git", "push"):
                calls.append("push")
                return ""
            if args[:3] == ("gh", "pr", "create"):
                calls.append("create")
                body = Path(args[args.index("--body-file") + 1]).read_text()
                self.assertIn("`cargo test` and $(do-not-execute)", body)
                return "https://github.com/quarylabs/sqruff/pull/42"
            return real(*args, **kwargs)

        with (
            patch.object(port, "recheck", return_value={"chain": []}),
            patch.object(port, "run", side_effect=execute),
        ):
            result = port.publish(self.args, self.state)
        self.assertEqual(calls, ["push", "create"])
        self.assertEqual(result["chain"], [42])
        self.assertEqual(json.loads(self.checkpoint.read_text())["status"], "published")

    def test_existing_remote_branch_is_not_overwritten(self):
        self.validated_commit()
        real = port.run

        def execute(*args, **kwargs):
            if args[:2] == ("git", "ls-remote"):
                return "c" * 40 + "\trefs/heads/port/sqlfluff-123"
            if args[:2] == ("git", "push"):
                self.fail("Must not push over an existing branch")
            return real(*args, **kwargs)

        with (
            patch.object(port, "recheck", return_value={"chain": []}),
            patch.object(port, "run", side_effect=execute),
            self.assertRaisesRegex(port.PortError, "Remote branch already exists"),
        ):
            port.publish(self.args, self.state)

    def test_ambiguous_create_failure_preserves_pushed_checkpoint(self):
        self.validated_commit()
        real = port.run

        def execute(*args, **kwargs):
            if args[:2] in [("git", "ls-remote"), ("git", "push")]:
                return ""
            if args[:3] == ("gh", "pr", "create"):
                raise port.PortError("Timed out; remote may have succeeded")
            return real(*args, **kwargs)

        with (
            patch.object(port, "recheck", return_value={"chain": []}),
            patch.object(port, "run", side_effect=execute),
            self.assertRaises(port.PortError),
        ):
            port.publish(self.args, self.state)
        saved = json.loads(self.checkpoint.read_text())
        self.assertEqual(saved["status"], "pushed")
        self.assertEqual(saved["commit"], port.git(self.root, "rev-parse", "HEAD"))


if __name__ == "__main__":
    unittest.main()
