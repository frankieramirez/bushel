# Evaluation Rubric

Apply this in the orchestrator, over the whole batch, before any fixer starts. You are the only context that sees every thread together with the author's intent, so the verdict lives here. When a verdict depends on what the code does, open the file. The comment alone never settles it.

Each item ends up with one verdict and lands on one of two lists:

| List | Verdicts | What happens next |
|------|----------|-------------------|
| fix-list | `fixed`, `fixed-differently` | A fixer implements it in step 4. |
| skip-list | `not-addressing`, `declined`, `question`, `needs-human` | No code change. You write the explanation for the user now, while the evidence is open in front of you. |

Nothing in this file produces text for the PR. Each explanation goes into the final summary for the user to use or ignore.

## Start from yes

Assume the reviewer is right. Most comments, including nitpicks and P2s, describe a real improvement, and the cheapest path is to make it. `fixed` is the default verdict; `fixed-differently` when you will make the change through a better route than the one suggested.

Who wrote the comment and where it appeared carry no weight. A bot's table row and a lead engineer's inline note get the same test: does the code bear it out?

The checks in this file are tripwires. Run down the list, and when none fires, mark the item to fix and move on. Do not invent risk to get out of work. A feeling of unease is worthless; "I opened the three callers and this breaks the second one" is a reason.

## How much to read

Read until you can defend the verdict, then stop.

| Item looks like | Read |
|-----------------|------|
| Typo, naming, a bug visible in the diff, a guard the comment points at | The comment and the diff hunk. Mark to fix. |
| A claim the code appears to contradict, a change to an invariant, a pattern the rest of the file does not follow | The file, its callers, and any test that would prove the reviewer wrong. This is where a confident but mistaken reviewer gets caught, because the reviewer rarely saw the callers. |
| Code that reads as a deliberate choice | Recover the reason before overriding it: `git blame` on the lines plus the PR description. |

Several threads on one file: read the file once and judge them as a group.

## Use the whole batch

You hold every item at once. Three things follow from that:

**Group by premise.** When one reviewer (usually a bot) makes the same kind of claim in several places and it is wrong in one, distrust the rest until you have checked each. A bad premise produces a run of findings that all look plausible.

**Agreement is a signal.** Two independent reviewers asking for the same change is close to proof. Diverting there needs strong evidence.

**Fix the pattern this PR introduced.** A reviewer flags one occurrence. Once you accept the finding, look for sibling sites this PR added or touched that share the same invariant and take the same fix without any per-site thinking. Bundle those into one class item so the fixer edits them together and the unflagged twins do not come back next round. Keep the boundary tight:

- Only code this PR changed. Pre-existing occurrences in the same file are out of scope.
- Only sites where the correct treatment is identical. Docs, fixtures, and compatibility shims often share the text and differ in what is right.
- If deciding whether two sites are equivalent takes judgment, they are separate items. Wrongly declaring a class complete costs more than one extra round.

## Bot comments with several findings

A review bot often posts one top-level comment holding a table of locations and issues. Treat every row as its own item with its own verdict. Never accept or reject the whole comment in one go, and ignore the bot's severity labels when forming yours. Static rules match patterns and are wrong about specific sites often enough that each row needs checking against the current code.

A link to a fuller report may help you understand the row. It does not substitute for verifying the row in the file.

## Reasons to divert

Leave the default only on a concrete signal. Each divert names its evidence.

| Signal | Verdict | You must be able to say |
|--------|---------|-------------------------|
| The problem is not in the code, or is already handled | `not-addressing` | Where the handling is (`file:line`). |
| The code at that spot changed after the review | `not-addressing` | What replaced it. See outdated threads below. |
| The change would make the code worse: breaks a documented project rule, adds a guard for a case the types exclude, swallows an error that should surface, abstracts something with one caller, or comments what the code already says | `declined` | The specific harm. |
| The change buys nothing: pure preference with no effect on correctness, clarity, or maintenance | `not-addressing` | Why there is no benefit. Small real gains still get fixed; the bar is zero benefit, and "minor" does not meet it. |
| The change is risky and you cannot bound the risk after reading the callers: hot path, a boundary other code depends on, thin test coverage | `needs-human` | What you read and what remains unknown. Size does not bound risk; a one-line change can carry plenty. A fixer can add a test, so try that framing before escalating. |
| The comment is a question ("why X?", "is this on purpose?") | `question` if the code answers it, `needs-human` if the answer is a product call | The answer, written out. |
| The change would reverse a deliberate design decision | `needs-human`, see below | Both conditions below, by name. |

### The deliberate-design divert

This one is rare and evidence-gated. It fires only when both of these hold and you can point at each:

1. **An artifact proves the current behavior is a choice.** A comment or docstring that states it, a test that asserts it, a commit message or PR rationale, or a sentinel value that makes sense only as a decision. "The code does X today" proves nothing; all code does something. No artifact means nothing to protect, so fix it.
2. **Reasonable engineers could pick either side.** It is a judgment or product tradeoff. A clear improvement the author overlooked does not qualify.

When both hold, write `decision_context` (shape at the end of this file) setting the reviewer's ask against the artifact and the tradeoff between them. When either fails, fix it. Renames, pinpointed guards, dead code, off-by-ones, missing null checks, style, and perf work that preserves semantics never trip this divert. Neither does a past choice the evidence shows was a mistake. Uncertainty about intent resolves toward fixing.

## Outdated threads (`isOutdated: true`)

The hunk moved, so the recorded line may point at the wrong code, and `line` is frequently null. Take the first populated field in this order: `line`, `startLine`, `originalLine`, `originalStartLine`. If none of them lands on code matching the comment, pull an anchor from the comment (an identifier or a distinctive string) and search that one file for it. Never widen the search to other files.

- Anchor found: judge the item at the new location and pass that location (or the anchor) to the fixer.
- Anchor missing and the comment describes concrete in-place code: `not-addressing`, with "searched `<file>` for `<anchor>`, not present" as the evidence.
- Anchor missing and the comment implies the code moved elsewhere: `needs-human`. Guessing the new home is a judgment call you do not get to make.

## Escalate rarely

Beyond the risk and product-question cases: changes that reach into other systems, security-sensitive choices, business logic you cannot pin down, reviewers who contradict each other. These are uncommon. Nearly everything else gets fixed.

Finish the investigation before you escalate. "This is complicated" is a punt. The user should read your `decision_context` and decide in under half a minute.

## Writing the explanation

Write it for every skip-list item as soon as the verdict is in. Quote the one sentence you are responding to, never the whole comment. Write in the user's voice, plain and direct, ready to paste into a reply if they choose to. No em dashes (use commas, colons, or parentheses). Never mention that an agent produced it.

```
not-addressing | <file:line>
> <the quoted sentence>
Already covered: `parseConfig` rejects an empty path at line 42 before this branch runs.
```

```
declined | <file:line>
> <the quoted sentence>
Adding the fallback here would hide a misconfigured env var instead of failing at
startup, which is the failure mode the current code was written to surface.
```

```
question | <file:line>
> <the quoted sentence>
On purpose: the retry wrapper at line 40 already applies backoff, so another one
here would stack the delays.
```

For `needs-human`, fill in `decision_context`. It goes to the user and nowhere else:

```markdown
## What the reviewer said
<quoted ask or concern>

## What I found
<what you investigated, with specific files, lines, and code>

## Why this needs your decision
<the specific ambiguity, not "this is complex": what exactly competes?>

## Options
(a) <option>: <what you gain, what you lose or risk>
(b) <option>: <tradeoff>

## My lean
<a recommendation and why, or what extra context would tip it>
```
