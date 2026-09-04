# GitHub operations

`scripts/map.sh` owns every write that shapes the map. It needs `gh` authenticated against this checkout. Set `GH_HOST` on every call when the remote is GitHub Enterprise (derive the host from `gh repo view --json url --jq .url`).

`SKILL_DIR` is the absolute directory of this skill. Set it on the first line of every Bash call.

`frontier` and `view` read the older `wayfinder:*` labels as their `scry:*` equivalents, so a map filed before 0.11.0 still walks. Nothing writes the old names.

| Subcommand | Arguments | What it does |
|------------|-----------|--------------|
| `ensure-labels` | none | Creates `scry:map` and `scry:{research,prototype,grilling,task}` if missing |
| `create-map` | `TITLE`, body on stdin | Opens an issue labelled `scry:map`. Prints `number<TAB>url` |
| `create-ticket` | `MAP_NUMBER TYPE TITLE`, body on stdin | Opens a child labelled `scry:TYPE`, attaches it as a sub-issue. Prints `number<TAB>url` |
| `wire` | `CHILD_NUMBER BLOCKER_NUMBER` | CHILD is blocked by BLOCKER (database id under the hood) |
| `frontier` | `MAP_NUMBER` | Open, unblocked, unclaimed children, map order. TSV: `number<TAB>title<TAB>type<TAB>url` |
| `claim` | `NUMBER` | Assigns the issue to the current `gh` user. Exits 1 if someone else already holds it |
| `view` | `NUMBER` | Prints number, title, url, state, labels, assignees, body |
| `parent` | `NUMBER` | Prints the parent map number, or empty |
| `comment` | `NUMBER`, body on stdin | Posts a comment |
| `close` | `NUMBER` | Closes the issue |
| `update-body` | `NUMBER`, body on stdin | Replaces the issue body |

Without `owner/repo`, the script uses `gh repo view` in this checkout. The script passes `--repo` to every `gh issue` and `gh label` call so an explicit owner/repo wins.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Done |
| 1 | Usage or unexpected `gh` failure |
| 3 | GitHub refused the write (HTTP 403, or "Resource not accessible"). Load `scratch.md` |

## Conventions the script already encodes

- Children are GitHub sub-issues. When that API is missing or refused, `create-ticket` writes `Part of #<map>` at the top of the child and the agent adds a task-list line on the map via `update-body`. If that edit is also refused, the script closes the issue it just opened and exits 3.
- Blocking uses native `blocked_by`. When that API is missing or refused, `wire` writes `Blocked by: #<n>` at the top of the child.
- The frontier finds children via sub-issues, then `Part of #<map>` on open issues and task-list `#N` lines on the map. It does not scrape every `#N` in the map body. It drops any child with an assignee or an open blocker.
- Claim is the assignee. The script refuses when another login already holds the ticket.

Do not call `gh issue create`, `gh api .../sub_issues`, or `gh api .../dependencies` yourself. The script is the one place those sequences live.
