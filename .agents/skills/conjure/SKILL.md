---
name: conjure
description: "Turn a finished plan, spec, or decision map into ready-for-agent GitHub issues, each with an agent brief and wired in dependency order, so a build session can take them one at a time. Use when asked to file the build tickets, break this spec into issues, turn the plan into tickets, slice the work, or /conjure."
argument-hint: "[map number | issue URL | spec path | blank for the plan in this conversation] [you-pick]"
disable-model-invocation: true
---

<!-- BEGIN MANA PERSONA -->
## Persona at invocation

Before conversational narration, read `Persona:` in the active project's `## Agent skills` block from `CLAUDE.md` or `AGENTS.md`. Prefer the file containing the block, then an existing file; ties use `CLAUDE.md`. A symlink pair is one file. Read the saved value anew on each invocation, including from a subdirectory using the project root. No accessible project or no line means ordinary behavior. Do not search another project or global settings for this preference.

During the `Persona at invocation` stage, `archmage` loads this skill's own [references/archmage.md](references/archmage.md) for the active workflow. `off` or an absent value leaves ordinary behavior active. An unknown value leaves ordinary behavior active and gets a brief explanation when conversational output is allowed; it does not stop the work. Explicit conversation instructions override the saved voice without writing settings. A request to enable Archmage for this workflow also loads the local reference.

Apply the voice only to lead-agent conversation. Deliverables, specialist roles, reply-only responses, and JSON-only output retain their contracts, with no added narration. End the persona with this workflow unless the user requests otherwise.
<!-- END MANA PERSONA -->

# Conjure

Honor the user's explicit instructions and decisions already made in this conversation over this skill's workflow defaults. A rule this file states with never, or as read-only, is a gate: it holds whatever the conversation says, and an instruction to cross one is declined and reported. Continue authorized work; ask only about unresolved choices that would materially change the result. Preparing or reviewing work does not authorize publishing it.

If a skill rule requires a pause or leaves requested work unfinished, name and link to the exact SKILL.md and quote the rule. Then explain what decision or prerequisite is missing. Distinguish a required gate from your interpretation.

The deciding is done. A map, a spec, or the plan in this conversation says what should be true. This skill slices that into build tickets sized for one session each, writes a brief on every one, and files them in the order they can be built.

## Operating principles

- **Decisions first.** A ticket is filed only when its question is already answered. An open decision goes back to the person, never into a brief.
- **One session per ticket.** A slice that needs two sessions is two tickets with a blocking edge.
- **The brief is the contract.** A later build session reads the brief and nothing else. Write it for a reader who has none of this conversation.
- **Refer by name.** In anything a person reads, use the ticket title with the link wrapped inside it.
- **Write to the tracker through the contract.** On Linear or Jira, use the host's connector for `create`, `ensure-labels`, `wire`, and `comment` when one is present. Otherwise run `scripts/tickets.sh`. GitHub always goes through the script. Never write the tracker any other way.

`SKILL_DIR` is the absolute directory this SKILL.md lives in. The Bash tool forgets variables between calls, so every block that runs the bundled script sets `SKILL_DIR` again on its first line.

## Scripts

`scripts/tickets.sh` creates tickets, labels, blocking edges, and comments on GitHub (`git` and `gh` only), Linear, or Jira (`python3` and the tracker's environment variables). Exit 3 means the token cannot write. `tickets.sh -h` prints usage.

## Arguments

Parse tokens, then treat the remainder as the source.

| Token | Effect |
|-------|--------|
| `you-pick` | Accept every recommended slice and edge in the Stage 2 round. Same meaning as the user saying "make the decisions" or "you pick". |

| Input | Source |
|-------|--------|
| none | The plan or spec already in this conversation. If none is obvious, stop and ask. |
| number or issue URL | That issue. Labelled `scry:map` (or `wayfinder:map`): the map. Any other issue: its body is the spec. |
| a path | That file is the spec |

## Execution spine

1. Load the source and the project's language (Stage 1).
2. Slice it and agree the order (Stage 2).
3. File the tickets and wire the edges (Stage 3).
4. Record the build order and report (Stage 4).

---

## Tracker

Read `docs/agents/issue-tracker.md` when it exists. Its `Tracker:` line names the tracker and its `Adapter flags:` line gives the flags for the bundled script. Missing file: GitHub, no flags. On a GitHub Enterprise host, pass `GH_HOST=<host>` inline too.

The operations below are `view`, `ensure-labels`, `create`, `wire`, and `comment`. On Linear or Jira, when the host exposes a connector for that tracker, use it for them; it is already authenticated. Inside an Orca worktree (`ORCA_WORKTREE_ID` is set and `command -v orca` succeeds), `orca linear` is such a connector for Linear; `orca linear --help` lists its operations. Otherwise run the script with the adapter flags. GitHub always goes through the script. Never mix the two in one run. For `local`, write the tickets as `references/scratch.md` describes. For `other`, follow the tracker file's Conventions by hand.

## Stage 1: Load

**A map.** Fetch it:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> view ID
```

`ID` is a tracker id or a GitHub, Linear, or Jira issue URL; the script extracts the id. Read Destination, Notes, Decisions so far, and every owning document Notes names. Then check for unfinished deciding: on GitHub,

```bash
gh issue list --state open --search "Part of #NUMBER" --json number,title,labels
if ! children="$(
  gh api --paginate repos/OWNER/REPO/issues/NUMBER/sub_issues \
    --jq '.[] | select(.state == "open") | "\(.number)\t\(.title)\t\([.labels[].name] | join(","))"'
)"; then
  echo "Unable to verify map children" >&2
  exit 1
fi
printf '%s\n' "$children"
```

and elsewhere the way the tracker file's Wayfinding operations section lists a map's children. An open child still labelled `scry:*` or `wayfinder:*` means a decision is still pending. Stop, name those tickets, and say the map is not ready to build from. A failed `gh api` call stops the workflow.

**A spec path or another issue.** Read it in full.

**Blank.** Use the plan in this conversation.

Read `CONTEXT.md` when it exists, and any ADR in the area. Use the project's words in every title and brief.

Resolve the label strings. When `docs/agents/triage-labels.md` exists, read it and take the strings it maps for `bug`, `enhancement`, and `ready-for-agent`. Missing file: the string equals the role name.

Write two lines before slicing:

```
Destination: <what is true when every ticket is closed>
Labels: <category string>, <ready string>
```

## Stage 2: Slice

Load `references/slicing.md` and follow it. Present the slices as one round: each with a title, a one-line summary, and the tickets it waits on, plus a recommended order. Wait for the answer. `you-pick` accepts the recommendations.

If the source still has an open decision that no slice can avoid, stop here and say what needs deciding.

## Stage 3: File

Load `references/agent-brief.md`. Ensure the labels exist once per session:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> ensure-labels --color d73a4a <category strings>
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> ensure-labels --color 0e8a16 <ready string>
```

Create one issue per slice. The body is the brief. When the source is a map, the first line of the body is `Builds toward: [<map title>](<map url>)`. Do not write `Part of #` and do not attach the ticket as a child of the map; that would make a map-walking session mistake it for a decision ticket.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> create "<title>" --label <category string> --label <ready string> <<'EOF'
Builds toward: [<map title>](<map url>)

<brief from references/agent-brief.md>
EOF
```

Wire blocking edges in a **second pass**, once every ticket has an id:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> wire CHILD_ID BLOCKER_ID
```

Exit 3 from the script means this token cannot write issues (usually HTTP 403). Read `references/scratch.md` and follow it. Do not keep retrying.

## Stage 4: Record

When the source is a map, post one comment on it listing the tickets in build order, each as its title wrapped around its link:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/tickets.sh" <adapter flags> comment MAP_ID <<'EOF'
## Build order

1. [<title>](<url>)
2. [<title>](<url>), after 1
EOF
```

## Report

```
Conjure: <destination in a few words>
Filed: <n> tickets
Order:
  1. <title> (<url>)
  2. <title> (<url>), after 1
Unspecified: <anything the source left open, or none>
```

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/slicing.md` | Stage 2 | What a one-session slice is, and the round that agrees the order |
| `references/agent-brief.md` | Stage 3 | The brief a build session reads |
| `references/scratch.md` | Stage 3, exit 3 only | Local tickets when GitHub writes fail |
