# Diff scope rules

All reviewers get these. They settle two things: which lines are yours to judge, and which tools are allowed to produce the evidence.

## The diff is handed to you

The orchestrator resolves the base, the file list, and the hunks before you exist, and passes them in your review context (inline, or as staged paths to Read). Never pick a base yourself. The `Scope mode:` line tells you how the hunks relate to the checkout you are sitting in.

| Scope mode | What the working tree holds | Evidence tools |
|---|---|---|
| `local-aligned` | The reviewed head. HEAD is the PR head branch and contains the PR head commit. | Read, Grep, `git blame`, `git log` on workspace paths. |
| `standalone` | The reviewed head. Current branch against its base. | Same as `local-aligned`. |
| `pr-remote` | Something else: another branch, a stale copy of the same name, a fork's twin. | `git show <ref>:<path>` at the reviewed head ref, or the hunks. Never the workspace. |
| `branch-remote` | Not the branch under review. | Same as `pr-remote`. |

## Remote modes

Under `pr-remote` and `branch-remote` any file in the checkout can differ from the reviewed head, including files the diff never touched. A workspace Read or Grep is not evidence here, even when the file looks identical.

- Read a file at the reviewed head with `git show <ref>:<path>`. `<ref>` is the `Reviewed head ref` from your context; the orchestrator sets it to a fetched ref such as `refs/review/pr-<n>-head`.
- Search that tree with `git grep <pattern> <ref> -- <path>`. Blame with `git blame <ref> -- <path>`.
- No head ref means the fetch failed. Work from the hunks alone. Do not guess a base and do not assume `main`.

## Three tiers

| Tier | Which lines | Treatment |
|---|---|---|
| In-diff | Added or modified in the hunks | Your main job. Any confidence anchor. |
| Diff-adjacent | Unchanged lines in the same function or block as a change, or old code the change newly reaches (a new caller of an existing buggy function) | Report it and name the interaction: new code plus old code together produce the defect. `pre_existing: false`. |
| Pre-existing | Unchanged code the diff neither touches nor interacts with | `pre_existing: true`. Lands in its own report section and does not count toward the verdict. |

To separate the last two, ask whether the diff changed the bug's reachability or its consequence. If nothing about it is new and you would have flagged the same line before this change, it is pre-existing. If the change made it reachable or made it wrong, it is diff-adjacent. When git history is what decides, add one provenance line from a targeted blame (the rule lives in `subagent-template.md`).

## Finding related code

When a claim rests on callers, implementations, or whether a construct shows up anywhere else, the search tool sets your recall. Use the strongest tier your harness exposes and drop down silently when one is missing.

1. Symbol-aware references (LSP or an equivalent MCP tool). Follows renames, re-exports, and barrel files. Rarely present in a reviewer harness.
2. Structural search (`ast-grep`, when installed). Matches the parsed tree, so formatting differences and hits inside strings or comments stay out of the result.
3. Plain `grep`. Always available, and the correct tool for lexical questions anyway: config keys, string literals, log messages.

All three miss usages hidden behind dynamic dispatch, reflection, dependency injection, string-keyed routing, generated code, and consumers outside the repo. That gap only bites a claim of absence: "unused", "no other caller", "safe to remove". For those, when coverage was grep-only or a hiding construct plausibly applies, either record the boundary in `residual_risks` (for example `callsite completeness: grep-only`) or drop the finding one anchor. Never assert an absence you could not check. A finding that never depended on exhaustive coverage needs no such note.

In the remote modes every tier points at the workspace by default. Route it through `git show` and `git grep <ref>` instead.
