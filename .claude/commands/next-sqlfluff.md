Port the next SQLFluff commit to sqruff.

## Workflow

1. **Read the current SHA** from `.sqlfluff-sha` in the project root.

2. **Find the next commit** in the local SQLFluff clone at `sqlfluff/` (relative to project root):
   ```bash
   git -C sqlfluff log --reverse --ancestry-path <SHA>..HEAD --format="%H" -n 1
   ```

3. **Inspect the commit** — show its message and full diff:
   ```bash
   git -C sqlfluff show <next-sha>
   ```

4. **Find the associated PR** — look up the PR that introduced this commit:
   ```bash
   git -C sqlfluff log --format="%s" -n 1 <next-sha>
   ```
   Extract the PR number from the commit message (usually in the format `(#1234)`). The PR URL is `https://github.com/sqlfluff/sqlfluff/pull/<number>`.

5. **Analyze the change** and determine what needs to be ported. Use this path mapping as a guide:
   - `src/sqlfluff/core/` → `crates/lib-core/`
   - `src/sqlfluff/core/rules/` → `crates/lib/src/rules/`
   - `src/sqlfluff/core/dialects/` → `crates/lib-dialects/src/`
   - `src/sqlfluff/core/parser/` → `crates/lib-core/src/parser/`
   - `src/sqlfluff/core/templaters/` → `crates/lib-core/src/templaters/`
   - `test/` → corresponding `#[test]` modules or test files in the relevant crate

6. **Implement the equivalent change** in sqruff's Rust codebase. Some SQLFluff commits (docs-only, CI, Python-specific tooling) may not need a code change — if so, note why and skip to step 8.

7. **Verify the change** builds and passes tests:
   ```bash
   cargo build
   cargo test
   ```
   Fix any issues before proceeding.

8. **Update `.sqlfluff-sha`** with the newly ported commit SHA.

9. **Commit the changes** with a message that includes the SQLFluff PR link. Format:
   ```
   port: <short description of what was ported>

   Ported from SQLFluff <commit-sha>
   https://github.com/sqlfluff/sqlfluff/pull/<pr-number>
   ```

10. **Summarize** what was ported, what was changed in sqruff, and any decisions made (e.g., skipped because docs-only, adapted because of language differences).
