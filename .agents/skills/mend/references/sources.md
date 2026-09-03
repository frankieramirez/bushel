# Sources

Recover why each side of a conflict exists before anyone edits a file.

## Commits that touch the file

```bash
# ours history of this path
git log --oneline -n 15 HEAD -- "$path"
# theirs
git log --oneline -n 15 THEIR_REF -- "$path"
# commits on either side since they diverged, for this path
git log --oneline --left-right HEAD...THEIR_REF -- "$path"
```

Read the full messages of the commits that actually overlap the hunk, not only the subjects.

## The hunk itself

```bash
git show :1:"$path"
git show :2:"$path"
git show :3:"$path"
```

`git checkout --conflict=diff3 -- "$path"` leaves the file with base/ours/theirs markers if you need the base in-place. Weaver can also read the stages without rewriting the working tree.

## GitHub, when it is this repo

```bash
gh api "/repos/OWNER/REPO/commits/$sha/pulls"
```

A commit that landed via PR has a title and a body that usually state the intent more clearly than the hunk. An issue referenced in the message does too. Skip this when `gh repo view` fails. Use `gh --jq` for any JSON shaping.

## What to write down

For each file, two sentences: what ours was doing, what theirs was doing. If a sentence would be "updated the file", you have not read enough. Name the behavior.

The operation's goal is already on the Stage 1 `goal` line. A rebase goal is the commit being replayed: theirs is the change that must land. A merge goal is combining the two branches: both sides are first-class.
