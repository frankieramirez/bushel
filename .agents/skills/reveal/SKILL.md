---
name: reveal
description: "Open or update a pull request with a scannable description (compact trees and diffs) plus a screenshot, recording, or command-output image. Use when opening a PR, creating a pull request, adding screenshots or a video to a PR, filing a PR with a demo, writing a visual PR description, attaching evidence to a pull request, or /reveal."
argument-hint: "[blank for current branch | PR number | PR URL]"
---

# Reveal

Open or update a pull request for the current branch. Every description carries a real image or video, and a shape a reviewer can scan.

## Operating principles

- **This branch only.** `git checkout`, `git switch`, and `gh pr checkout` are out. A PR number that belongs on another branch is a stop.
- **Committed work.** Uncommitted project files stop the run. Capture files live in a temp directory.
- **Always a file.** `--attach` gets at least one PNG, JPEG, GIF, WebP, SVG, MP4, MOV, or WebM. A failed run is discarded.
- **Never force-push.**

## Arguments

The remainder after any tokens is the target.

| Input | Target |
|-------|--------|
| none | The current branch |
| number or PR URL | That pull request, if its head is this branch |

## Execution spine

1. Resolve the branch and any existing pull request (Stage 1).
2. Push, creating the upstream if needed (Stage 2).
3. Capture proof (Stage 3).
4. Write the body and ship (Stage 4).

`SKILL_DIR` is the absolute directory this SKILL.md lives in. The Bash tool forgets variables between calls, so every block that runs a bundled script sets `SKILL_DIR` again on its first line.

---

## Stage 1: Scope

Confirm you are in a git checkout and `gh repo view` works. Record:

```
Branch: <current branch>
Base: <default branch, or the PR base when one exists>
```

**Number or URL.** Fetch it:

```bash
gh pr view NUMBER --json number,url,title,headRefName,baseRefName,isCrossRepository,state
```

If `headRefName` is not the current branch, or `isCrossRepository` is true, stop. Say which branch that PR is on.

**Blank.** Look up a PR for this branch:

```bash
gh pr view --json number,url,title,headRefName,baseRefName,state
```

No PR is fine. Stage 4 will create one.

Stop when `git status --porcelain` lists project files. Temp capture files sit outside the repo.

## Stage 2: Push

```bash
if git rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1; then
  git push
else
  git push -u origin HEAD
fi
```

If the push is rejected because the remote moved, `git pull --rebase` only when the tree is otherwise clean and the rebase is conflict-free. Otherwise stop. Never force-push.

## Stage 3: Capture

Read `references/capture.md` and follow it. You need at least one file before Stage 4.

## Stage 4: Ship

Read `references/body.md` and `references/attach.md`. Write the body to a temp file. Run `scripts/open-pr.sh`.

## Report

```
Reveal: <title>
PR: <url>
Attach: <yes | skipped: reason>
Evidence: <file list>
```

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/capture.md` | Stage 3 | What to record, and the SVG stand-in |
| `references/body.md` | Stage 4 | Scannable PR body: trees and diffs |
| `references/attach.md` | Stage 4 | Image paths, `--attach`, `open-pr.sh` |
