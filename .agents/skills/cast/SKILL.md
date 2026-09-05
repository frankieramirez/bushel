---
name: cast
description: "Implement one ready ticket or spec on the current branch, then open a pull request with visual evidence. Use when asked to cast a ticket, implement this ticket, build this issue, take the next ready ticket, or /cast. Pass next to claim the oldest unclaimed ready-for-agent issue, and no-pr to stop after the commit."
argument-hint: "[ticket number | issue URL | spec path | next | blank for the conversation] [no-pr]"
disable-model-invocation: true
---

<!-- BEGIN MANA PERSONA -->
## Persona at invocation

Before conversational narration, read `Persona:` in the active project's `## Agent skills` block from `CLAUDE.md` or `AGENTS.md`. Prefer the file containing the block, then an existing file; ties use `CLAUDE.md`. A symlink pair is one file. Read the saved value anew on each invocation, including from a subdirectory using the project root. No accessible project or no line means ordinary behavior. Do not search another project or global settings for this preference.

During the `Persona at invocation` stage, `archmage` loads this skill's own [references/archmage.md](references/archmage.md) for the active workflow. `off` or an absent value leaves ordinary behavior active. An unknown value leaves ordinary behavior active and gets a brief explanation when conversational output is allowed; it does not stop the work. Explicit conversation instructions override the saved voice without writing settings. A request to enable Archmage for this workflow also loads the local reference.

Apply the voice only to lead-agent conversation. Deliverables, specialist roles, reply-only responses, and JSON-only output retain their contracts, with no added narration. End the persona with this workflow unless the user requests otherwise.
<!-- END MANA PERSONA -->

# Cast

Honor the user's explicit instructions and decisions already made in this conversation over this skill's workflow defaults. A rule this file states with never, or as read-only, is a gate: it holds whatever the conversation says, and an instruction to cross one is declined and reported. Continue authorized work; ask only about unresolved choices that would materially change the result. Preparing or reviewing work does not authorize publishing it.

If a skill rule requires a pause or leaves requested work unfinished, name and link to the exact SKILL.md and quote the rule. Then explain what decision or prerequisite is missing. Distinguish a required gate from your interpretation.

Build the work described by one ticket, spec, or the current conversation. Stay on the current branch. Commit when the work matches the ticket. Push and open a pull request with visual evidence. Pass `no-pr` to stop after the commit (and push only if an upstream already exists).

## Operating principles

- **One ticket.** The invocation names the work. Do not wander onto adjacent issues.
- **Never switch to an existing branch.** `git checkout <branch>`, `git switch <branch>`, and `gh pr checkout` are out. If the ticket belongs on another branch, stop and say so. The one branch this skill creates is a fresh one off the default branch, when the session starts there, before any edit (Stage 1).
- **Claim before work.** A ticket from the tracker gets assigned to the person driving this session first, so a parallel session skips it. Held by someone else: stop.
- **The ticket is the contract.** A comment labelled as an agent brief, or a spec file, wins over the original issue body when they disagree.
- **Leave the review to a later pass.** This skill commits the implementation. It does not run a multi-reviewer critique.
- **Ship by default.** After the commit, push (creating the upstream if needed) and open a pull request with visual evidence. `no-pr` restores commit-only, with a push only when an upstream already exists.
- **Orca is optional.** Inside an Orca worktree (`ORCA_WORKTREE_ID` is set and `command -v orca` succeeds), the skill also keeps the worktree card current: the linked ticket, the status column, and a one-line comment. Without Orca nothing changes. An `orca` call that fails is noted in the report and never stops the run. When the skill stops early, leave the reason as the comment: `orca worktree set --worktree active --comment "<reason>" --json`.

## Arguments

Parse tokens, then treat the remainder as the target.

| Token | Effect |
|-------|--------|
| `no-pr` | Stop after Stage 4. Push only when an upstream already exists. |

| Input | Target |
|-------|--------|
| none | The ticket or spec already in this conversation. If none is obvious, stop and ask for a number. |
| number or issue URL | That GitHub issue |
| `next` | The oldest open `ready-for-agent` issue that nobody holds and nothing blocks |
| a path | That file, treated as the spec |

## Execution spine

1. Load the ticket (Stage 1).
2. Build it (Stage 2).
3. Check the diff against the ticket (Stage 3).
4. Commit, and push only when the branch already has an upstream (Stage 4).
5. Capture proof and open the pull request (Stage 5). Skip when `no-pr`.

`SKILL_DIR` is the absolute directory this SKILL.md lives in. The Bash tool forgets variables between calls, so every block that runs a bundled script sets `SKILL_DIR` again on its first line.

---

## Tracker

Read `docs/agents/issue-tracker.md` when it exists. Its `Tracker:` line names the tracker and its `Adapter flags:` line gives the flags for the bundled script. Missing file: GitHub, no flags. On a GitHub Enterprise host, pass `GH_HOST=<host>` inline too. Ticket ids are whatever the tracker uses (`42`, `ENG-42`, `PLAT-42`).

The operations below are `next`, `claim`, and `view`. On Linear or Jira, when the host exposes a connector for that tracker, use it for them; it is already authenticated. Inside an Orca worktree, `orca linear` is such a connector for Linear: `orca linear issue <id> --comments --relations --json` is `view`, `orca linear assignee set` is `claim`, and `orca linear --help` lists the rest. `next` through a connector means: the oldest open issue carrying the ready label, with no assignee and no open blocking relation. `claim` means: read the assignee, stop if it is someone else, assign yourself, read it again. After a connector `next` plus `claim`, view the ticket. If it is closed, missing the ready label, or still blocked, unassign yourself and stop before creating `cast/<id>-*`. Otherwise run the script with the adapter flags. GitHub always goes through the script. Never mix the two in one run. For `local`, the ticket is a file: `next` is the lowest-numbered file with `Status: ready-for-agent` and no open `Blocked by:`, and claim is rewriting that line to `Status: claimed`. Re-read the file after claiming; if a `Blocked by:` file is still open, set `Status: ready-for-agent` and stop. For `other`, follow the tracker file's Conventions by hand.

## Stage 1: Load

**`next`.** Resolve the ready label: the string `docs/agents/triage-labels.md` maps for `ready-for-agent` when that file exists, else `ready-for-agent`. Then:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> next <ready string> --claim
```

Empty output means nothing is ready. Say so in one line and stop; a loop that calls this on a schedule should stay quiet. `--claim` assigns the ticket only while it is still open, still carries the ready label, and is still unblocked; otherwise the script releases it and tries the next candidate. The first field is the ticket id. Do not call `claim` again on this path.

**Id or URL.** Claim it before reading further. An explicit id does not have to carry the ready label:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> claim ID
```

`ID` is a tracker id (`42`, `ENG-42`, `PLAT-42`) or a GitHub, Linear, or Jira issue URL; the script extracts the id. Exit 1 with "already claimed by" names the other holder: stop and say who has it. A ticket already assigned to you is fine. Exit 3 means the token cannot write; note it in the report and continue unclaimed.

Fetch the ticket with its comments:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> view ID
```

Prefer, in this order: the latest comment headed `## Agent Brief`; a linked spec path named in the body; the ticket body itself.

**Orca card.** Inside an Orca worktree (`ORCA_WORKTREE_ID` is set and `command -v orca` succeeds), link the ticket to the worktree card once it is claimed. GitHub: `orca worktree set --worktree active --issue <n> --json`. Linear: `--linear-issue <ENG-42>` instead. Jira, local, or other: `--comment "<id> <title>"` instead, since the card has no field for those. Skip this for a spec path or the conversation.

**Branch.** Compare the current branch with the repo default:

```bash
git rev-parse --abbrev-ref HEAD
gh repo view --json defaultBranchRef --jq .defaultBranchRef.name
```

When they match, create the working branch now, before any edit, and never commit to the default branch:

```bash
git switch -c cast/<id>-<short-kebab-slug-from-the-title>
```

The id is lowercased as it appears on the tracker (`cast/42-flat-tax`, `cast/eng-42-flat-tax`). For a spec path or the conversation with no ticket, name it `cast/<slug>`. Any other current branch is the working branch as it stands.

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

When the change has meaningful behavior to verify and the repo has a test harness, read `references/tdd.md` and follow it at the seams you wrote down. Docs, configuration, and other changes with no behavioral effect do not require a TDD loop just because a harness exists. If there is no harness, build without a red-green loop and say so once.

Typecheck and run meaningful behavior tests around the files you touch as you go. Run the project's required validation: use the `Validation:` line in the `## Agent skills` block of `CLAUDE.md` or `AGENTS.md` when one exists, else what the repo's manifest and docs name. A successful validation may be reused when no edits have happened since it ran. Classify a failure against the pre-change baseline first. Rerun it after a new edit or an unresolved concern that needs a fresh run.

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

Capture at least one proof file. The body ends with a closing line for the ticket (`Closes #42`, `Closes ENG-42`; see the closing line in `references/body.md`). Then run `scripts/open-pr.sh` with the title, body file, and attaches.

**Orca card.** Inside an Orca worktree (`ORCA_WORKTREE_ID` is set and `command -v orca` succeeds), move the card once the PR exists:

```bash
orca worktree set --worktree active --workspace-status in-review --comment "PR <url>" --json
```

On Linear, also attach the PR to the issue so it shows there before the merge: `orca linear attach <ENG-42> --url <pr url> --title "Pull request" --json`.

## Report

```
Cast: <ticket title> (#NUMBER)
Claimed: <yes | already mine | no: reason | none: spec path or conversation>
Branch: <created cast/... | existing branch name>
Commit: <sha>
Pushed: <yes, to branch | no, no upstream | no, push failed: reason>
PR: <url | none: no-pr | none: reason>
Evidence: <file list, or none>
Validation: <one line>
Orca: <linked <id>, in-review | not present | failed: reason>
Open: <any criterion left unmet, or none>
```

## Scripts

`scripts/tickets.sh` finds the next unclaimed ready ticket, claims it, and reads it, on GitHub (`git` and `gh` only), Linear, or Jira (`python3` and the tracker's environment variables). Exit 3 means the token cannot write. `tickets.sh -h` prints usage. `scripts/open-pr.sh` opens or edits the pull request with `--attach`.

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/tdd.md` | Stage 2, for meaningful behavior changes with a test harness | Red-green at agreed seams |
| `references/spec-check.md` | Stage 3 | Diff vs ticket before commit |
| `references/capture.md` | Stage 5 | What to record, and the SVG stand-in |
| `references/body.md` | Stage 5 | Scannable PR body: trees and diffs |
| `references/attach.md` | Stage 5 | Image paths, `--attach`, `open-pr.sh` |
