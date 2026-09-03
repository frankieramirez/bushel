---
name: cast
description: "Implement one ready ticket or spec on the current branch, then open a pull request with visual evidence. Use when asked to cast a ticket, implement this ticket, build this issue, or /cast. Pass no-pr to stop after the commit."
argument-hint: "[ticket number | issue URL | spec path | blank for the conversation] [no-pr]"
disable-model-invocation: true
---

# Cast

Build the work described by one ticket, spec, or the current conversation. Stay on the current branch. Commit when the work matches the ticket. Push and open a pull request with visual evidence. Pass `no-pr` to stop after the commit (and push only if an upstream already exists).

## Operating principles

- **One ticket.** The invocation names the work. Do not wander onto adjacent issues.
- **Never switch branches.** `git checkout`, `git switch`, and `gh pr checkout` are out. If the ticket belongs on another branch, stop and say so.
- **The ticket is the contract.** A comment labelled as an agent brief, or a spec file, wins over the original issue body when they disagree.
- **Leave the review to a later pass.** This skill commits the implementation. It does not run a multi-reviewer critique.
- **Ship by default.** After the commit, push (creating the upstream if needed) and open a pull request with visual evidence. `no-pr` restores commit-only, with a push only when an upstream already exists.

## Arguments

Parse tokens, then treat the remainder as the target.

| Token | Effect |
|-------|--------|
| `no-pr` | Stop after Stage 4. Push only when an upstream already exists. |

| Input | Target |
|-------|--------|
| none | The ticket or spec already in this conversation. If none is obvious, stop and ask for a number. |
| number or issue URL | That GitHub issue |
| a path | That file, treated as the spec |

## Execution spine

1. Load the ticket (Stage 1).
2. Build it (Stage 2).
3. Check the diff against the ticket (Stage 3).
4. Commit, and push only when the branch already has an upstream (Stage 4).
5. Capture proof and open the pull request (Stage 5). Skip when `no-pr`.

`SKILL_DIR` is the absolute directory this SKILL.md lives in. The Bash tool forgets variables between calls, so every block that runs a bundled script sets `SKILL_DIR` again on its first line.

---

## Stage 1: Load

**Number or URL.** Fetch the issue:

```bash
gh issue view NUMBER --comments
```

Prefer, in this order: the latest comment headed `## Agent Brief`; a linked spec path named in the body; the issue body itself.

**A path.** Read that file. It is the spec and the contract.

**Blank.** Use the ticket or spec already in this conversation. If none is obvious, stop and ask.

Read `CONTEXT.md` when it exists, and any ADR that sits in the same area as the change. Use the project's words for types and names.

Write a 2 to 4 line intent you will implement:

```
Intent: <what will be true when this ticket is done>
Seams: <public interfaces you will test at, or "none: no test harness">
```

Do not start coding until that intent is written. If the ticket is still a question (a decision, not a build), stop. This skill builds ready work.

## Stage 2: Build

If the repo has a test harness, read `references/tdd.md` and follow it at the seams you wrote down. If it does not, build without a red-green loop and say so once.

Typecheck and the tests around the files you touch as you go. Run the project's full suite once the slice is in.

Stay inside the ticket's scope. Adjacent cleanup waits.

## Stage 3: Spec check

Read `references/spec-check.md` and walk it against the diff and the ticket. If a criterion fails, fix it before committing. If a criterion cannot be met on this branch, stop and report it. Do not commit a partial that pretends to be the ticket.

## Stage 4: Commit

At the start of the session, record `git status --porcelain` and the unstaged and staged diffs (`git diff`, `git diff --cached`). Stop when a file this session will edit already has unstaged or staged hunks.

Stage only this session's changes. `git add <file>` stages every hunk in that file, dirty ones included, and `git commit` includes whatever was already in the index. Before you commit, confirm `git diff --cached` holds only this session.

```bash
git add <files you changed>
git commit -m "$(cat <<'EOF'
<subject from the ticket title>

<one or two lines on what landed, with the issue number>
EOF
)"
```

Follow the repo's commit conventions when it has them.

Push only when an upstream is already configured:

```bash
if git rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1; then
  git push
fi
```

If the push is rejected because the remote moved, `git pull --rebase` only when the tree is otherwise clean and the rebase is conflict-free. Otherwise stop. Never force-push.

## Stage 5: Ship

Skip this stage when `no-pr` was passed. Stage 4 has already committed, and pushed only when an upstream existed. Stage 5 never runs if Stage 4 did not commit.

Read `references/capture.md`, `references/body.md`, and `references/attach.md`. Push so the branch exists on the remote:

```bash
if git rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1; then
  git push
else
  git push -u origin HEAD
fi
```

Same rebase-or-stop rule as Stage 4. Never force-push.

Capture at least one proof file. Then run `scripts/open-pr.sh` with the title, body file, and attaches.

## Report

```
Cast: <ticket title> (#NUMBER)
Commit: <sha>
Pushed: <yes, to branch | no, no upstream | no, push failed: reason>
PR: <url | none: no-pr | none: reason>
Evidence: <file list, or none>
Validation: <one line>
Open: <any criterion left unmet, or none>
```

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/tdd.md` | Stage 2, when a test harness exists | Red-green at agreed seams |
| `references/spec-check.md` | Stage 3 | Diff vs ticket before commit |
| `references/capture.md` | Stage 5 | What to record, and the SVG stand-in |
| `references/body.md` | Stage 5 | Scannable PR body: trees and diffs |
| `references/attach.md` | Stage 5 | Image paths, `--attach`, `open-pr.sh` |
