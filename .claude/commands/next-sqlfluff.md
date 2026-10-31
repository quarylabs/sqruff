Port the next SQLFluff commit to sqruff.

## Safety limits

- Create **at most one pull request per invocation**. A user-requested loop or
  active goal may start the next invocation after publishing and auditing this
  port. Continue sequentially until caught up, the open-port limit is reached,
  or a failure, duplicate, or concurrent publisher requires reconciliation.
  Without an explicit loop request, stop after this invocation.
- Keep **at most fifty open SQLFluff port PRs** in the repository. Count the
  matching open PRs before starting any port work. If there are already fifty,
  stop without creating a branch or PR and report the existing stack.
- Every new SQLFluff port PR must extend the active PR stack. If the stack is
  empty, the new PR starts it from `main`; otherwise, the new PR targets the
  current stack tip. Never create a parallel or sibling port PR.

## Workflow

### Reuse the checkout and contain temporary files

Run every port in the same sqruff checkout. Switch branches there and reuse its
Cargo `target/`, Bazel output base, shared disk cache, and repository cache. Do
not create a new worktree, temporary clone, or Bazel `--output_base` per port.
Keep the existing `sqlfluff/` reference clone and fetch it without changing its
checkout. Run one port and one build/test invocation at a time.

Prefer `next-sqlfluff-optimized.md` and its checkpointed helper for repeated
ports. The helper confines Cargo and Bazel-test scratch files to one disposable
directory under the ignored `.cache/sqlfluff-port-tests/` and removes it on
success or failure. This includes `mktemp` directories made by test scripts.
When running validation manually, apply the same containment:

```bash
mkdir -p .cache/sqlfluff-port-tests
(
  export TMPDIR="$(mktemp -d "$PWD/.cache/sqlfluff-port-tests/run-XXXXXXXX")"
  trap 'rm -rf -- "$TMPDIR"' EXIT
  bazel test --test_env=TMPDIR --sandbox_writable_path="$TMPDIR" //...
)
```

Do not run an unwrapped `bazel test` that leaves test scratch in the system temp
folder. After an interrupted run, inspect its checkpoint and remove only that
run's scratch directory after confirming its tests have stopped. Never sweep
unrelated `/tmp` files or clear shared caches between ports.

1. **Require a clean worktree and fetch the latest refs:**

   ```bash
   git status --short
   git fetch origin --prune
   ```

   Preserve and identify existing work before switching branches. Resume it if
   it belongs to the exact next port on the selected base; otherwise stop without
   overwriting it. An explicitly authorized stash must be recorded and preserved.

2. **Choose the base branch and stack tip.** List open sqruff port PRs and inspect their head and base branches:

   ```bash
   gh pr list --repo quarylabs/sqruff --state open --limit 1000 \
     --json number,title,body,url,headRefName,baseRefName,createdAt
   ```

   Keep PRs whose head branch starts with `port/sqlfluff-` or whose title/body identifies them as SQLFluff ports.

   Count these matching open port PRs. If the count is fifty or more, stop and
   report the stack; do not perform the remaining steps.

   - If none exist, use `main` as the base branch and start from `origin/main`.
   - If port PRs exist, build their base/head relationship and select the **stack tip**: the open port PR whose head branch is not the base of another open port PR. Use that PR's head branch as the base branch. The new PR must target this branch, not `main`.
   - If historical independent port PRs make more than one tip, select the tip whose `.sqlfluff-sha` is furthest ahead of `origin/main` on SQLFluff's first-parent history. Report the pre-existing fork in the final summary; do not create another PR directly against `main`.

   Check out the exact remote base and record the base PR number and URL when stacking:

   ```bash
   git checkout --detach origin/<base-branch>
   ```

3. **Read the current SHA from the selected base**, not necessarily from `main`:

   ```bash
   git show origin/<base-branch>:.sqlfluff-sha
   ```

   This makes the next commit follow the pending port at the tip of the stack.

4. **Ensure the SQLFluff clone exists** at `sqlfluff/` (relative to project root). If it doesn't exist, clone it:

   ```bash
   git clone https://github.com/sqlfluff/sqlfluff.git sqlfluff
   ```

5. **Find the next commit** in the local SQLFluff clone at `sqlfluff/`. First make sure it's up to date, then get the next commit after the saved SHA on the main branch (first-parent only, so merge commits are followed but side-branch commits are skipped):

   ```bash
   git -C sqlfluff fetch origin main
   git -C sqlfluff log --reverse --first-parent --format="%H" <SHA>..origin/main | head -n 1
   ```

6. **Inspect the commit** — show its message and full diff:

   ```bash
   git -C sqlfluff show <next-sha>
   ```

7. **Find the associated PR** — look up the PR that introduced this commit:

   ```bash
   git -C sqlfluff log --format="%s" -n 1 <next-sha>
   ```

   Extract the PR number from the commit message (usually in the format `(#1234)`). The PR URL is `https://github.com/sqlfluff/sqlfluff/pull/<number>`.

8. **Check whether this exact port already exists.** Search both open and closed/merged sqruff PRs for the SQLFluff PR number, commit SHA, branch name, and upstream links:

   ```bash
   gh pr list --repo quarylabs/sqruff --state all \
     --search "<sqlfluff-pr-number> OR <next-sha>" \
     --json number,title,url,body,headRefName,state
   ```

   If this same upstream commit or PR was already ported, **do nothing** and report the existing sqruff PR. An unrelated open SQLFluff port is not a reason to stop; it is the base of the new stacked PR.

9. **Create a branch from the selected base** for this port, named after the upstream PR:

   ```bash
   git checkout -b port/sqlfluff-<pr-number> origin/<base-branch>
   ```

10. **Analyze the change** and determine what needs to be ported. Use this path mapping as a guide:

- `src/sqlfluff/core/` → `crates/lib-core/`
- `src/sqlfluff/core/rules/` → `crates/lib/src/rules/`
- `src/sqlfluff/core/dialects/` → `crates/lib-dialects/src/`
- `src/sqlfluff/core/parser/` → `crates/lib-core/src/parser/`
- `src/sqlfluff/core/templaters/` → `crates/lib-core/src/templaters/`
- `test/` → corresponding `#[test]` modules or test files in the relevant crate

11. **Implement the equivalent change** in sqruff's Rust codebase. Some SQLFluff commits (docs-only, CI, Python-specific tooling) may not need a code change — if so, note why and skip to step 14. Do NOT skip the Bazel test step.

12. **Check that test fixtures are ported.** If the SQLFluff commit adds or modifies test fixture files (SQL files, YAML parse trees, etc.), verify that equivalent fixtures exist in sqruff. If they don't, add them. Fixture files in SQLFluff under `test/fixtures/dialects/<dialect>/` map to `crates/lib-dialects/test/fixtures/dialects/<dialect>/sqlfluff/` in sqruff. If the change is already ported (code + fixtures both present), note that and skip to step 14. Do NOT skip the Bazel test step.

13. **Update dialect fixture expectations if needed.** If new SQL fixtures need accompanying `.yml` parse trees, or the port intentionally changes existing parse trees, regenerate them from the project root using Cargo:

    ```bash
    UPDATE_EXPECT=1 SQRUFF_TEST_DIALECT=<dialect> cargo test -p sqruff-lib-dialects --test dialects
    ```

    Replace `<dialect>` with the affected dialect (for example, `postgres`). Repeat for each affected dialect, or omit `SQRUFF_TEST_DIALECT` to regenerate all dialect fixtures when a shared grammar change affects multiple dialects. This writes the expected `.yml` files alongside the `.sql` files under `crates/lib-dialects/test/fixtures/dialects/`.

    Review the generated fixture diffs and keep only changes justified by the port. Fix unexpected parse results before accepting new expectations. Then run the mandatory Bazel tests below without `UPDATE_EXPECT` to verify the updated fixtures.

14. **Verify the change builds and passes Bazel tests** to ensure nothing is broken. This step is MANDATORY even if no code changes were made (skip-only SHA updates still need a green CI):

    ```bash
    # Use the disposable TMPDIR wrapper above, or the optimized helper.
    bazel test --test_env=TMPDIR --sandbox_writable_path="$TMPDIR" //...
    ```

    Fix any failures before proceeding.

15. **Update `.sqlfluff-sha`** with the newly ported commit SHA.

16. **Commit the port.** Use a conventional commit prefix (`feat:` for new features/rules/dialects, `fix:` for bug fixes, `chore:` for refactors/docs/tests/cleanup). Format:

    **Commit message / PR title:**

    ```
    <prefix>: <short description of what was ported> (<sqlfluff-pr-number>)
    ```

    **Commit body / PR body:**

    ```
    ## Summary
    - <1-3 bullet points describing what was ported and any decisions made>

    Ported from SQLFluff <commit-sha>
    https://github.com/sqlfluff/sqlfluff/pull/<pr-number>
    https://github.com/sqlfluff/sqlfluff/commit/<commit-sha>
    ```

17. **Recheck the stack immediately before publishing.** Fetch again and repeat steps 2 and 8. This closes the race where another run creates a port while this run is testing.

    - If this exact port now exists, do not push or create a duplicate PR. Report the existing PR and stop.
    - If another port PR became the new stack tip, rebase this branch onto that tip, resolve `.sqlfluff-sha` so the commits advance in upstream order, rerun affected tests, and use the new tip as the PR base.
    - Never publish a sibling SQLFluff port PR against `main` while an open port stack exists.

18. **Push, create the PR, and add it to the GitHub stack:**

    ```bash
    git push -u origin port/sqlfluff-<pr-number>
    gh pr create --base <base-branch> --head port/sqlfluff-<pr-number>
    ```

    After creating the PR, use `gh stack` to represent the same dependency
    chain in GitHub's stack UI. If the existing port PRs already belong to a
    GitHub stack, append the new PR to that stack:

    ```bash
    gh stack link <stack-number> <new-sqruff-pr-number>
    ```

    If no GitHub stack exists yet, link all open port PRs in bottom-to-top
    merge order, ending with the new PR:

    ```bash
    gh stack link <bottom-pr-number> ... <new-sqruff-pr-number>
    ```

    `gh stack` supplements the PR base branches; it does not replace setting
    the new PR's base to the previous stack tip. Verify the resulting order
    with `gh stack view`. If the extension is unavailable or linking fails,
    leave the correctly based PR in place and report that the GitHub stack UI
    still needs to be updated.

    When stacking, begin the PR body with:

    ```markdown
    > Stacked on sqruff #<base-pr-number>. Merge #<base-pr-number> first.
    ```

    Include the base PR URL. Ensure the PR diff contains only this port relative to its base branch.

19. **Summarize and checkpoint this port.** Report the new PR, base, merge order,
    tests, watermark, and decisions. For an explicitly requested loop, recheck
    capacity and the current stack tip, then begin the next invocation in this
    same checkout with a new checkpoint. Preserve the existing fifty-PR limit.
    Otherwise stop. After a lower PR merges, rebase or retarget its immediate
    child onto the updated `main` before merging the child if GitHub does not do
    so automatically. A porting loop does not authorize merging PRs.
