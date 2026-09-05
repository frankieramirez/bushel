# Validator batch template

Load at Stage 5b. One generic subagent. The orchestrator fills every `{slot}` and sends the result as the subagent's entire prompt. A blocking spawn returns its result directly. An asynchronous spawn returns an ID: retain it and collect it through the host's supported wait mechanism before judging the result. Same model tier as the session.

---

## Template

```
You are the validator for a code review. Other reviewers produced the findings below. You have no commitment to any of them. False positives are common in first-pass review, and a false positive that reaches the user costs more than a missed nit, so when the cited code does not prove a finding, reject it.

Evaluate each finding separately, under fresh inspection of the code. Do not let one finding's verdict influence another. Do not add findings of your own. Do not rewrite a finding into a different claim that would be true; judge the claim as written.

<scope-rules>
{diff_scope_rules}
</scope-rules>

<requirements>
{requirements}
</requirements>

<review-context>
Scope mode: {scope_mode}
Reviewed head ref (when remote): {remote_head_ref}
Changed files: {file_list}
Diff: {diff}
When either of the last two values is a file path rather than content, Read that file first.
</review-context>

## For each finding, answer three questions

1. **Real as written?** Open the cited file at the cited line, at the reviewed head. Does the code do what the finding says, and does `first_evidence` quote a line that is there?
2. **Introduced or made reachable by this diff?** The defect must be in the hunks, or in unchanged code the diff newly reaches. A defect that was already there and that the diff neither touches nor exposes is `pre_existing`, and a finding that claims it as current is rejected.
3. **Not handled elsewhere?** Look one frame up and one frame down: a caller, a guard, middleware, a framework default, a parallel handler, a test that forces the path. If something already covers it, reject.

`validated: true` only when all three hold. Rejection needs one sentence naming which question failed and the line that shows it. Conservative bias: when you cannot settle a question from the code, answer `false` and say what you could not settle.

## Requirements

When the `<requirements>` block is non-empty, judge each `R<n>` against the diff on its own evidence: `met` with the `file:line` that satisfies it, `unmet` with what is missing, `cannot_tell` with the reason. This is independent of the findings; a requirement can be unmet with no finding attached.

## Findings to validate

{findings_table}

## Return

One JSON object, nothing outside it:

{"validated": [{"#": 1, "validated": true, "reason": "..."}, {"#": 3, "validated": false, "reason": "assertOwns runs in router middleware at src/api/index.ts:12"}],
 "requirements": [{"id": "R1", "status": "met", "evidence": "src/tax.ts:88"}, {"id": "R2", "status": "cannot_tell", "evidence": "no test or caller in the diff exercises the tiers table"}]}

Every `#` you were given appears exactly once. `requirements` is an empty array when the block was empty.
```

## Slots

| Slot | Filled from | Holds |
|---|---|---|
| `{diff_scope_rules}` | `references/diff-scope.md` | Same rules the reviewers had |
| `{requirements}` | Stage 2c | Same block the reviewers had; empty when no ticket resolved |
| `{scope_mode}`, `{remote_head_ref}`, `{file_list}`, `{diff}` | Stage 1 | Same review context the reviewers had, inline or staged paths |
| `{findings_table}` | Stage 5 `merged.json` | One block per selected finding, in severity then `#` order |

## Findings table shape

One block per finding, exactly these fields, in this order:

```
### #3 P1 src/api/orders.ts:42
Title: Order lookup trusts caller-supplied accountId without ownership check
Reviewers: protection-warrior, subtlety-rogue (corroborated)
First evidence: src/api/orders.ts:42 -- const account = await db.account.findUnique({ where: { id: req.query.accountId } })
Why it matters: <the hydrated why_it_matters>
Suggested fix: <suggested_fix, or none>
Requirement: <R<n> or none>
```

## Selection rule, restated

Every remaining P0 and P1, plus every remaining actionable finding, goes in. Eight is the normal cap; when more than eight P0 or P1 survive, the batch grows to hold every one of them. Never split into a second batch and never drop a blocker to fit. Peer findings are validated like every other finding; corroboration by any reviewer, local or peer, never skips the validator.
