Port the next SQLFluff commit to sqruff.

## Workflow

1. **Require a clean worktree and fetch the latest refs:**

   ```bash
   git status --short
   git fetch origin --prune
   ```

   Stop if the worktree is dirty. Do not overwrite unrelated work.

2. **Choose the base branch and stack tip.** List open sqruff port PRs and inspect their head and base branches:

   ```bash
   gh pr list --repo quarylabs/sqruff --state open \
     --json number,title,body,url,headRefName,baseRefName,createdAt
   ```

   Keep PRs whose head branch starts with `port/sqlfluff-` or whose title/body identifies them as SQLFluff ports.

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
   git -C sqlfluff pull
   git -C sqlfluff log --reverse --first-parent --format="%H" <SHA>..main | head -n 1
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

13. **Verify the change** builds and passes tests:

    ```bash
    cargo build
    cargo test
    ```

    Fix any issues before proceeding.

14. **Run all Bazel tests** to ensure nothing is broken. This step is MANDATORY even if no code changes were made (skip-only SHA updates still need a green CI):

    ```bash
    bazel test //...
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

18. **Push and create the PR against the selected base branch:**

    ```bash
    git push -u origin port/sqlfluff-<pr-number>
    gh pr create --base <base-branch> --head port/sqlfluff-<pr-number>
    ```

    When stacking, begin the PR body with:

    ```markdown
    > Stacked on sqruff #<base-pr-number>. Merge #<base-pr-number> first.
    ```

    Include the base PR URL. Ensure the PR diff contains only this port relative to its base branch.

19. **Summarize the stack and merge order.** Report what was ported, the new PR, its base PR, the bottom-to-top merge order, tests run, and any decisions made. After a lower PR merges, rebase or retarget its immediate child onto the updated `main` before merging the child if GitHub does not do so automatically.
