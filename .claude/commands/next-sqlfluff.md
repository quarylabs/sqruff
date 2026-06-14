Port the next SQLFluff commit to sqruff.

## Workflow

1. **Start from main** — make sure you're on a clean, up-to-date main:
   ```bash
   git checkout main
   git pull
   ```

2. **Read the current SHA** from `.sqlfluff-sha` in the project root.

3. **Ensure the SQLFluff clone exists** at `sqlfluff/` (relative to project root). If it doesn't exist, clone it:
   ```bash
   git clone https://github.com/sqlfluff/sqlfluff.git sqlfluff
   ```

4. **Find the next commit** in the local SQLFluff clone at `sqlfluff/`. First make sure it's up to date, then get the next commit after the saved SHA on the main branch (first-parent only, so merge commits are followed but side-branch commits are skipped):
   ```bash
   git -C sqlfluff pull
   git -C sqlfluff log --reverse --first-parent --format="%H" <SHA>..main | head -n 1
   ```

5. **Inspect the commit** — show its message and full diff:
   ```bash
   git -C sqlfluff show <next-sha>
   ```

6. **Find the associated PR** — look up the PR that introduced this commit:
   ```bash
   git -C sqlfluff log --format="%s" -n 1 <next-sha>
   ```
   Extract the PR number from the commit message (usually in the format `(#1234)`). The PR URL is `https://github.com/sqlfluff/sqlfluff/pull/<number>`.

7. **Check whether this port already exists.** Before creating a branch, look for an existing port of this SQLFluff PR in the sqruff repo (`quarylabs/sqruff`). Search both open and closed/merged PRs for one matching this port — for example by the branch name `port/sqlfluff-<pr-number>`, or by the `(<pr-number>)` reference / SQLFluff links in the PR title or body:
   ```bash
   gh pr list --repo quarylabs/sqruff --state all --search "sqlfluff-<pr-number>"
   ```
   If a matching PR already exists, **do nothing** — do not create a branch, make changes, or open a new PR. Instead, report that the port already exists and reference the existing PR (number and URL), then stop. Otherwise, continue to step 8.

8. **Create a branch** for this port, named after the PR:
   ```bash
   git checkout -b port/sqlfluff-<pr-number>
   ```

9. **Analyze the change** and determine what needs to be ported. Use this path mapping as a guide:
   - `src/sqlfluff/core/` → `crates/lib-core/`
   - `src/sqlfluff/core/rules/` → `crates/lib/src/rules/`
   - `src/sqlfluff/core/dialects/` → `crates/lib-dialects/src/`
   - `src/sqlfluff/core/parser/` → `crates/lib-core/src/parser/`
   - `src/sqlfluff/core/templaters/` → `crates/lib-core/src/templaters/`
   - `test/` → corresponding `#[test]` modules or test files in the relevant crate

10. **Implement the equivalent change** in sqruff's Rust codebase. Some SQLFluff commits (docs-only, CI, Python-specific tooling) may not need a code change — if so, note why and skip to step 13. Do NOT skip the Bazel test step.

11. **Check that test fixtures are ported.** If the SQLFluff commit adds or modifies test fixture files (SQL files, YAML parse trees, etc.), verify that equivalent fixtures exist in sqruff. If they don't, add them. Fixture files in SQLFluff under `test/fixtures/dialects/<dialect>/` map to `crates/lib-dialects/test/fixtures/dialects/<dialect>/sqlfluff/` in sqruff. If the change is already ported (code + fixtures both present), note that and skip to step 13. Do NOT skip the Bazel test step.

12. **Verify the change** builds and passes tests:
    ```bash
    cargo build
    cargo test
    ```
    Fix any issues before proceeding.

13. **Run all Bazel tests** to ensure nothing is broken. This step is MANDATORY even if no code changes were made (skip-only SHA updates still need a green CI):
    ```bash
    bazel test //...
    ```
    Fix any failures before proceeding.

14. **Update `.sqlfluff-sha`** with the newly ported commit SHA.

15. **Commit and create a PR**. Use a conventional commit prefix (`feat:` for new features/rules/dialects, `fix:` for bug fixes, `chore:` for refactors/docs/tests/cleanup). Use the same message for both the commit and the PR title/body. Format:

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

    ```bash
    git push -u origin port/sqlfluff-<pr-number>
    gh pr create
    ```

16. **Summarize** what was ported, what was changed in sqruff, and any decisions made (e.g., skipped because docs-only, adapted because of language differences).
