Port exactly one next SQLFluff commit using the checkpointed helper below. This
is an alternative to `next-sqlfluff.md`; do not run both workflows in one turn.

## Boundaries

- At most one PR per invocation, extending the active stack in upstream
  first-parent order. Stop after publishing, finding a duplicate, reaching
  upstream main, or encountering a blocker. A background goal may start another
  invocation on its next continuation.
- Default to at most fifty open SQLFluff port PRs. Preserve explicit session/goal
  overrides: use `--limit 0` only when the user has already removed that cap.
- Preserve unrelated work, stashes, and lockfiles. Do not merge PRs or force-push.
  Only one porting invocation may own this worktree at a time. If another goal is
  using it, stop or use a separately authorized isolated checkout.
- The scripts automate bookkeeping, not the assessment of Rust behavior or
  fixtures. Keep all required Cargo and Bazel checks, including SHA-only ports.

## 1. Preflight once

Choose a unique checkpoint path outside the checkout, and retain its exact path
in the goal's continuation summary. Invoke from the sqruff root:

```bash
python3 .hacking/scripts/sqlfluff_port.py preflight --state /private/tmp/sqlfluff-port-<run-id>.json
```

The helper checks the clean worktree, fetches refs, paginates open PRs, identifies
the active stack tip (including older forks), verifies the first-parent watermark,
finds the next commit, and checks exact duplicates in open and historical PRs.
It fetches SQLFluff without changing its checkout. Its JSON result is the baseline
for this invocation; do not repeat the same queries individually.

- `ready`: continue using the returned `base_sha`, `base_branch`, `branch`,
  `next_sha`, upstream PR, and merge chain.
- `caught_up` or `duplicate`: report the result and stop.
- Nonzero exit / `blocked`: resolve the stated issue without bypassing the check.
  Unknown/divergent watermarks and tied tips require explicit reconciliation.

On interruption, read the existing checkpoint rather than creating another one.
Inspect the current worktree, then use `recheck --state <path>` when resuming a
prepared port. If nothing relevant changed, reuse completed analysis and tests.
For `publishing` or `pushed`, inspect remote branch and PR state before retrying:
timeouts are ambiguous successes. For `published`, finish stack linking/audit and
stop; never create another PR from that checkpoint.

## 2. Inspect and implement

Create the returned branch from the exact base SHA; never overwrite an existing
branch. Read the subject and **first-parent diff**, including merge commits:

```bash
git switch -c <branch> <base_sha>
git -C sqlfluff show -s --format=fuller <next_sha>
git -C sqlfluff diff <next_sha>^1 <next_sha>
```

Port equivalent Rust behavior and tests, following actual repository layout:

- SQLFluff dialects → `crates/lib-dialects/src/`.
- Rules → `crates/lib/src/rules/`.
- Core/parser/templaters → corresponding modules under `crates/lib-core/`.
- Dialect fixtures →
  `crates/lib-dialects/test/fixtures/dialects/<dialect>/sqlfluff/`.

Read applicable repository instructions and upstream context as needed. Docs,
release, CI, or Python-specific changes may justify a SHA-only port. Already
present code is a no-op only if equivalent fixtures exist too. Record the reason.

Use focused tests while developing. Review actual changed code and generated
fixtures; do not regenerate unrelated fixtures. Write `.sqlfluff-sha` to the
returned `next_sha` **before final validation**.

## 3. Stage, validate, and commit

Inspect the intended diff and stage explicit paths, including `.sqlfluff-sha`.
Keep the summary Markdown outside the worktree. Do not use `git add .` to sweep
unrelated files into a port.

```bash
python3 .hacking/scripts/sqlfluff_port.py verify --state <checkpoint-path>
```

This checks the branch, staged watermark, whitespace, and absence of unstaged or
untracked files. It runs formatting checks, `cargo build`, `cargo test`, and
`bazel test //...` once on the final tree and checkpoints results and timings.
Output streams normally; use the returned process/session handle to wait for the
same invocation. Do not restart tests merely because a tool yielded.

After successful validation, commit exactly the staged port. Use the existing
signing policy (including any user requirement for signed commits). Use a
conventional title such as `fix(postgres): support optional SRID (6543)` and
include the upstream SHA and PR/commit links in the commit body.

Changing code, fixtures, the staged tree, or the integration base invalidates the
result. Revalidate after such changes. A commit of the identical staged tree does
not need another full test run. No extra post-SHA test is needed: the full suite
already tested the updated SHA.

## 4. Publish once

Write a short Markdown summary file containing bullets explaining the port and
decisions. Then invoke:

```bash
python3 .hacking/scripts/sqlfluff_port.py publish --state <checkpoint-path> --summary <summary-file>
```

The helper checks that the clean branch contains exactly one commit above the
recorded base and matches the validated tree. It performs the one required fresh
remote stack/capacity/duplicate check immediately before publishing, pushes
without force, and creates the correctly based PR using `--body-file`. It adds
base-PR links, upstream links, and the recorded test list automatically.

Do not run another full remote audit immediately before calling `publish`; the
helper does that. If it reports a changed base or an existing port, reconcile it
and stop or prepare a new valid checkpoint. Never simply change the checkpoint's
base/SHA/test fields to make stale work pass.

Remote publication is not atomic across machines. On a failed/ambiguous request,
inspect remote state before retrying. The helper records its phase and will not
create a duplicate PR it can find. Do not repeatedly retry unresolved failures.

## 5. Link, audit, and stop

Append the returned PR to the existing GitHub stack with
`gh stack link <stack-number> <new-pr-number>`. If no stack exists, link the
returned merge chain bottom-to-top. Verify once with `gh stack view` and a compact
PR inspection confirming its head SHA, base branch, and diff. Check the current
remote tip to detect a concurrent publisher; report any fork and stop further
writes. Record signature verification when required by the user's signing policy.

Do not routinely `unstack --local`, check out the whole stack, or recreate its
metadata after a successful link. Those are recovery operations for demonstrated
metadata problems. If linking fails, the correctly based PR remains valid; record
the outstanding UI repair instead of recreating the PR.

Report the new PR, base, merge order, upstream watermark, tests, and decisions.
Keep a compact continuation checkpoint: state-file path, PR/branch/SHA, stack ID,
completed verification, and any unfinished operation. Do not repeat resolved
historical-fork analysis unless relevant refs change. Stop this invocation.
