# Settings

Every setting this skill changes, where it lives, what happens when it is unset, and which skills read it. Load this at Stage 1 and fill the Stage 2 table from it.

Nothing here is required. Every reader falls back, so removing a setting is always safe and is sometimes the point.

| Setting | Lives in | Default when unset | Read by |
|---------|----------|--------------------|---------|
| labels | The table rows in `docs/agents/triage-labels.md`, and the `Triage labels:` line | The label string equals the role name | Triage, ticket filing, ticket building, map walking |
| validation | The `Validation:` line in the `## Agent skills` block | Each skill detects a test command from the repo's manifest and docs | Review, feedback resolution, ticket building, merge repair |
| proof | The `Proof:` line in the block | The capturing skill picks a route from what the host offers | Pull request bodies |
| docs | The `Domain docs:` line in the block, plus whether `CONTEXT.md` or `CONTEXT-MAP.md` exists | Single context, and the file is created by whichever skill first has something to write in it | Anything that reads project vocabulary |
| peer | The `Peer reviewer:` line in the block | No second opinion reviewer, and the diff never leaves the machine | Review |
| worktree | `.worktreeinclude` and `orca.yaml` at the repo root | Nothing is copied or shared into a fresh worktree | Orca, when it creates a worktree |
| pr-surface | The `## Pull requests as a request surface` section of `docs/agents/issue-tracker.md` | `No.`, so triage sees issues only | Triage |
| key | The `Project:` and `Adapter flags:` lines of `docs/agents/issue-tracker.md` | The bundled script falls back to `LINEAR_TEAM` or `JIRA_PROJECT` | Every tracker call in every skill |
| pointer | Which of `CLAUDE.md` and `AGENTS.md` holds the block | Whichever one exists | Every setting above |
| persona | The `Persona:` line in the `## Agent skills` block | Off, with ordinary skill behavior | Every skill's lead agent |

## The seven roles

Two categories, `bug` and `enhancement`. Five states, `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. A triaged ticket carries exactly one category and one state. The label strings are what the tracker actually has; the roles never change.

## What a stale value looks like

Worth putting first in the Stage 2 question, because these are the ones a user came here to fix.

- `key` names a project the `check` subcommand cannot read.
- `validation` names a command that no longer runs, or a script the manifest no longer has.
- `labels` maps a role to a string that is not on the tracker any more.
- `peer` names a CLI that is not on the PATH.
- `pointer` names a file that does not exist.
- `persona` contains a value other than `archmage` or `off`.
