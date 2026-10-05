---
name: session
description: Run a pathway work session from a restored offline environment through backlog selection, implementation, merge, and hand-off. Use at the start of work on this repository, when the user asks to bootstrap the sandbox, choose backlog work, or plan a session, and again after a PR lands when they ask for a report or next-session prompt.
---

# Session lifecycle for pathway

A pathway session has five phases: **(1)** restore and verify the environment, **(2)** survey the
backlog and repository context, **(3)** propose a scoped session and stop, **(4)** implement the
approved work, and **(5)** after merge, report what is proven and write the next opening prompt.
The four hand-off artifacts are templated in [`standup-template.md`](standup-template.md).

Two rules govern the whole loop: do not propose work the restored environment cannot execute,
and do not check an acceptance criterion whose proof does not exist. An unavailable native runner,
release, or maintainer action remains an explicitly named open item.

## 1. Restore the environment

Pathway's development environment is carried by the `sandbox/developer-linux-64` orphan branch.
It is restored into the checkout by `scripts/restore.sh`; the current reviewed plan is
`.pixi-sandbox.toml`. Check first, restore only when necessary:

```bash
test -x "$HOME/.local/bin/pixi" && "$HOME/.local/bin/pixi" --version
# If pixi is absent or does not belong to this checkout:
bash scripts/restore.sh
export PATH="$HOME/.local/bin:$PATH"   # the current shell does not reread the profile
pixi --version
pixi sandbox --version
```

Let the restore register user tools. The default is
`PIXI_SANDBOX_USER_TOOLS=register`; use `PIXI_SANDBOX_USER_TOOLS=skip` only on a shared or
locked-down runner. The restore's final line is evidence:

- `user tools: registered pixi and pixi-sandbox in ...` means the launchers are ready.
- `user tools: NOT registered ...` means the packed binary predates user-tool registration;
  report that fact instead of assuming the PATH was updated.

The current transport is Linux-only because `.pixi-sandbox.toml` publishes `default` for
`linux-64`. Do not promise a macOS or Windows restore until the reviewed plan contains that
bundle and the publisher has produced it. After a push to `main`, verify what the published
transport actually carries rather than trusting a remembered version:

```bash
git fetch origin
git show origin/sandbox/developer-linux-64:.pixi-sandbox/manifest.json
```

Check `source.commit`, the `pixi` and `pixi-sandbox` versions, `platform`, and the lock or
manifest hash. At the time this skill was created, the published manifest named Pathway commit
`4dae9c7`, pixi `0.81.0`, and pixi-sandbox `0.5.2`; those values must move when the transport is
repacked.

### Pixi is the only environment entrypoint

Use the restored environment for every repository command. The normal Pathway task surface
is:

```bash
pixi run --frozen fmt             # rustfmt and Biome rewrite
pixi run --frozen lint            # rustfmt, clippy, cargo-deny, Biome, actionlint
pixi run --frozen typecheck      # TypeScript and Astro checks
pixi run --frozen test            # Rust tests/doctests and Bun tests
pixi run --frozen gates           # fmt-independent commit gate: lint, typecheck, test
pixi run --frozen ci              # gates, coverage, production and release builds
pixi run --frozen backlog task list -s "To Do" --plain
pixi run --frozen skills …
pixi run --frozen hooks-install
```

Never use a bare `cargo`, `rustc`, `bun`, `turbo`, `biome`, `actionlint`, or `convco` when the
command belongs to this repository. Use `pixi run --frozen` so the lockfile and restored tools
are the ones being tested. Workspace-specific commands belong to their package and are reached
through the same Turbo graph, for example:

```bash
pixi run --frozen bun x turbo run test --filter=@repo/rust-core
pixi run --frozen bun x turbo run lint --affected
```

The root `AGENTS.md` is authoritative for the task graph: do not add a duplicate pixi task or
bypass Turbo with a package `cwd`. The sandbox is intended to be air-gapped; if a command needs
network data, identify it as such rather than weakening the offline gate. `lint-advisories` and
`lint-sandbox-plan` are intentionally outside `gates`/`ci` and require network or the static
pixi-sandbox release binary:

```bash
pixi run --frozen lint-advisories
pixi run --frozen lint-sandbox-plan
```

## 2. Survey before proposing

1. **Synchronize first.** Run `git fetch origin`, then compare the working branch with
   `origin/main`. Do not edit on a stale base; report the divergence before proposing work.
2. **Read the standing context.** Read `AGENTS.md`, `.knowledge/index.md`, and
   `.knowledge/CONTEXT.md`; then read the relevant architecture or feature concept linked from
the index. Decisions D1–D7 and the three-crate layout are constraints, not invitations to
   reopen settled design. If a decision appears wrong, bring a measurement.
3. **Inspect the backlog.** Prefer the CLI, always non-interactively:

   ```bash
   pixi run --frozen backlog task list -s "To Do" --plain
   pixi run --frozen backlog search "<query>" --plain
   pixi run --frozen backlog task <id> --plain
   ```

   If the environment is unavailable, read `backlog/tasks/*.md` directly. Task files carry the
   authoritative status, dependency, priority, acceptance criteria, and Definition of Done.
4. **Filter honestly.** A candidate is unblocked only when every dependency is Done. Order
   candidates by priority (High before Medium before Low), then by backlog order. A spike that
   unblocks several tasks may go first, but say why.
5. **Avoid duplicate work.** Check `git log --oneline -15`, recent merged PRs
   (`gh pr list --state merged --limit 5`), and release tags before proposing a task or release
   operation. A completed task or released artifact is not a new task.

## 3. Propose, then stop

Fill in the **Session standup** from [`standup-template.md`](standup-template.md), including the
actual baseline and the environment facts learned during restore. Present the candidate tasks,
dependencies, locally provable slices, and decisions needing user approval. Then stop and wait.
Do not start implementation merely because a task looks unblocked.

The opening prompt, standup, task hand-off, and report are a protocol between sessions. Keep
proposals and measurements in `.knowledge/CONTEXT.md` under a dated `Session scratchpad` entry;
if the section does not exist yet, create it at the end of the file on the first session close.
Do not turn an unapproved idea into production code or a task-file acceptance claim.

## 4. Work under the repository rules

- Work one approved backlog task at a time and make one focused conventional commit per task.
- For source changes, follow the local TDD or refactor skill before editing: read
  [`.agents/skills/tdd/SKILL.md`](../tdd/SKILL.md) for behavior and
  [`.agents/skills/refactor/SKILL.md`](../refactor/SKILL.md) for behavior-preserving structure.
  A change mixing both must complete the TDD slice before the refactor. Documentation-only
  changes do not need a red-green cycle; this skill itself is an agent-document change.
- Run `pixi run --frozen fmt` while committing and `pixi run --frozen gates` before pushing.
  Run `pixi run --frozen ci` when the change touches release, docs, build, or workflow behavior.
  Validate the commit with `pixi run --frozen lint-commit < <file>` or the installed hook.
- Complete the task in the format already present in that file: update frontmatter dates and
  status, check only proven AC/DoD items, and append `Implementation Notes` plus `Final Summary`
  where the task's markers provide them. Do not rewrite its plan or silently widen its scope.
- Isolate tests in fixtures/temp directories. Do not use this checkout or a real home directory
  as a test fixture. Preserve the core invariant: `crates/core` stays NAPI-free.
- Push the working branch and open a pull request. Use a conventional squash title and wait for
  required checks. Read the failing job before changing code; a red platform job is not proof
  that the implementation is wrong.

## 5. Close only after merge

A session is not closed when a branch is pushed. After the PR merges:

1. Watch the workflows caused by the merge:
   `gh run list --branch main --limit 10`. Read the verdict for `ci`, then `docs` and
   `publish sandbox` (and any path-triggered workflow). A red post-merge run belongs to this
   session; fix it before reporting or carry the exact missing proof forward.
2. Finish task files in the PR or a follow-up commit. Every checked AC must have evidence. If a
   criterion needs a native platform, a release, or a maintainer action, leave it open and name
   the missing proof in both the notes and the report.
3. Rebaseline the merged tree:
   `git fetch origin && git log --oneline origin/main -1`, then run
   `pixi run --frozen test`. Record passing/skipped counts and compare them with the opening
   baseline.
4. Append a dated entry to `.knowledge/CONTEXT.md`'s `Session scratchpad` (newest last), recording
   landed work, measurements, unresolved proposals, and the next opening prompt. Preserve the
   file's OKF frontmatter and existing decisions. The context entry is a separate focused
   documentation commit when it cannot be included in the PR.
5. Print the **Session report** from the template and include the fully filled **Session opening
   prompt** at its end. The prompt must be pasteable as the first message of the next session
   and must identify the expected test count, the next decision, and any proof boundary.

A session that merged nothing still reports `Merged: nothing`. Ideas belong in the context
scratchpad, not in speculative source files or checked acceptance criteria.
