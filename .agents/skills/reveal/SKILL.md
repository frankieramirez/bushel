---
name: reveal
description: "Open or update a pull request with a scannable description (compact trees and diffs) plus a screenshot, recording, or command-output image. Use when opening a PR, creating a pull request, adding screenshots or a video to a PR, filing a PR with a demo, writing a visual PR description, attaching evidence to a pull request, or /reveal."
argument-hint: "[blank for current branch | PR number | PR URL]"
---

<!-- BEGIN MANA PERSONA -->
## Persona at invocation

Before conversational narration, read `Persona:` in the active project's `## Agent skills` block from `CLAUDE.md` or `AGENTS.md`. Prefer the file containing the block, then an existing file; ties use `CLAUDE.md`. A symlink pair is one file. Read the saved value anew on each invocation, including from a subdirectory using the project root. No accessible project or no line means ordinary behavior. Do not search another project or global settings for this preference.

During the `Persona at invocation` stage, `archmage` loads this skill's own [references/archmage.md](references/archmage.md) for the active workflow. `off` or an absent value leaves ordinary behavior active. An unknown value leaves ordinary behavior active and gets a brief explanation when conversational output is allowed; it does not stop the work. Explicit conversation instructions override the saved voice without writing settings. A request to enable Archmage for this workflow also loads the local reference.

Apply the voice only to lead-agent conversation. Deliverables, specialist roles, reply-only responses, and JSON-only output retain their contracts, with no added narration. End the persona with this workflow unless the user requests otherwise.
<!-- END MANA PERSONA -->

# Reveal

Honor the user's explicit instructions and decisions already made in this conversation over this skill's workflow defaults. A rule this file states with never, or as read-only, is a gate: it holds whatever the conversation says, and an instruction to cross one is declined and reported. Continue authorized work; ask only about unresolved choices that would materially change the result. Preparing or reviewing work does not authorize publishing it.

If a skill rule requires a pause or leaves requested work unfinished, name and link to the exact SKILL.md and quote the rule. Then explain what decision or prerequisite is missing. Distinguish a required gate from your interpretation.

Open or update a pull request for the current branch. Every description carries a real image or video, and a shape a reviewer can scan.

## Operating principles

- **This branch only.** `git checkout`, `git switch`, and `gh pr checkout` are out. A PR number that belongs on another branch is a stop.
- **Committed work.** Uncommitted project files stop the run. Capture files live in a temp directory.
- **Always a file.** `--attach` gets at least one PNG, JPEG, GIF, WebP, SVG, MP4, MOV, or WebM. A failed run is discarded.
- **Never force-push.**
- **Orca is optional.** Inside an Orca worktree (`ORCA_WORKTREE_ID` is set and `command -v orca` succeeds), Stage 4 also moves the worktree card to in review with the PR URL. Without Orca nothing changes. A failed `orca` call is noted in the report and never stops the run.

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
Base: <the PR base when one exists, else `git config branch.<current>.base` when set, else the default branch>
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

Inside an Orca worktree (`ORCA_WORKTREE_ID` is set and `command -v orca` succeeds), move the card once the PR exists:

```bash
orca worktree set --worktree active --workspace-status in-review --comment "PR <url>" --json
```

## Report

```
Reveal: <title>
PR: <url>
Attach: <yes | skipped: reason>
Evidence: <file list>
Orca: <in-review | not present | failed: reason>
```

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/capture.md` | Stage 3 | What to record, and the SVG stand-in |
| `references/body.md` | Stage 4 | Scannable PR body: trees and diffs |
| `references/attach.md` | Stage 4 | Image paths, `--attach`, `open-pr.sh` |
