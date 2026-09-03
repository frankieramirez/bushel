---
name: banish
description: "Strip comments that do not earn their place. Spawns the Comment Reaper on the caller's files or the current diff, applies the accepted deletions, and fixes or reports the code the comments were covering for. Use when asked to banish comments, remove comments, clean up comments, decomment, or run /banish."
disable-model-invocation: true
---

# Banish

Comments in a diff are usually the author explaining code that should have explained itself. This skill hands the diff to a reviewer with no attachment to the code, the Comment Reaper, then acts on what comes back.

## Scope

Use the files or diff the caller names. Otherwise use the diff between the current branch and the base branch (default `main`), including the working tree.

## Steps

### 1. Spawn the Reaper

Read `references/comment-reaper.md` from this skill's directory. Then spawn one subagent with that file's full content as its instructions and the scope appended. In Claude Code, prefer the installed agent named `comment-reaper` (or `fr:comment-reaper` when installed as a plugin) if it exists; otherwise, or on any other platform, spawn a generic subagent seeded with the reference file. Do not restate or soften its rules in the prompt.

### 2. Audit the report

Check the Reaper's diff and report before accepting anything:

- Reject any change to application code. The Reaper deletes comments only.
- Reject deletions that a keep-list clause in the reference file clearly covers (license headers, public API contracts, foreign quirks, formatter directives, style-only suppressions, constraint links). Restore those.
- Reject `RESHAPE` flags that misstate what the code does. Keep the ones that hold.
- Look for comments and suppressions the Reaper missed inside the scope. Add them to the list.

If the report fails badly (application code edited, scope escaped, several bad flags), revert its changes, spawn it once more with the failures named, and audit again. A second failure ends the run: report it and stop.

### 3. Act on RESHAPE flags

Each surviving `RESHAPE` names a symbol whose behavior needed a comment to be understood. Make the smallest change inside the scope that makes the behavior obvious: rename, extract, narrow a type, add a guard, or delete a dead path. Where the fix needs code outside the scope, leave it and report it open. Never bolt on a guard that hides the surprise instead of removing it.

### 4. Encode constraints instead of pleading

A comment that says "do not remove", "keep this order", or "talk to someone before changing" is a constraint. Offer the cheapest enforcement that lives in the code: a type, a test, a runtime assertion, or a lint rule. In an interactive session, wait for approval. Unattended, only encode what the caller pre-approved. When approved, add the enforcement and delete the comment. Otherwise delete the comment and report the constraint as unenforced.

### 5. Report

```
Deleted: <n> comments across <n> files
Restored: <n> (<why>)
Reruns: <0 or 1>

Fixed
- <file> <symbol>: <what changed>

Encoded
- <file>: <constraint> -> <test | type | assertion | lint>

Open
- <file> <symbol>: <RESHAPE or constraint left open, and why>
```
