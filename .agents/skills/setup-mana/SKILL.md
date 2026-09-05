---
name: setup-mana
description: "Set up a repository for the other skills using your chosen tracker, asking where tickets live when that choice is missing. Supports GitHub Issues, Linear, Jira, markdown files under .scratch/, or a tracker you describe in a paragraph. Detects and writes the configuration and a pointer block in CLAUDE.md or AGENTS.md. Run once per repo, and again to refresh configuration or switch trackers. Use when asked to set up mana, set up this repo for the skills, choose the issue tracker, connect Linear or Jira, point the skills at a tracker, or /setup-mana."
argument-hint: "[blank] [github | linear | jira | local] [you-pick]"
disable-model-invocation: true
---

<!-- BEGIN MANA PERSONA -->
## Persona at invocation

Before conversational narration, read `Persona:` in the active project's `## Agent skills` block from `CLAUDE.md` or `AGENTS.md`. Prefer the file containing the block, then an existing file; ties use `CLAUDE.md`. A symlink pair is one file. Read the saved value anew on each invocation, including from a subdirectory using the project root. No accessible project or no line means ordinary behavior. Do not search another project or global settings for this preference.

During the `Persona at invocation` stage, `archmage` loads this skill's own [references/archmage.md](references/archmage.md) for the active workflow. `off` or an absent value leaves ordinary behavior active. An unknown value leaves ordinary behavior active and gets a brief explanation when conversational output is allowed; it does not stop the work. Explicit conversation instructions override the saved voice without writing settings. A request to enable Archmage for this workflow also loads the local reference.

Apply the voice only to lead-agent conversation. Deliverables, specialist roles, reply-only responses, and JSON-only output retain their contracts, with no added narration. End the persona with this workflow unless the user requests otherwise.
<!-- END MANA PERSONA -->

# Setup

Honor the user's explicit instructions and decisions already made in this conversation over this skill's workflow defaults. A rule this file states with never, or as read-only, is a gate: it holds whatever the conversation says, and an instruction to cross one is declined and reported. Continue authorized work; ask only about unresolved choices that would materially change the result. Preparing or reviewing work does not authorize publishing it.

If a skill rule requires a pause or leaves requested work unfinished, name and link to the exact SKILL.md and quote the rule. Then explain what decision or prerequisite is missing. Distinguish a required gate from your interpretation.

Use the tracker the user chose, then write the per-repo configuration the other skills read. Ask when that choice is missing. Re-running updates the same files in place.

## Operating principles

- **Reuse the answer.** A tracker chosen in this conversation or recorded in the tracker file needs no confirmation. Ask where tickets live only when the choice is missing or the user requests a switch without naming the destination. Everything else is detected, defaulted, written, and reported.
- **Write the answer, report the doubt.** The files get written on the answer, not on a passing check. A refused token or a missing key does not stop the run: write the config, mark the check unverified in the report, and name the exact variable to set. Never invent a value nobody gave you. Leave the template placeholder and say which line to fill.
- **A detected line is written only when it ran.** The validation command is found, run once, and recorded only when it finishes without a missing binary. Otherwise the line is left out, which costs nothing, because every skill that reads it falls back to detecting a command itself.
- **Secrets stay in the environment.** The files name the variable, never the value.
- **Edit the file that exists.** Prefer the file that already contains the `## Agent skills` block. When both files contain independent blocks, use `CLAUDE.md`. If neither contains the block, use the existing file, preferring `CLAUDE.md` when both exist. When neither file exists, create `AGENTS.md`. Never create the second file, and when one is a symlink to the other, edit once.

`SKILL_DIR` is the absolute directory this SKILL.md lives in. The Bash tool forgets variables between calls, so every block that runs the bundled script sets `SKILL_DIR` again on its first line.

## Execution spine

1. Detect (Stage 1).
2. Resolve where the tickets live (Stage 2).
3. Write (Stage 3).
4. Verify and report (Stage 4).

---

## Stage 1: Detect

Read, do not assume:

```bash
git remote -v
gh repo view --json url,nameWithOwner --jq '{url, repo: .nameWithOwner}' 2>&1 | head -3
ls -l CLAUDE.md AGENTS.md 2>/dev/null
sed -n '1,12p' docs/agents/issue-tracker.md 2>/dev/null
cat docs/agents/triage-labels.md 2>/dev/null
gh auth status 2>&1 | head -5
env | grep -o -E '^(LINEAR_API_KEY|LINEAR_TEAM|JIRA_BASE_URL|JIRA_EMAIL|JIRA_API_TOKEN|JIRA_PROJECT)=' | sort
git log --oneline -50 | grep -o -E '\b[A-Z][A-Z0-9]{1,9}-[0-9]+\b' | cut -d- -f1 | sort | uniq -c | sort -rn | head -3
ls .scratch 2>/dev/null | head -5
[ -n "${ORCA_WORKTREE_ID:-}" ] && command -v orca >/dev/null && echo orca: yes
```

**Detection order.** Exactly one tracker is marked as detected. Take the first rule that fires. GitHub is last on purpose: nearly every repo has a GitHub remote, and that alone is not evidence of where the tickets are.

1. An existing `docs/agents/issue-tracker.md`. Its `Tracker:` line is the current choice.
2. A live Linear or Jira connector the host exposes, or `orca linear team list --json` succeeding inside an Orca worktree.
3. `LINEAR_API_KEY` set, or `JIRA_BASE_URL` set.
4. A ticket key prefix in the last 50 commit subjects or in the branch names, like `ENG-12`. That points at Linear or Jira. Pick whichever of the two has any other signal, and Linear when neither does.
5. A `.scratch/` directory with ticket files in it.
6. A GitHub remote with `gh` logged in.
7. Nothing fires: no option is marked detected, and GitHub is listed first.

**Validation command.** In this order, stop at the first hit: the `Validation:` line in an existing `## Agent skills` block; `package.json` scripts named `test`, `typecheck`, `lint`, `check`; `Makefile` or `justfile` targets with those names; `pyproject.toml`, `Cargo.toml`, `go.mod` defaults. Run the candidate once. Keep it only when it exits 0 or fails on a test. A missing binary, or any failure before a test runs, means there is no candidate. Do not look for a second one, and do not ask.

**Existing labels.** `gh label list --limit 200 --json name --jq '.[].name'` on GitHub, or the connector's label list. A label maps to a role only on an exact match after stripping a leading `kind/`, `type/`, `status/`, `status: `, or `state:` and normalizing separators to `-`. So `kind/bug` maps to bug, and `status: blocked` maps to nothing. Two candidates for one role means neither wins and the role name is used.

Say what is present in two or three lines. Do not ask about any of it.

## Stage 2: Where do the tickets live

Use the latest explicit tracker choice from the request or earlier in this conversation, including `github`, `linear`, `jira`, or `local` on the invocation. Otherwise retain the valid choice in `docs/agents/issue-tracker.md`. A later request to switch trackers supersedes that earlier choice: use the newly named destination, or ask the question below when it is missing. A GitHub remote alone is only a detection signal. Under `you-pick`, use the detected option when no explicit choice exists and report it; with no detection, write nothing and stop, reporting `Tracker: none detected; pass github, linear, jira, or local`. Never ask under `you-pick`.

Use the platform's blocking question tool (`AskUserQuestion` in Claude Code; call `ToolSearch` with `select:AskUserQuestion` first if the schema is not loaded), falling back to the conversation where no such tool exists. One question, headed `Tracker`, with these four options and the detected one moved to the top:

| Option | Description |
|--------|-------------|
| GitHub Issues | Issues in this repo. Needs `gh` logged in with write access. Issues and pull requests share one number space. |
| Linear | Needs a Linear connector the host exposes, or `LINEAR_API_KEY`, plus the team key, like `ENG`. |
| Jira | Needs a Jira connector, or `JIRA_BASE_URL`, `JIRA_EMAIL`, and `JIRA_API_TOKEN`, plus the project key. |
| Local files | Markdown under `.scratch/`. Nothing to log in to and nothing to configure. |

The detected option's description starts with `Detected: ` and the signal in plain words, such as `Detected: commits reference ENG-118.` Exactly one option carries that prefix, and none does when nothing fired. Never write the word recommended.

Four options is the tool's maximum, so any other tracker arrives through the free text answer the tool always offers. Treat what the user types there as the `other` tracker: their sentence becomes the Conventions paragraph. A bare name gets one follow up for that paragraph, because the tracker file is useless without it.

**The key, for Linear and Jira only.** Take the first that hits, and ask nothing when one does:

1. The `Project:` line of an existing `docs/agents/issue-tracker.md`.
2. `LINEAR_TEAM` or `JIRA_PROJECT` in the environment. The bundled script already falls back to these.
3. The connector. `orca linear team list --json`, or the host's Linear or Jira tool set. One team or project comes back: take it. Several come back: that is the one follow up this skill is allowed, as a pick from the list. It is a fact, not a judgment, so it stays fast.
4. The most common ticket key prefix from Stage 1.

All four miss: ask once in plain words, `What is the Linear team key? It is the prefix on your issue ids, like ENG.` No answer: leave the template placeholder in `Project:` and `Adapter flags:`, write the file anyway, and name that line in the report. Never fall back to GitHub. GitHub and local files never get a follow up.

## Stage 3: Write

Load the template for the chosen tracker, fill it, write it. Nothing is shown for approval first. The report says what landed, and the files are ordinary markdown to edit.

1. `docs/agents/issue-tracker.md` from the matching template. On `other`, the user's paragraph goes under `## Conventions` and `Adapter flags:` stays `none`.
2. `docs/agents/triage-labels.md`, only when Stage 1 matched an existing label to a role. When every role uses its own name, write no file: a missing file already means the label string equals the role name, so the file would say nothing.
3. The `## Agent skills` block, in the selected pointer file. Merge the setup-owned lines into an existing block in place and leave every other line, including `Persona: archmage`, in its original order. Both files exist independently: prefer the file that already has the block, or `CLAUDE.md` when both do, and say so. If neither file has a block, use the existing file, preferring `CLAUDE.md` when both exist. Neither file exists: create `AGENTS.md`, because every agent reads it, and say in the report that renaming it to `CLAUDE.md` works just as well. A rerun never disables or removes a persona setting.

The block, carrying only the lines that have a value:

```markdown
## Agent skills

Issue tracker: <GitHub owner/repo | Linear team KEY | Jira project KEY | local files under .scratch/ | other>. See `docs/agents/issue-tracker.md`.
Triage labels: mapped. See `docs/agents/triage-labels.md`.
Validation: `<the command>`
```

The `Triage labels:` line is written only alongside its file. The `Validation:` line is written only when the command ran clean in Stage 1. Update or remove only setup-owned lines according to those conditions; preserve all other existing lines exactly, including `Persona:`. Setup never adds a persona setting. How pull request proof is captured, the domain docs layout, a second opinion reviewer, and the Orca worktree files all have working defaults, and none of them is a question this skill asks.

## Stage 4: Verify and report

Skip this whole stage for `local` and `other`. The bundled script rejects both trackers, and there is nothing to log in to.

With a connector, read the team or project through it and list its labels. Otherwise:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags from the tracker file> check
```

Exit 3 means the token was refused. A missing key makes the script exit before it asks. Neither is a stop, because the files are already written: record `unverified` with the exact variable and carry on to the report.

The check passed: create every label the roles need, using the mapping when one was written and the role names otherwise. Never create labels against a tracker that did not answer.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> ensure-labels --color d73a4a <the bug and enhancement strings>
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> ensure-labels --color 0e8a16 <the five state strings>
```

On `local`, one thing is worth a line: say whether `.scratch/` is gitignored, since tickets there are committed otherwise.

## Report

```
Setup
Tracker: <choice, and the key or repo>
Check: <user, project | unverified: set LINEAR_API_KEY, then run this again>
Labels: <created: a, b, c | already present | not needed on this tracker>
Validation: <the command and its result | none recorded, each skill detects one>
Wrote: <files written or updated in place>
```

Say that the files apply to new sessions and that editing them by hand is fine. Everything except the tracker and any existing persona setting was defaulted. To change one of those later, the label names, the validation command, how pull request proof is captured, a persona, a second opinion reviewer, or the worktree files, edit the block directly, or run the `attune` skill when it is installed. To switch trackers, run this one again and name the tracker (`setup-mana linear`), or say you want to switch and it asks.

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/issue-tracker-github.md` | Stage 3, GitHub | Tracker file template |
| `references/issue-tracker-linear.md` | Stage 3, Linear | Tracker file template |
| `references/issue-tracker-jira.md` | Stage 3, Jira | Tracker file template |
| `references/issue-tracker-local.md` | Stage 3, local | Tracker file template |
| `references/issue-tracker-other.md` | Stage 3, other | Tracker file template |
| `references/triage-labels.md` | Stage 3 | Label mapping template |

## Scripts

`scripts/tickets.sh` talks to the tracker: `check`, `ensure-labels`, and the operations the other skills use. GitHub needs `git` and `gh`. Linear and Jira need `python3` and the environment variables above. `tickets.sh -h` prints usage.
