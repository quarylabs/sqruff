#!/usr/bin/env python3
"""
Script to catch up sqruff dialect fixtures with sqlfluff dialect fixtures.

The way that it works is that it takes a dialect name as an argument. Both
`sqlfluff` and `sqruff` have a fixtures folder. The script will look for the first
commit in `sqlfluff` where sqlfluff moved ahead of sqruff's current state.
It does this by finding the last commit where sqlfluff matched sqruff's current state,
then returns the next commit after that.

By a fixture being different or a file missing it doesn't mean the yaml files that are used to
output the ast to check it but only the sql files.

As well as find that first commit, it can also copy the files from that specific commit.
As such it can be used to catch up sqruff dialect fixtures with sqlfluff dialect fixtures and implement
the changes in the sqlfluff dialects commit by commit.
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path
from typing import List, Optional, Tuple
from claude_code_sdk import query, ClaudeCodeOptions
import anyio


def normalize_sql_content(content: str) -> str:
    """Return content with all whitespace at beginning or end removed."""
    return content.strip()


def run_command(cmd: List[str], cwd: Optional[str] = None) -> Tuple[int, str, str]:
    """Run a command and return exit code, stdout, and stderr."""
    try:
        result = subprocess.run(
            cmd, capture_output=True, text=True, cwd=cwd, check=False
        )
        return result.returncode, result.stdout, result.stderr
    except Exception as e:
        return 1, "", str(e)


def get_sql_files(directory: Path) -> List[Path]:
    """Get all .sql files in a directory."""
    return list(directory.glob("*.sql"))


def copy_sql_files(src_dir: Path, dst_dir: Path) -> List[Path]:
    """Copy all .sql files from src_dir to dst_dir, ignoring whitespace-only changes."""
    copied_files = []
    for sql_file in get_sql_files(src_dir):
        dst_file = dst_dir / sql_file.name
        if dst_file.exists():
            with open(sql_file, "r") as src, open(dst_file, "r") as dst:
                if normalize_sql_content(src.read()) == normalize_sql_content(
                    dst.read()
                ):
                    continue
        shutil.copy2(sql_file, dst_file)
        copied_files.append(dst_file)
        print(f"Copied: {sql_file.name}")
    return copied_files


def compare_directories(sqruff_dir: Path, sqlfluff_dir: Path) -> bool:
    """
    Compare sqruff vs sqlfluff directories for differences in .sql files.
    Returns True if there are differences that indicate sqlfluff has moved ahead of sqruff.
    Whitespace-only changes in SQL files are ignored.

    Differences that matter:
    - Files in sqlfluff but not in sqruff (missing in sqruff)
    - Files in both but with different content

    Not a difference:
    - Files in sqruff but not in sqlfluff (sqruff is ahead, which is fine)
    """
    sqruff_files = {f.name: f for f in get_sql_files(sqruff_dir)}
    sqlfluff_files = {f.name: f for f in get_sql_files(sqlfluff_dir)}

    # Check for files that exist in sqlfluff but are missing in sqruff
    missing_in_sqruff = set(sqlfluff_files.keys()) - set(sqruff_files.keys())
    if missing_in_sqruff:
        print(f"Files missing in sqruff: {missing_in_sqruff}")
        return True

    # Compare content of files that exist in both. Whitespace-only differences
    # are ignored to avoid false positives when only formatting changed.
    for filename in sqlfluff_files:
        if filename in sqruff_files:
            sqlfluff_file = sqlfluff_files[filename]
            sqruff_file = sqruff_files[filename]

            try:
                with open(sqruff_file, "r") as f1, open(sqlfluff_file, "r") as f2:
                    sqruff_content = f1.read()
                    sqlfluff_content = f2.read()
                    if normalize_sql_content(sqruff_content) != normalize_sql_content(
                        sqlfluff_content
                    ):
                        print(f"Content differs for file: {filename}")
                        return True
            except Exception as e:
                print(f"Error comparing {filename}: {e}")
                return True

    return False


def find_first_difference_commit(sqlfluff_path: Path, dialect: str) -> Optional[str]:
    """Find the first commit where sqlfluff moved ahead of sqruff's current state."""
    dialect_dir = sqlfluff_path / "test" / "fixtures" / "dialects" / dialect

    if not dialect_dir.exists():
        print(f"Warning: {dialect_dir} does not exist in sqlfluff repository")
        return None

    # Save current HEAD to restore later
    head_cmd = ["git", "rev-parse", "HEAD"]
    exit_code, current_head, stderr = run_command(head_cmd, cwd=sqlfluff_path)
    if exit_code != 0:
        print(f"Error getting current HEAD: {stderr}")
        return None
    current_head = current_head.strip()

    # Get git log for the dialect directory (oldest first)
    cmd = ["git", "log", "--oneline", "--reverse", "--", str(dialect_dir)]
    exit_code, stdout, stderr = run_command(cmd, cwd=sqlfluff_path)

    if exit_code != 0:
        print(f"Error getting git log: {stderr}")
        return None

    commits = stdout.strip().split("\n")
    if not commits or commits[0] == "":
        print(f"No commits found for {dialect} directory")
        return None

    sqruff_dialect_dir = Path("crates/lib-dialects/test/fixtures/dialects") / dialect

    try:
        last_matching_commit = None

        # Check each commit from oldest to newest to find the last commit that matches sqruff
        for i, commit_line in enumerate(commits):
            if not commit_line.strip():
                continue

            commit_hash = commit_line.split()[0]

            # Create a temporary directory to check out the specific commit
            temp_dir = Path(f"/tmp/sqlfluff_checkout_{commit_hash}")
            if temp_dir.exists():
                shutil.rmtree(temp_dir)
            temp_dir.mkdir(parents=True)

            # Use git show to get the files at this commit
            show_cmd = [
                "git",
                "show",
                f"{commit_hash}:{dialect_dir.relative_to(sqlfluff_path)}",
            ]
            exit_code, _, stderr = run_command(show_cmd, cwd=sqlfluff_path)

            if exit_code != 0:
                # This commit might not have the dialect directory yet
                continue

            # Extract files from this commit to temp directory
            extract_cmd = [
                "git",
                "archive",
                commit_hash,
                f"test/fixtures/dialects/{dialect}",
            ]
            extract_process = subprocess.run(
                extract_cmd, cwd=sqlfluff_path, capture_output=True
            )

            if extract_process.returncode != 0:
                continue

            # Extract the archive to temp directory
            extract_archive_cmd = ["tar", "-xf", "-", "-C", str(temp_dir)]
            tar_process = subprocess.run(
                extract_archive_cmd, input=extract_process.stdout, capture_output=True
            )

            if tar_process.returncode != 0:
                continue

            commit_dialect_dir = temp_dir / "test" / "fixtures" / "dialects" / dialect

            if not commit_dialect_dir.exists():
                continue

            # Compare with current sqruff fixtures
            if not compare_directories(sqruff_dialect_dir, commit_dialect_dir):
                # This commit matches sqruff's current state
                last_matching_commit = i
                print(f"Found matching commit: {commit_hash} - {commit_line}")

            # Clean up temp directory
            try:
                shutil.rmtree(temp_dir)
            except Exception as e:
                print(f"Warning: Could not clean up temp directory {temp_dir}: {e}")

        # If we found a matching commit, return the next commit after it
        if last_matching_commit is not None and last_matching_commit + 1 < len(commits):
            next_commit_line = commits[last_matching_commit + 1]
            next_commit_hash = next_commit_line.split()[0]
            print(f"Found first commit where sqlfluff moved ahead: {next_commit_hash}")
            print(f"Commit message: {next_commit_line}")
            return next_commit_hash
        elif last_matching_commit is not None:
            print("sqruff is up to date with sqlfluff")
            return None
        else:
            print(
                "No matching commits found - sqruff and sqlfluff have completely diverged"
            )
            return commits[0].split()[0]  # Return first commit

    finally:
        # Always restore the original HEAD
        restore_cmd = ["git", "checkout", current_head]
        exit_code, _, stderr = run_command(restore_cmd, cwd=sqlfluff_path)
        if exit_code != 0:
            print(f"Warning: Could not restore git HEAD to {current_head}: {stderr}")


def copy_from_commit(sqlfluff_path: Path, dialect: str, commit_hash: str) -> List[Path]:
    """Copy SQL files from a specific commit in sqlfluff to sqruff."""
    sqruff_dialect_dir = Path("crates/lib-dialects/test/fixtures/dialects") / dialect

    if not sqruff_dialect_dir.exists():
        print(f"Error: sqruff dialect directory {sqruff_dialect_dir} does not exist")
        return []

    # Get the list of SQL files that were changed in this commit
    cmd = ["git", "show", "--name-only", commit_hash]
    exit_code, stdout, stderr = run_command(cmd, cwd=sqlfluff_path)

    if exit_code != 0:
        print(f"Error getting files from commit {commit_hash}: {stderr}")
        return []

    # Filter for SQL files in the dialect directory
    changed_files = stdout.strip().split("\n")
    sql_files_to_copy = []
    for file_path in changed_files:
        if (
            file_path.endswith(".sql")
            and f"test/fixtures/dialects/{dialect}/" in file_path
        ):
            sql_files_to_copy.append(Path(file_path).name)

    if not sql_files_to_copy:
        print(
            f"No SQL files were changed in commit {commit_hash} for dialect {dialect}"
        )
        return []

    print(f"SQL files changed in commit {commit_hash}: {sql_files_to_copy}")

    # Create a temporary directory to extract files from the commit
    temp_dir = Path(f"/tmp/sqlfluff_copy_{commit_hash}")
    if temp_dir.exists():
        shutil.rmtree(temp_dir)
    temp_dir.mkdir(parents=True)

    try:
        # Extract files from this commit to temp directory
        extract_cmd = [
            "git",
            "archive",
            commit_hash,
            f"test/fixtures/dialects/{dialect}",
        ]
        extract_process = subprocess.run(
            extract_cmd, cwd=sqlfluff_path, capture_output=True
        )

        if extract_process.returncode != 0:
            print(
                f"Error extracting files from commit {commit_hash}: {extract_process.stderr}"
            )
            return []

        # Extract the archive to temp directory
        extract_archive_cmd = ["tar", "-xf", "-", "-C", str(temp_dir)]
        tar_process = subprocess.run(
            extract_archive_cmd, input=extract_process.stdout, capture_output=True
        )

        if tar_process.returncode != 0:
            print(f"Error extracting archive: {tar_process.stderr}")
            return []

        commit_dialect_dir = temp_dir / "test" / "fixtures" / "dialects" / dialect

        if not commit_dialect_dir.exists():
            print(f"Error: dialect directory not found in commit {commit_hash}")
            return []

        # Copy only the SQL files that were changed. Skip files where the change
        # is only whitespace.
        copied_files = []
        for sql_file_name in sql_files_to_copy:
            src_file = commit_dialect_dir / sql_file_name
            dst_file = sqruff_dialect_dir / sql_file_name

            if src_file.exists():
                if dst_file.exists():
                    with open(src_file, "r") as src, open(dst_file, "r") as dst:
                        if normalize_sql_content(src.read()) == normalize_sql_content(
                            dst.read()
                        ):
                            print(f"Skipped {sql_file_name} (whitespace only changes)")
                            continue
                shutil.copy2(src_file, dst_file)
                copied_files.append(dst_file)
                print(f"Copied: {sql_file_name}")

        return copied_files

    finally:
        # Clean up temp directory
        if temp_dir.exists():
            try:
                shutil.rmtree(temp_dir)
            except Exception as e:
                print(f"Warning: Could not clean up temp directory {temp_dir}: {e}")


async def main():
    parser = argparse.ArgumentParser(
        description="Compare and sync dialect fixtures between sqruff and sqlfluff"
    )
    parser.add_argument("sqlfluff_path", help="Path to the sqlfluff repository")
    parser.add_argument(
        "dialect", help="Dialect name (e.g., clickhouse, postgres, etc.)"
    )
    parser.add_argument(
        "--copy-only",
        action="store_true",
        help="Only copy SQL files, don't find earliest commit",
    )
    parser.add_argument(
        "--find-commit-only",
        action="store_true",
        help="Only find earliest commit, don't copy files",
    )
    parser.add_argument(
        "--copy-from-commit", help="Copy SQL files from a specific commit hash"
    )
    parser.add_argument(
        "--claude-attempt-change",
        action="store_true",
        help="Attempt to implement the change in the sqlfluff commit in the sqruff dialect with claude",
    )

    args = parser.parse_args()

    # Validate paths
    sqlfluff_path = Path(args.sqlfluff_path)
    if not sqlfluff_path.exists():
        print(f"Error: sqlfluff path {sqlfluff_path} does not exist")
        sys.exit(1)

    sqruff_dialect_dir = (
        Path("crates/lib-dialects/test/fixtures/dialects") / args.dialect
    )
    if not sqruff_dialect_dir.exists():
        print(f"Error: sqruff dialect directory {sqruff_dialect_dir} does not exist")
        sys.exit(1)

    sqlfluff_dialect_dir = (
        sqlfluff_path / "test" / "fixtures" / "dialects" / args.dialect
    )
    if not sqlfluff_dialect_dir.exists():
        print(
            f"Error: sqlfluff dialect directory {sqlfluff_dialect_dir} does not exist"
        )
        sys.exit(1)

    print(f"Comparing {args.dialect} dialect fixtures...")
    print(f"sqruff: {sqruff_dialect_dir}")
    print(f"sqlfluff: {sqlfluff_dialect_dir}")
    print()

    # Handle copy from specific commit
    if args.copy_from_commit:
        print(f"Copying SQL files from commit {args.copy_from_commit}...")
        copied_files = copy_from_commit(
            sqlfluff_path, args.dialect, args.copy_from_commit
        )
        print(
            f"Copied {len(copied_files)} SQL files from commit {args.copy_from_commit}"
        )
        print()
        return

    # Find first difference commit (needed for copy operations and when explicitly requested)
    first_difference_commit = None
    if not args.find_commit_only:
        # We need to find the commit when copying (unless copying from current state)
        print("Finding first commit where fixtures differ...")
        first_difference_commit = find_first_difference_commit(
            sqlfluff_path, args.dialect
        )
        if first_difference_commit:
            print(f"First difference commit: {first_difference_commit}")
        print()

    # Copy SQL files if requested
    if not args.find_commit_only:
        if first_difference_commit:
            print(f"Copying SQL files from commit {first_difference_commit}...")
            copied_files = copy_from_commit(
                sqlfluff_path, args.dialect, first_difference_commit
            )
            print(
                f"Copied {len(copied_files)} SQL files from commit {first_difference_commit}"
            )
        else:
            print("Copying SQL files from current sqlfluff state...")
            copied_files = copy_sql_files(sqlfluff_dialect_dir, sqruff_dialect_dir)
            print(f"Copied {len(copied_files)} SQL files")
        print()

    # Show commit info if only finding commit
    if args.find_commit_only and not first_difference_commit:
        print("Finding first commit where fixtures differ...")
        first_difference_commit = find_first_difference_commit(
            sqlfluff_path, args.dialect
        )
        if first_difference_commit:
            print(f"First difference commit: {first_difference_commit}")
        print()

    # Attempt to implement the change in the sqlfluff commit in the sqruff dialect with claude
    if args.claude_attempt_change:
        # Get the sqlfluff commit
        commit_hash = first_difference_commit
        if not commit_hash:
            print("No commit found to implement changes from")
            return
        print(f"Fetching changes from SQLFluff commit {commit_hash}...")

        # Get the diff for this commit, excluding test fixture files
        cmd = ["git", "show", "--format=", commit_hash, "--", "src/sqlfluff/dialects/"]
        exit_code, diff_output, stderr = run_command(cmd, cwd=sqlfluff_path)

        if exit_code != 0:
            print(f"Error getting commit diff: {stderr}")
            return

        # Filter out fixture changes and extract only dialect implementation changes
        relevant_changes = []
        current_file_diff = []
        in_relevant_file = False

        for line in diff_output.split("\n"):
            if line.startswith("diff --git"):
                # Save previous file diff if it was relevant
                if in_relevant_file and current_file_diff:
                    relevant_changes.extend(current_file_diff)
                current_file_diff = [line]
                # Check if this is a dialect implementation file (not a test fixture)
                in_relevant_file = (
                    "/dialects/" in line and "/test/" not in line and ".py" in line
                )
            elif in_relevant_file:
                current_file_diff.append(line)

        # Don't forget the last file
        if in_relevant_file and current_file_diff:
            relevant_changes.extend(current_file_diff)

        if not relevant_changes:
            print("No relevant dialect implementation changes found in this commit")
            return

        # Get commit message for context
        cmd = ["git", "show", "--format=%B", "-s", commit_hash]
        exit_code, commit_message, stderr = run_command(cmd, cwd=str(sqlfluff_path))

        if exit_code == 0:
            commit_message = commit_message.strip()
        else:
            commit_message = "Could not retrieve commit message"

        # Join the relevant changes back into a diff string
        dialect_changes = "\n".join(relevant_changes)

        print(f"Found {len(relevant_changes)} lines of dialect implementation changes")
        print(f"Commit message: {commit_message}")

        # Prepare the prompt for Claude
        prompt = f"""The following changes were made to SQLFluff dialects in commit {commit_hash}:

Commit message: {commit_message}

Dialect implementation changes:
```diff
{dialect_changes}
```

Please analyze these SQLFluff dialect changes and implement the equivalent changes in the Sqruff Rust dialect implementation for the {args.dialect} dialect. 

The Sqruff dialect files are located in the current directory. Look for the appropriate Rust files that correspond to the SQLFluff Python changes and apply similar modifications while adapting them to Rust patterns and Sqruff's architecture.

Key considerations:
1. SQLFluff uses Python while Sqruff uses Rust
2. Grammar definitions may have different syntax but should achieve the same parsing behavior
3. Keyword additions/modifications should be reflected in the Rust dialect
4. Look for files like `{args.dialect}.rs` or related modules in the current directory

Please implement these changes in the Sqruff dialect."""

        print(
            f"Attempting to implement the change in the sqlfluff commit {args.claude_attempt_change} in the sqruff dialect with claude..."
        )
        async for message in query(options=options, prompt=prompt):
            print(message)

    print("Done!")


options = ClaudeCodeOptions(
    max_turns=30,
    cwd=(Path(__file__).parent.parent.parent / "crates" / "lib-dialects"),
    allowed_tools=["Read", "Write"],
    permission_mode="acceptEdits",
)


anyio.run(main)
