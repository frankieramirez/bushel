# Finish the review

Load this after every reviewer has returned. It owns merge, validation, the report, and the three Stage 6 action modes.

## Stage 5: Merge findings

Turn several reviewer returns into one deduplicated, confidence-gated finding set. These are mechanics, applied uniformly, not per-finding judgment calls.

Keep the per-reviewer artifacts (`{run_dir}/{reviewer}.json`) as the source of detail. The compact returns are merge inputs; the artifacts hold `why_it_matters` and the full `evidence` array. Build a lookup keyed by reviewer plus fingerprint (normalized `file`, `line` as a string, whitespace-normalized lowercase `title`) so detail can be rehydrated after merging.

Apply in order:

1. **Suppress by confidence.** Drop every finding at confidence 0 or 25 silently. Keep a count.
2. **Quote-the-line gate.** Any finding at confidence 75 or 100 must carry `first_evidence`: the verbatim motivating line with `file:line`. Missing it means demote to 50. Record how many were demoted.
3. **Exact dedup.** Merge findings sharing a fingerprint. Union their `reviewers` lists, union and deduplicate their evidence, and keep the most specific `why_it_matters`.
4. **Semantic dedup.** Merge differently worded findings **only** when they describe the same defect *and* the same fix path. Findings with different failure modes or fix paths are not duplicates even on the same lines. Keep genuine disagreement visible rather than collapsing it.
5. **Cross-reviewer promotion.** A finding independently reported by two or more reviewers gets its confidence raised one anchor (max 100) and is marked corroborated in the report; agreement across independent lenses is the strongest signal available. `fast-pass` never counts toward this, and `existing-feedback` counts as an independent reviewer only when its finding was verified against the current code (see below).
6. **Confidence gate.** Drop confidence-50 findings from the primary set, **except** P0 (a critical-but-unverified concern is never silently dropped) and except items routed to a soft bucket below.
7. **Soft-bucket demotion.**
   - A current P0 or P1 stays primary unless it is a duplicate, was validated false, or is genuinely pre-existing. Never demote one just because only one reviewer found it.
   - For testing findings, keep at most one umbrella "this subsystem has no coverage" finding per changed subsystem when that is materially true; move narrower case-by-case coverage items to `testing_gaps`.
   - Move a single-reviewer P2 or P3 advisory from `testing` to `testing_gaps`, and from `maintainability`, `reliability`, `frontend-races`, or `adversarial` to `residual_risks`, unless it quotes an explicit violated contract or proves a current user-facing defect.
   - A claim that depends on unproven deployment topology (multiple instances, restarts, infrastructure behavior) is a residual risk unless the changed code or repo evidence establishes that condition.
8. **Pre-existing partition.** `pre_existing: true` findings go in their own section and do not count toward the verdict. Exception: when the new change *depends* on a pre-existing gap for correctness, flip it to `pre_existing: false` and keep it primary. Nearby cleanup stays pre-existing.
9. **Sort and number.** Sort by severity (P0 first), then confidence descending, then file path, then line. Assign stable `#` numbers in that order. **Reuse those numbers everywhere.** Never re-derive them per section.
10. **Detail hydration.** Rehydrate every retained finding's `why_it_matters` and `evidence` from the artifact lookup. Every retained finding needs a non-empty `why_it_matters` and at least one evidence item. If an artifact is missing or malformed, re-read the cited line and reconstruct only what you can directly verify. Never invent impact from the title. If the required fields still cannot be established, drop the candidate as malformed and note it in Coverage.
11. **Reconcile the harvested PR feedback.** Every item harvested in Stage 2b must land in exactly one bucket, and Coverage must report the counts:
    - **Became a finding** (possibly merged with a persona finding, which corroborates it).
    - **Already addressed** in the current code: name the file and line that shows it.
    - **Not a finding**: give the reason (the rule does not apply at this site, the flagged code was removed, it is a preference with no defect).
    Never leave a harvested item unaccounted for. A bot finding you cannot verify either way is a finding at confidence 50, not a silent drop.
12. **Partition the actionable queue.** Actionable is `gated_auto` or `manual` with owner `downstream-resolver`. Normalize any concrete P0 or P1 to `downstream-resolver` unless the report entry names the specific product decision, missing authority, external dependency, or release action that blocks implementation. A broad redesign, several related edits, or sensitive code is not such a blocker. Reviewer caution and `requires_verification: true` do not remove a fixable defect from the queue.
13. **Group by theme when findings span distinct concerns.** A group has a short title, the `#`s it covers, one line of context, and the preferred resolution with ordering ("decide X once, resolves #1 and #7, do #1 first"). Groups never merge findings or change severity, route, or numbering. A finding appears in at most one group; unrelated findings stay ungrouped. Skip grouping entirely when every finding is about the same thing. Mark each group as mechanical work or a decision gate.

## Stage 5b: Validation pass

Independent verification of the findings most likely to waste the user's time if wrong.

1. Select every remaining P0 and P1, plus every remaining actionable finding.
2. Put them in **one** batch, ordered by severity then `#`, and dispatch a single validator subagent. Eight findings is the normal cap; when more than eight P0 or P1 survive, expand the batch rather than dropping any or splitting into a second batch.
3. The validator gets, per finding: `#`, title, severity, file, line, `first_evidence`, `why_it_matters`, `suggested_fix`, and the same diff and scope rules the reviewers had. Its brief: *for each finding, independently verify against the actual code whether the defect is real as described. Return `{"#": n, "validated": true|false, "reason": "..."}` per finding. Default to `validated: false` when the evidence does not hold up: a false positive that reaches the user costs more than a missed nit.*
4. Run it foreground. The Agent call is the wait. No polling, sleeps, or status turns.
5. `validated: false` drops that finding and records the reason. On malformed validator output or infrastructure failure, drop the affected P2 and P3 findings but **keep** the affected P0 and P1 marked validation-degraded, and say so in Coverage. Prune triage groups after drops.

## Stage 6 report

Assemble the markdown report. Sections, in order (omit any that would be empty):

1. **Header.** Scope (what is being reviewed, the PR or branch), intent, and the reviewer team with a one-line reason per conditional lens.
2. **Triage Groups.** When groups exist: a compact table of `| Group | Findings | Context | Preferred resolution | Mechanical or decision |`. Every referenced `#` must appear in the findings below.
3. **Findings**, grouped by severity: `### P0: Critical`, `### P1: High`, `### P2: Moderate`, `### P3: Low`. Per finding make four things unambiguous: **what and where** (one scannable line: the symptom plus `file:line`, not the mechanism); **why it matters** (what breaks and who is hit, never a restatement of the code); **what response it needs** (a bug states its fix, a design call presents options and the tradeoff without forcing one, a coverage gap names the test and a precedent to mirror); and **how sure** (confidence, and whether more than one reviewer or the PR's own bots corroborated it, which is the strongest signal there is). For findings that came from existing PR feedback, say so and name the source, so the user knows the point is already visible on the PR.
4. **Existing PR feedback.** The Stage 5 step 11 reconciliation: what was harvested, what became findings, what was already addressed, what was dismissed and why. Name each bot by its actual login (`coderabbitai`, `github-actions`, and so on). Omit only when no PR was reviewed.
5. **Pre-existing.** Separate, does not count toward the verdict.
6. **Coverage.** Reviewers that ran and any that failed; the lite roster when it was used; suppressed counts by anchor; quote-the-line demotions; soft-bucket demotions; validator results, drops, and any degraded blockers; harvested-feedback counts by outcome; untracked files excluded from scope; residual risks; testing gaps; intent uncertainty; the run artifact path.
7. **Verdict.** `Ready to merge` / `Ready with fixes` / `Not ready`, plus the fix order when relevant.
8. **Actionable recap**, last. The prioritized list of what to do, each item carrying severity, `file:line`, the terse what, and its response type.

Hard constraints:

- **ASCII-safe.** No box drawing, no per-item horizontal rules, no Unicode arrows, no middot. Use `->`. A single `---` before the verdict is fine. Em dashes are fine in the *report* (it is yours, not the user's voice); they are banned in anything posted to GitHub.
- **Stable `#`s reused across every section.** A multi-file finding is one row with one `#`.
- **Escape literal `|` as `\|`** inside table cells.
- **Do not paste file contents or reprint the diff.** Cite `file:line` and spend words only on what the diff cannot show.
- **The closing stands alone.** A reader who sees only the last screen gets the verdict and the prioritized list.
- Cover every finding. Brevity governs expression, never coverage. A nit is one line; a P1 design call earns room.

Write the rendered report to `{run_dir}/report.md`, the merged findings to `{run_dir}/findings.json`, and `{run_dir}/metadata.json`:

```json
{
  "run_id": "<run-id>",
  "branch": "<git branch --show-current at dispatch time>",
  "head_sha": "<git rev-parse HEAD at dispatch time>",
  "pr": "<url or null>",
  "scope_mode": "local-aligned | pr-remote | branch-remote | standalone",
  "verdict": "<Ready to merge | Ready with fixes | Not ready>",
  "completed_at": "<ISO 8601 UTC>"
}
```

## Quality gates

Before delivering, verify:

1. **Every finding is actionable.** "Consider", "might want to", "could be improved" with no concrete action means rewrite it or drop it.
2. **No skim-level false positives.** Confirm the surrounding code was read: the bug is not handled elsewhere in the same function, the "unused" import is not used in a type position, the "missing" null check is not guaranteed by the caller.
3. **Severity is calibrated.** A style nit is never P0. An auth bypass is never P3.
4. **Line numbers are accurate.** A finding pointing at the wrong line is worse than no finding.
5. **No linter duplication.** Nothing the project's formatter or linter already catches.
6. **Nothing harvested vanished.** Every Stage 2b item is in one of the three buckets.

---

# Stage 6 action modes

Under `mode:agent`, none of these run: emit the JSON and stop.

## Report only

Nothing else happens. State the run artifact path so the findings can be picked up later.

## Apply mode: fix, commit, push

**Scope invariant.** Apply only when the working tree *is* what was reviewed: `local-aligned` or standalone. Under `pr-remote` or `branch-remote` the tree is not the reviewed head, so stop and say so rather than editing the wrong code.

Note whether the tree was already dirty (`git status --porcelain`) before touching anything.

**What to apply.** Default to applying every finding that is a clear improvement and a reversible edit, regardless of severity. The work lands as a visible, revertible diff, so leaving a clean fix unapplied "to be safe" is the failure mode, not the safe choice.

- **Apply** clear improvements: the common case.
- **Push back** when a reviewer was wrong: do not apply, and say why in the report.
- **Skip with judgment** on taste calls and conflicting suggestions, and surface what was skipped and why. Never silently drop.
- **Do not apply** `advisory` or `owner: human` items, or anything in the pre-existing section, unless the user asked for it.

Severity and confidence tell you what to do first, not whether to act.

**Order and mechanics.** Work P0 to P3. For each finding, apply `suggested_fix` when it still holds against the current code (adapted to surrounding style), otherwise implement a fix from `why_it_matters` and the evidence. Re-verify each finding against the current code before editing, since the tree may have moved since dispatch; if the target code no longer exists, mark the finding obsolete.

For a large actionable queue, dispatch fixer subagents in parallel, but never two on the same file at once. Serialize the overlapping ones.

**Verify.** Run the project's tests plus typecheck and lint for the touched area (targeted by default, broadened when fixes span files). If a fix breaks something, revert that fix and report it as a finding instead. An unverified fix is not finished. Never leave the tree red.

**Self-review the fix diff** against the pre-apply state before committing. If the same guard was added to several parallel surfaces, extract it or explain why the duplication is intentional. If an exported function now accepts a broader input, update the nearby types, docs, or tests that define the contract. If any of that changes files, re-run the affected checks.

**Commit and push.**

- Tree was clean before the review: commit the fixes as one labeled commit, `fix: address review findings`, or the repo's nearest convention. Follow the repo's commit rules (conventional prefixes, scope, changeset requirements).
- Tree was dirty before the review: the fixes are interleaved with in-flight work, so **do not** commit. Report what changed and let the user commit it with their own work.
- Push to the current branch. Never force-push, never rebase, never amend a pushed commit.
- Report the pushed SHA. If the push failed or was skipped, say so as the **first line** of the output: fixes that sit unpushed get orphaned when the PR is merged.

**Report back:** a table of `# | severity | file:line | finding | outcome (fixed / skipped + reason / obsolete)`, the verification results, the commit SHA, and the push status. Flag prominently any applied fix touching auth, a cross-service contract, or concurrency, since a passing test does not prove those safe.

## Comment mode: inline PR comments

**Read `references/voice.md` in full before writing any comment text.** The comments post under the user's name; sounding like a bot is the failure mode. No em dashes anywhere.

**Requires a PR** and requires the finding's file and line to exist in the PR diff (GitHub rejects an inline comment on a line outside the diff).

**What gets a comment:**

- Every primary actionable finding, one comment each, on its line.
- **Not** anything sourced from existing PR feedback. Those points are already on the PR; repeating them under the user's name is noise. Mention them in the chat summary instead.
- **Not** pre-existing findings, advisory items, residual risks, or testing gaps, unless the user asked for them.
- A P2 or P3 nit gets a comment only when it is genuinely worth a teammate's attention. When in doubt, leave it in the report.

**How to post.** Batch every comment into **one review submission** so the author gets a single notification instead of one per finding. Build a JSON payload and post it in a single call:

```bash
cat > "$RUN_DIR/review.json" <<'JSON'
{
  "commit_id": "<PR head sha>",
  "event": "COMMENT",
  "comments": [
    {"path": "src/foo.ts", "line": 42, "side": "RIGHT", "body": "<comment text>"},
    {"path": "src/bar.ts", "start_line": 10, "line": 14, "side": "RIGHT", "body": "<comment text>"}
  ]
}
JSON
gh api --method POST repos/OWNER/REPO/pulls/PR_NUMBER/reviews --input "$RUN_DIR/review.json"
```

- `event` is `COMMENT`, never `APPROVE` or `REQUEST_CHANGES`. Approving or blocking is the user's call, not this skill's.
- Use a **top-level review body only** when there is something worth saying about the change as a whole, and keep it to a couple of sentences in the user's voice. An empty or missing body is fine and usually better.
- Write bodies through a heredoc or a JSON file, never `echo` with escape sequences: `\n` posted literally shows up as one run-on line with visible backslashes.
- Multi-line findings use `start_line` plus `line`. Single-line findings use `line` alone.
- If the whole-review POST fails, fall back to posting the comments individually against `repos/OWNER/REPO/pulls/PR_NUMBER/comments` with `commit_id`, `path`, `line`, and `side`. Never fall back to `gh pr review` in a way that opens an unsubmitted draft.

**Verify what landed.** Read back the posted comments and confirm each body renders with real line breaks, not literal `\n`, and that no em dash survived. Fix any that did with a `PATCH` to `repos/OWNER/REPO/pulls/comments/COMMENT_ID`.

**Report back:** the review URL, a list of `# | file:line | first line of the comment`, and any finding that could not be commented on (line outside the diff) so the user knows it is only in the report.
