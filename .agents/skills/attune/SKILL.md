---
name: attune
description: "Change one setting the other skills read for this repo, without redoing setup: the triage label names, the command that proves the project works, how pull request proof is captured, the domain docs layout, whether a second CLI reviews every diff, the worktree files, whether pull requests enter triage as requests, the tracker key, or the Archmage persona. Run it with nothing to see every current setting and what reads it. Use when asked to change the validation command, set the test command, rename the triage labels, enable or disable Archmage, add or remove a peer reviewer, turn off the second reviewer, fix the Linear team key, or /attune."
argument-hint: "[blank to list every setting] [labels | validation | proof | docs | peer | worktree | pr-surface | key | pointer | persona] [new value]"
---

<!-- BEGIN MANA PERSONA -->
## Persona at invocation

Before conversational narration, read `Persona:` in the active project's `## Agent skills` block from `CLAUDE.md` or `AGENTS.md`. Prefer the file containing the block, then an existing file; ties use `CLAUDE.md`. A symlink pair is one file. Read the saved value anew on each invocation, including from a subdirectory using the project root. No accessible project or no line means ordinary behavior. Do not search another project or global settings for this preference.

During the `Persona at invocation` stage, `archmage` loads this skill's own [references/archmage.md](references/archmage.md) for the active workflow. `off` or an absent value leaves ordinary behavior active. An unknown value leaves ordinary behavior active and gets a brief explanation when conversational output is allowed; it does not stop the work. Explicit conversation instructions override the saved voice without writing settings. A request to enable Archmage for this workflow also loads the local reference.

Apply the voice only to lead-agent conversation. Deliverables, specialist roles, reply-only responses, and JSON-only output retain their contracts, with no added narration. End the persona with this workflow unless the user requests otherwise.
<!-- END MANA PERSONA -->

# Attune

Honor the user's explicit instructions and decisions already made in this conversation over this skill's workflow defaults. A rule this file states with never, or as read-only, is a gate: it holds whatever the conversation says, and an instruction to cross one is declined and reported. Continue authorized work; ask only about unresolved choices that would materially change the result. Preparing or reviewing work does not authorize publishing it.

If a skill rule requires a pause or leaves requested work unfinished, name and link to the exact SKILL.md and quote the rule. Then explain what decision or prerequisite is missing. Distinguish a required gate from your interpretation.

Change one thing the other skills read. This skill edits existing configuration, or creates the instruction file for a first persona setting. It does not pick the tracker, and it does not write `docs/agents/issue-tracker.md` from scratch. The `persona` setting can be configured before a tracker exists.

## Operating principles

- **One setting per run.** Named on the invocation, or picked from the table. Never walk the user through all of them.
- **Edit the line, keep the file.** Change only the line or the rows this setting owns. Every other line keeps its text and its order, byte for byte. Never re-render a file from a template.
- **Show a file, write a line.** A one-line change is written, and the before and after goes in the report. A whole file (`docs/agents/triage-labels.md`, `orca.yaml`, `.worktreeinclude`) is shown first.
- **Secrets stay in the environment.** The files name the variable, never the value.
- **Nothing here is required.** Every setting has a working default, and every skill that reads one falls back when it is missing. Removing a setting is always allowed, and for the second opinion reviewer it is the safe direction.

`SKILL_DIR` is the absolute directory this SKILL.md lives in. The Bash tool forgets variables between calls, so every block that runs the bundled script sets `SKILL_DIR` again on its first line.

## Execution spine

1. Read the current state (Stage 1).
2. Pick one setting (Stage 2).
3. Change it (Stage 3).
4. Write one thing and report (Stage 4).

---

## Stage 1: Read the current state

Resolve the project root before reading settings, even when invoked in a subdirectory:

```bash
PROJECT_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)
printf '%s\n' "$PROJECT_ROOT"
```

Outside Git, use the host's known workspace root when available in place of the current-directory fallback. Retain the resolved absolute path for this invocation. Shell state does not persist between calls: set `PROJECT_ROOT` to that recorded path and use it as the working directory for every later read, edit, and command, including Stage 3. Do not resolve it again from a later shell's directory.

Prefer the instruction file that contains the `## Agent skills` block, then an existing file; ties use `CLAUDE.md`. A symlink pair is one file. For persona configuration with neither file present, use `AGENTS.md`.

A named persona invocation skips tracker inspection and CLI discovery; read the instruction files and the settings reference. Other invocations inspect:

```bash
PROJECT_ROOT="<absolute project root recorded above>"; cd "$PROJECT_ROOT" || exit 1
ls -l CLAUDE.md AGENTS.md 2>/dev/null
awk 'FNR==1 {show=0} /^## Agent skills$/ {show=1; print; next} /^## / {show=0} show' CLAUDE.md AGENTS.md 2>/dev/null
sed -n '1,12p' docs/agents/issue-tracker.md 2>/dev/null
cat docs/agents/triage-labels.md 2>/dev/null
ls -l .worktreeinclude orca.yaml CONTEXT.md CONTEXT-MAP.md 2>/dev/null
[ -n "${ORCA_WORKTREE_ID:-}" ] && command -v orca >/dev/null && echo orca: yes
for c in codex gemini cursor-agent opencode grok; do command -v "$c" >/dev/null && echo "peer available: $c"; done
```

`docs/agents/issue-tracker.md` is missing: persona still works. A bare run can list the settings and offer persona; explain that other settings require setup if one is selected. For a named setting other than persona, report the missing tracker and stop.

Read `references/settings.md` at this stage. It carries every setting, the exact file and line it lives on, its default when unset, which skills read it, and what the edit is.

## Stage 2: Pick one setting

A setting named on the invocation skips this stage. A setting and a value both named on the invocation skip Stage 2 and every question in Stage 3: write it and report.

Otherwise print the table first, filled in from Stage 1. The table is the whole answer to a bare run:

```
Setting      Current                                   Read by
labels       role name equals label string (no file)   triage, ticket filing, building
validation   pnpm test && pnpm typecheck               review, feedback, building, merges
proof        unset, the host picks                     pull request bodies
docs         unset, CONTEXT.md at the root             anything reading project vocabulary
peer         unset, the diff stays on this machine     review
worktree     .worktreeinclude present, no orca.yaml    fresh worktrees
pr-surface   No, issues only                           triage
key          ENG, verified as frankie                  every tracker call
pointer      AGENTS.md                                 everything above
persona      off                                      every skill's lead agent
```

Then ask one question with the platform's blocking question tool (`AskUserQuestion` in Claude Code; call `ToolSearch` with `select:AskUserQuestion` first if the schema is not loaded), falling back to the conversation where no such tool exists. Four options is the tool's maximum, so order the settings this way and offer the first four: every setting whose check failed or whose value is stale, then every setting with no value, then the rest in table order. The free text answer takes any other name from the table. Nothing to change is a fine answer, because the table was the point.

## Stage 3: The flows

One setting, one flow. Stop when it is written. Resolve project configuration paths below against the absolute project root recorded in Stage 1, including paths passed to file-editing tools. Supporting references and bundled scripts still resolve from `SKILL_DIR`.

**labels.** Show the current mapping, or say the roles use their own names. List the labels that already exist on the tracker, through the connector or by reading with the bundled script. Ask for the mapping in one question, with the roles that already have a good match filled in. Write `docs/agents/triage-labels.md` from `references/triage-labels.md`, keeping the prose above and below the table and rewriting only the rows. Then create every string that does not exist yet:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
PROJECT_ROOT="<absolute project root from Stage 1>"; cd "$PROJECT_ROOT" || exit 1
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags from the tracker file> ensure-labels --color d73a4a <the category strings>
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> ensure-labels --color 0e8a16 <the state strings>
```

Existing labels are never renamed or deleted on the tracker. The report names the ones that are now unused. Jira has no label registry, so `ensure-labels` is a no-op there and any string works. On `local` and `other` the role names are the strings and there is nothing to change: say so and stop.

**validation.** Show the current line. Run the proposed command once before writing it, always, including a value passed on the invocation. It is written only when it runs clean. It refuses to run: say what failed and write nothing.

**proof.** Three choices: a screenshot or recording tool the host offers, Orca's embedded browser (`orca screenshot`, only inside an Orca worktree), or command output rendered to an image. Report whether `gh` is 2.99.0 or newer, since `--attach` needs it. Writing nothing is a choice, and it is the default.

**docs.** Single context is `CONTEXT.md` at the root with decision records in `docs/adr/`. Multi context is `CONTEXT-MAP.md` pointing at per-area files, worth it only in a monorepo. Picking multi context writes the `CONTEXT-MAP.md` skeleton, since presence is what the reading skills check.

**peer.** List which of `codex`, `gemini`, `cursor-agent`, `opencode`, and `grok` are on the PATH. Print the consequence in one line before the question, because writing this line is the consent: `A second opinion reviewer sends the diff and a brief to that CLI on every review. The diff leaves this machine.` The options are the installed CLIs plus removing the line. `claude` is not offered, because a repo line cannot know which host will read it. Removing the line is always offered, and it is the default state.

**worktree.** Only inside an Orca worktree, or when the user asks anyway. Read `references/worktree.md` and follow it. Show the draft. An existing file is edited key by key, never replaced.

**pr-surface.** Flip the body of `## Pull requests as a request surface` in `docs/agents/issue-tracker.md` between `No.` and the paragraph that makes external pull requests enter triage as requests with attached code. GitHub only. On any other tracker, say it does not apply and stop.

**key.** Rewrite `Project:` and `Adapter flags:` in the tracker file, then verify:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
PROJECT_ROOT="<absolute project root from Stage 1>"; cd "$PROJECT_ROOT" || exit 1
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> check
```

This is the repair path for a setup run that wrote the config before the key was there. Running it with no change, just to verify, is a useful run: the user exports `LINEAR_API_KEY`, comes back, and confirms. Exit 3 or a missing variable: name the variable and leave the file as it is.

**pointer.** Move the `## Agent skills` block between `CLAUDE.md` and `AGENTS.md`, removing it from the file it left while preserving every setting line, including `Persona:`. A symlink pair is one file: report that and stop.

**persona.** With `archmage`, add or replace `Persona: archmage` in the block. With `off`, remove the line. An already matching setting is a no-op; disabling an absent setting creates no file or block. A bare `persona` invocation reports the current value and offers `archmage` and `off`; a named supported value writes immediately. Persona works without `docs/agents/issue-tracker.md`. An unsupported requested value writes nothing and explains the supported values. An unknown saved value behaves as off until changed. Apply a successfully changed voice to subsequent narration immediately, including this run's report, while preserving its required fields.

## Editing the block without clobbering it

The `## Agent skills` block runs from its heading to the next `## ` heading or the end of the file. Change only the one line this setting owns. Keep every other line's text and order exactly. The line is absent: insert it in the order below. The block is absent: create it in the pointer file, under the existing content, holding only the line being set.

```markdown
## Agent skills

Issue tracker: ...
Triage labels: ...
Validation: ...
Proof: ...
Domain docs: ...
Peer reviewer: ...
Persona: archmage
```

Removing a setting removes its whole line. Never leave a key with an empty value: `scan` reads an empty reviewer as a CLI name it cannot find.

## Stage 4: Report

```
Attuned
Setting: <name>
Was: <old value, or unset>
Now: <new value, or unset>
Wrote: <file, and which line or rows>
Check: <only for labels and key>
```

New sessions read this. Persona is also reread on each skill invocation, so the next invocation in this session sees the change. Switching trackers is a different job: run the repo setup skill again and name the tracker for that.

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/settings.md` | Stage 1 | Every setting, its file and line, its default, and which skills read it |
| `references/triage-labels.md` | Stage 3, labels | Label mapping template |
| `references/worktree.md` | Stage 3, worktree | `.worktreeinclude` and `orca.yaml` shapes |

## Scripts

`scripts/tickets.sh` talks to the tracker: `check` for the key flow and `ensure-labels` for the labels flow. GitHub needs `git` and `gh`. Linear and Jira need `python3` and their environment variables. `tickets.sh -h` prints usage.
