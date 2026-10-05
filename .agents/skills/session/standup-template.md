# Pathway session lifecycle templates

A session produces four artifacts: the **opening prompt** for the next session, the **standup**
that proposes this one, a **task hand-off** before each task commit, and the **report** after the
PR merges. Keep the headings and order stable so a later agent can resume without reconstructing
the session from chat.

The report ends with the next opening prompt. A session that merges nothing still writes one.
Ideas and measurements that were not implemented go in the dated `Session scratchpad` section of
`.knowledge/CONTEXT.md`, never in speculative production files or checked acceptance criteria.

## 1. Session opening prompt

Write this at close and paste it as the first message of the next session:

```
Restore Pathway's sandbox and baseline the suite (expect <N> passing / <M> skipped — <the
transport commit, tool versions, or vendoring fact that matters>), then read
`.knowledge/CONTEXT.md` § Session scratchpad — the <date> heading lists <K> open items.

<Optional remote check and interpretation. Example: "First run `gh release list`. If <release>
exists, read its CI and publish runs and close <task> only after AC#<n> has evidence. If it does
not exist, leave AC#<n> open and work the local slice instead.">

I want to take task-<n> this session — <what it delivers and the decisions already settled>.
Work in slices: <locally provable work> first, then <push/release/native-runner proof> last; the
latter needs <the sanction or external action being reserved>.

Propose the slice and stop. Repository rules are in `AGENTS.md`; the session procedure and
handoff templates are in `.agents/skills/session/`.
```

A good prompt names the expected test count, points to the dated scratchpad heading, carries
settled decisions forward, and states what may or may not be pushed. Do not write "decide later"
when the decision was already made, and do not claim a remote proof that has not happened.

## 2. Session standup

Write this after restore and backlog survey, then stop for user confirmation:

```
Session proposal — <date>

Environment: restored; platform <linux-64>; baseline <N> tests passing, <M> skipped.
User tools: <registered in ~/.local/bin | NOT registered — explain the restore output>.
Transport: <origin/sandbox/developer-linux-64 manifest source.commit, pixi, pixi-sandbox,
lock/vendor fact>.

Backlog: <X> To Do, <Y> unblocked. Candidates, in recommended order:
1. task-<n> (<priority>, <type>) — <what it delivers and why now>
2. task-<m> …
   …
Not this session: task-<k> (blocked by task-<j>); task-<l> (deferred — <reason>).

Slices: <locally provable work> first; <work needing a push, release, or native runner> last,
and only with the required sanction.
Decisions I need before starting: <shape-changing questions with a recommendation each — or
"none">.

Per task, "done" means: acceptance criteria and Definition of Done checked only where proven,
one focused conventional commit, `pixi run --frozen gates` green, and the task file completed in
its existing format.
Need from you: confirm the scope (or pick a different candidate) before I start.
```

## 3. Task hand-off

Write this before committing each task:

```
task-<n> — <title>

Changed: <file> (<why>), …
Evidence: <command> → <result>. Suite <N> passing / <M> skipped (was <N0>/<M0>).
Gates: fmt <✓/—>  lint <✓/—>  typecheck <✓/—>  test <✓/—>  convco <✓/—>.
Task record: <AC/DoD checked and notes/final summary updated, or the exact item left open>.
Left undone: <anything outside the approved slice, or "nothing">.
```

Do not turn a hand-off into a claim about a GitHub job that is still running. Say `pending` and
record the run ID instead.

## 4. Session report

Write this after merge and after watching post-merge workflows:

```
Session report — <date>

Merged: PR #<N> "<squash title>" → main at <sha>.
Post-merge runs: ci <verdict, run>, docs <verdict, run>, publish sandbox <verdict, run>,
<any other triggered workflow> <verdict>.
Landed: <commit subject> (<task-id>), …

Tasks: task-<n> Done — every checked AC has evidence. task-<m> still In Progress — AC#<k> needs
<the proof unavailable here: native runner, release, maintainer action>.
Suite on merged main: <N> passing / <M> skipped (was <N0>/<M0>). Gates: <summary>.

Recorded in `.knowledge/CONTEXT.md`: <dated scratchpad heading and what it preserves>.
Open, in the order a session should consider them: <task — why next or what blocks it>, …
Environment facts for next time: <transport commit/tool versions, platform limitations,
vendoring or lockfile facts>.

Next session should start with:

> <template 1, fully filled in>
```

If no PR merged, write `Merged: nothing`, record the useful measurements and proposals in the
scratchpad, and make the next opening prompt say what should be decided first.
