#!/usr/bin/env python3
"""Small, checkpointed helpers for next-sqlfluff-optimized.md (stdlib only)."""

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

REPO = "quarylabs/sqruff"
UPSTREAM = "https://github.com/sqlfluff/sqlfluff"


class PortError(Exception):
    pass


def run(*args, cwd=None, timeout=120):
    """No shell interpolation; never automatically retry a remote mutation."""
    try:
        result = subprocess.run(
            args, cwd=cwd, text=True, capture_output=True, timeout=timeout, check=False
        )
    except subprocess.TimeoutExpired as exc:
        raise PortError(
            f"Timed out after {timeout}s: {args[0:3]}. "
            "A remote write may have succeeded; inspect remote state before retrying."
        ) from exc
    if result.returncode:
        raise PortError(f"Command failed: {args[0:3]}\n{result.stderr[-4000:]}")
    return result.stdout.strip()


def git(root, *args):
    return run("git", *args, cwd=root)


def save(path, state):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as out:
        json.dump(state, out, indent=2)
        out.write("\n")
        temporary = Path(out.name)
    temporary.replace(path)


def pages(endpoint):
    return json.loads(run("gh", "api", "--paginate", "--slurp", endpoint))


def open_ports():
    prs = [
        pr
        for page in pages(f"repos/{REPO}/pulls?state=open&per_page=100")
        for pr in page
    ]
    return [
        pr
        for pr in prs
        if pr["head"]["ref"].startswith("port/sqlfluff-")
        or re.search(
            r"sqlfluff", pr["title"] + "\n" + (pr.get("body") or ""), re.IGNORECASE
        )
    ]


def select_tip(prs, cursor, history):
    """Select only a tip on upstream first-parent history; reject tied forks."""
    if not prs:
        return None
    bases = {pr["base"]["ref"] for pr in prs}
    tips = [pr for pr in prs if pr["head"]["ref"] not in bases]
    if not tips:
        raise PortError("Port PR graph has no tip; reconcile the stack.")
    ranked = []
    for pr in tips:
        sha = cursor(pr)
        if sha not in history:
            raise PortError(
                f"PR #{pr['number']} watermark is not on upstream first-parent history."
            )
        ranked.append((history[sha], pr))
    ranked.sort(key=lambda item: item[0])  # newest-first history
    if len(ranked) > 1 and ranked[0][0] == ranked[1][0]:
        raise PortError(
            "Multiple stack tips have the same upstream watermark; reconcile them."
        )
    return ranked[0][1]


def exact_port(pr, state):
    head = pr.get("head", {}).get("ref", pr.get("headRefName", ""))
    text = pr.get("title", "") + "\n" + (pr.get("body") or "")
    if head == state["branch"] or re.search(
        rf"(?<![0-9a-f]){state['next_sha']}(?![0-9a-f])", text
    ):
        return True
    number = state["upstream_pr"]
    return bool(
        number
        and (
            re.search(rf"github\.com/sqlfluff/sqlfluff/pull/{number}(?!\d)", text)
            or re.search(rf"\(#{number}\)|\({number}\)", pr.get("title", ""))
        )
    )


def duplicates(state):
    queries = [state["next_sha"]]
    if state["upstream_pr"]:
        queries.append(str(state["upstream_pr"]))

    def search(query):
        # Search has a 1000-result ceiling; never treat truncated results as complete.
        from urllib.parse import urlencode

        endpoint = "search/issues?" + urlencode(
            {"q": f"repo:{REPO} is:pr {query} in:title,body", "per_page": 100}
        )
        result = pages(endpoint)
        if any(p.get("incomplete_results") or p["total_count"] > 1000 for p in result):
            raise PortError("Duplicate search incomplete; resolve before proceeding.")
        return [pr for page in result for pr in page["items"]]

    with ThreadPoolExecutor(max_workers=3) as pool:
        found = [pr for result in pool.map(search, queries) for pr in result]
    found.extend(
        pr
        for page in pages(
            f"repos/{REPO}/pulls?state=all&head=quarylabs:{state['branch']}&per_page=100"
        )
        for pr in page
    )
    return sorted({pr["html_url"] for pr in found if exact_port(pr, state)})


def snapshot(root, limit):
    git(root, "fetch", "origin", "--prune")
    prs = open_ports()
    if limit and len(prs) >= limit:
        raise PortError(f"Capacity exhausted: {len(prs)} open ports, limit {limit}.")
    upstream = root / "sqlfluff"
    if not upstream.exists():
        run("git", "clone", f"{UPSTREAM}.git", str(upstream), timeout=300)
    git(upstream, "fetch", "origin", "main")
    upstream_tip = git(upstream, "rev-parse", "origin/main")
    commits = git(upstream, "rev-list", "--first-parent", upstream_tip).splitlines()
    history = {sha: index for index, sha in enumerate(commits)}
    tip = select_tip(
        prs, lambda pr: git(root, "show", f"{pr['head']['sha']}:.sqlfluff-sha"), history
    )
    base_branch = tip["head"]["ref"] if tip else "main"
    base_sha = tip["head"]["sha"] if tip else git(root, "rev-parse", "origin/main")
    if git(root, "rev-parse", f"origin/{base_branch}") != base_sha:
        raise PortError("PR listing and fetched branch disagree; rerun preflight.")
    cursor = git(root, "show", f"{base_sha}:.sqlfluff-sha")
    if cursor not in history:
        raise PortError("Selected watermark is not on upstream first-parent history.")
    index = history[cursor]
    next_sha = commits[index - 1] if index else None
    chain = []
    node = tip
    heads = {pr["head"]["ref"]: pr for pr in prs}
    seen = set()
    while node:
        if node["number"] in seen:
            raise PortError("Cycle in port PR stack.")
        seen.add(node["number"])
        chain.append(node["number"])
        node = heads.get(node["base"]["ref"])
    return {
        "root": str(root),
        "limit": limit,
        "base_branch": base_branch,
        "base_sha": base_sha,
        "base_pr": tip["number"] if tip else None,
        "cursor": cursor,
        "next_sha": next_sha,
        "upstream_tip": upstream_tip,
        "remaining": index,
        "open_ports": len(prs),
        "chain": chain[::-1],
        "other_ports": sorted(pr["number"] for pr in prs if pr["number"] not in seen),
    }


def preflight(args):
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel"))
    if git(root, "status", "--porcelain"):
        raise PortError("Worktree is dirty; preserve it and stop.")
    if Path(args.state).exists():
        raise PortError("Checkpoint exists. Inspect/resume it or choose a new path.")
    state = snapshot(root, args.limit)
    state["status"] = "caught_up"
    if state["next_sha"]:
        subject = git(root / "sqlfluff", "show", "-s", "--format=%s", state["next_sha"])
        match = re.search(r"\(#(\d+)\)", subject)
        number = int(match[1]) if match else None
        state.update(
            subject=subject,
            upstream_pr=number,
            branch=f"port/sqlfluff-{number or state['next_sha'][:12]}",
        )
        state["duplicates"] = duplicates(state)
        state["status"] = "duplicate" if state["duplicates"] else "ready"
    save(args.state, state)
    return state


def recheck(state):
    fresh = snapshot(Path(state["root"]), state["limit"])
    found = duplicates(state)
    if found:
        raise PortError(
            f"Exact port already exists: {found}. Do not create another PR."
        )
    for key in ("base_sha", "base_branch", "cursor", "next_sha"):
        if fresh[key] != state[key]:
            raise PortError(
                f"Stack changed ({key}); reconcile/rebase and revalidate before publication."
            )
    return fresh


def staged_tree(state):
    root = Path(state["root"])
    if git(root, "branch", "--show-current") != state["branch"]:
        raise PortError("Not on the planned port branch.")
    if git(root, "diff") or git(root, "ls-files", "--others", "--exclude-standard"):
        raise PortError(
            "Stage the exact intended files; unstaged/untracked work exists."
        )
    if git(root, "show", ":.sqlfluff-sha") != state["next_sha"]:
        raise PortError("Staged watermark does not match the planned next commit.")
    git(root, "diff", "--cached", "--check")
    return git(root, "write-tree")


def verify(args, state):
    tree = staged_tree(state)
    root = Path(state["root"])
    state.pop("validated_tree", None)
    state["status"] = "testing"
    state["tests"] = []
    save(args.state, state)
    for command in [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "build"],
        ["cargo", "test"],
        ["bazel", "test", "//..."],
    ]:
        start = time.monotonic()
        print("Running " + " ".join(command), file=sys.stderr, flush=True)
        # Stream output to stderr; stdout remains one compact machine-readable result.
        result = subprocess.run(
            command, cwd=root, stdout=sys.stderr, stderr=sys.stderr, check=False
        )
        state["tests"].append(
            {
                "command": command,
                "seconds": round(time.monotonic() - start, 2),
                "exit_code": result.returncode,
            }
        )
        save(args.state, state)
        if result.returncode:
            raise PortError("Validation failed; fix the failure and rerun verify.")
    if staged_tree(state) != tree:
        raise PortError("Tree changed during validation; rerun verify.")
    state.update(validated_tree=tree, status="validated")
    save(args.state, state)
    return {"status": state["status"], "tree": tree, "tests": state["tests"]}


def publish(args, state):
    root = Path(state["root"])
    if git(root, "status", "--porcelain"):
        raise PortError(
            "Commit the validated changes before publishing; worktree must be clean."
        )
    if staged_tree(state) != state.get("validated_tree"):
        raise PortError("Current tree was not validated.")
    if git(root, "rev-list", "--parents", "-n", "1", "HEAD").split()[1:] != [
        state["base_sha"]
    ]:
        raise PortError("Expected exactly one commit directly above the recorded base.")
    # Read body as data, never as shell code. Freeze it before performing remote writes.
    summary = Path(args.summary).read_text().strip()
    if not summary:
        raise PortError("Summary file is empty.")
    title = git(root, "show", "-s", "--format=%s", "HEAD")
    head = git(root, "rev-parse", "HEAD")
    fresh = recheck(state)
    remote = git(
        root, "ls-remote", "--heads", "origin", f"refs/heads/{state['branch']}"
    )
    if remote and not (
        remote.split()[0] == head
        and state.get("commit") == head
        and state["status"] in ("publishing", "pushed")
    ):
        raise PortError(
            "Remote branch already exists outside this checkpoint; reconcile it."
        )
    state["chain"] = fresh["chain"]
    state.update(status="publishing", commit=head)
    save(args.state, state)
    prefix = ""
    if state["base_pr"]:
        number = state["base_pr"]
        prefix = f"> Stacked on sqruff #{number}. Merge #{number} first.\n> https://github.com/{REPO}/pull/{number}\n\n"
    body = (
        prefix
        + "## Summary\n"
        + summary
        + f"\n\nPorted from SQLFluff {state['next_sha']}\n"
    )
    if state["upstream_pr"]:
        body += f"{UPSTREAM}/pull/{state['upstream_pr']}\n"
    body += f"{UPSTREAM}/commit/{state['next_sha']}\n\n## Validation\n"
    body += (
        "\n".join("- `" + " ".join(test["command"]) + "`" for test in state["tests"])
        + "\n"
    )
    state["body_sha256"] = hashlib.sha256(body.encode()).hexdigest()
    save(args.state, state)
    run("git", "push", "-u", "origin", state["branch"], cwd=root)
    state["status"] = "pushed"
    save(args.state, state)
    with tempfile.NamedTemporaryFile(mode="w", suffix=".md") as file:
        file.write(body)
        file.flush()
        url = run(
            "gh",
            "pr",
            "create",
            "--repo",
            REPO,
            "--base",
            state["base_branch"],
            "--head",
            state["branch"],
            "--title",
            title,
            "--body-file",
            file.name,
            cwd=root,
        )
    state.update(status="published", pr_url=url)
    save(args.state, state)
    return {
        "status": "published",
        "pr_url": url,
        "base_pr": state["base_pr"],
        "chain": state["chain"] + [int(url.rsplit("/", 1)[1])],
        "next_step": "Link and verify GitHub stack UI once; do not recreate this PR.",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("preflight", "recheck", "verify", "publish"):
        command = sub.add_parser(name)
        command.add_argument(
            "--state", required=True, help="Checkpoint path outside the worktree"
        )
        if name == "preflight":
            command.add_argument(
                "--limit",
                type=int,
                default=50,
                help="Open-port limit; 0 only for an existing explicit user override",
            )
        if name == "publish":
            command.add_argument(
                "--summary", required=True, help="Markdown summary file"
            )
    args = parser.parse_args()
    try:
        root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel")).resolve()
        if Path(args.state).resolve().is_relative_to(root):
            raise PortError(
                "Keep the checkpoint outside the worktree (for example /private/tmp)."
            )
        if args.command == "preflight":
            if args.limit < 0:
                raise PortError("Limit must be nonnegative.")
            result = preflight(args)
        else:
            state = json.loads(Path(args.state).read_text())
            if Path(state["root"]).resolve() != root:
                raise PortError("Checkpoint belongs to a different worktree.")
            if state.get("status") in ("caught_up", "duplicate", "published"):
                raise PortError(
                    f"Checkpoint is {state['status']}; do not publish another port."
                )
            if args.command == "recheck":
                fresh = recheck(state)
                result = {
                    "status": "unchanged",
                    "base_sha": fresh["base_sha"],
                    "open_ports": fresh["open_ports"],
                }
            else:
                result = globals()[args.command](args, state)
        print(json.dumps(result, indent=2))
    except (PortError, OSError, ValueError, KeyError) as exc:
        print(json.dumps({"status": "blocked", "reason": str(exc)}), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
