# Finish the review

Load this after every reviewer has returned. It owns merge, validation, the report, the `mode:agent` contract, and the three Stage 6 action modes.

## Stage 5: Merge findings

Turn several reviewer returns into one deduplicated, confidence-gated finding set. The mechanical gates run in `scripts/review.sh merge`, uniformly and deterministically. The judgment calls stay with you, applied between two runs of the script.

The per-reviewer artifacts (`{run_dir}/{reviewer}.json`) are the source of detail: `why_it_matters` and the full `evidence` array. The script reads them directly. A reviewer whose artifact is missing or malformed is read from `{run_dir}/returns/{reviewer}.json`, the compact return Stage 4 saved, and its findings carry `hydration: "return-only"` so you know the detail is thin.

### Pass 1: the mechanical gates

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/review.sh" merge "$RUN_DIR" --roster <comma-separated reviewer identifiers you dispatched, fast-pass included>
```

The script writes `{run_dir}/merged.json` and applies, in order:

1. **Fast-pass clamp.** Every `fast-pass` finding above 50 becomes 50.
2. **Suppress by confidence.** 0 and 25 are dropped; counts kept per anchor.
3. **Quote-the-line gate.** A 75 or 100 with no `first_evidence` (nor an `evidence[0]`) becomes 50.
4. **Exact dedup.** Findings sharing a fingerprint (normalized path, line, whitespace-normalized lowercase title) merge: `reviewers` union, evidence union, the longest `why_it_matters`, the more conservative `autofix_class` and `owner`, `requires_verification` OR, `pre_existing` AND, the higher severity and confidence.
5. **Cross-reviewer promotion.** Two or more independent reviewers raise a finding one anchor (max 100) and mark it `corroborated`. `fast-pass` is never independent; Lore Bard counts only when its own contribution was 75 or above, which means it quoted the current code; a peer counts only when its artifact says `independence_verified: true`.
6. **Confidence gate.** At 50 after promotion: P0 stays with `gate: "p0_escape"`; Marksmanship Hunter items are tagged `soft_candidate: "testing_gaps"`; Balance Druid, Restoration Shaman, Windwalker Monk, Havoc Demon Hunter, Augmentation Evoker, and Discipline Priest items are tagged `soft_candidate: "residual_risks"`; Lore Bard items are tagged `soft_candidate: "harvested"` because an unverifiable bot finding is never a silent drop; anything else is dropped into `dismissed` with reason `confidence gate`. Any single-reviewer P2 or P3 `advisory` from the soft-bucket reviewers is also tagged at any confidence. The script tags; it never moves a tagged item for you.
7. **Partition.** `pre_existing: true` to `pre_existing`, tagged items to `soft_candidates`, the rest to `findings`.
8. **Sort and number.** Severity, then confidence descending, then file, then line. Stable `#` from 1 across `findings` then `pre_existing`.

Read `merged.json`. Its `counts` block is the raw material for Coverage; its `dismissed` array already holds the gate-6 drops with `stage: "merge"`.

### Your judgment, on `merged.json`

Edit the object and write it to `{run_dir}/reconciled.json`. Work through every item in `findings`, `soft_candidates`, and `pre_existing`:

1. **Semantic dedup.** Merge differently worded findings **only** when they describe the same defect *and* the same fix path. Union both `reviewers` and `independent_reviewers`; never add a name to `independent_reviewers` merely because it is in `reviewers`. Findings with different failure modes or fix paths are not duplicates even on the same lines. Keep genuine disagreement visible.
2. **Soft-bucket decisions.** For each `soft_candidates` item set `bucket`: `primary` when it quotes an explicit violated contract, a dangling reference, or proves a current user-facing defect; `testing_gaps` or `residual_risks` otherwise, matching its tag. For a `harvested` tag, reconcile it under step 6 below. A current P0 or P1 stays primary unless it is a duplicate, was validated false, or is genuinely pre-existing; never demote one because only one reviewer found it. For testing findings, keep at most one umbrella "this subsystem has no coverage" finding per changed subsystem when that is materially true; the narrower case-by-case items go to `testing_gaps`. A claim that depends on unproven deployment topology (multiple instances, restarts, infrastructure behavior) is a residual risk unless the changed code or repo evidence establishes that condition.
3. **Pre-existing exception.** When the new change *depends* on a pre-existing gap for correctness, flip it to `pre_existing: false` and `bucket: primary`. Nearby cleanup stays pre-existing.
4. **Detail hydration.** Every retained finding needs a non-empty `why_it_matters` and at least one evidence item. For `hydration: "return-only"` or `hydration: "thin"`, re-read the cited line, fill both fields with only what you can directly verify, and set `hydration: "artifact"`; the script never re-checks these fields on pass 2, so a thin finding you leave alone reaches the report thin. Never invent impact from the title. If the required fields still cannot be established, set `bucket: dismissed` with reason `malformed: <what is missing>`.
5. **Lead judgment.** Reviewers saw a slice of the code and a three-line intent. You hold the whole diff, the conversation, and the repo. Apply three filters, setting `bucket: dismissed`, `reason`, and `stage: "lead"` on each casualty:
   - **Nitpick gravity.** Reviewers fill their review. When one reviewer's entire return is P3 style or taste items, the code it looked at is fine. Drop those items and say so for that reviewer in Coverage.
   - **Consistent with the codebase.** A pattern flagged as wrong that matches how the rest of the repo does the same thing is not this diff's defect. One grep settles it. A standards file overrides precedent: a documented rule wins.
   - **Code the diff did not touch.** A suggestion to change a line outside the hunks, where the diff makes that line neither newly reachable nor newly wrong, is out of scope. A real bug there gets `pre_existing: true`; anything else is dismissed.
6. **Reconcile the harvested PR feedback.** Every item harvested in Stage 2b lands in exactly one bucket, and Coverage reports the counts: **became a finding** (possibly merged with a persona finding, which corroborates it); **already addressed** in the current code, with the file and line that shows it; **not a finding**, with the reason. A bot finding you cannot verify either way stays a finding at confidence 50 with `bucket: primary`. Never leave a harvested item unaccounted for.
7. **Requirement tagging.** For each `R<n>` in the requirements block, find the finding that shows it unmet (set its `requirement`) or the `file:line` that shows it met. Record your own reading now; the validator gives an independent one in Stage 5b.
8. **Partition the actionable queue.** Actionable is `gated_auto` or `manual` with owner `downstream-resolver`. Normalize any concrete P0 or P1 to `downstream-resolver` unless the report entry names the specific product decision, missing authority, external dependency, or release action that blocks implementation. A broad redesign, several related edits, or sensitive code is not such a blocker. Reviewer caution and `requires_verification: true` do not remove a fixable defect from the queue.
9. **Group by theme when findings span distinct concerns.** A group has a short title, the `#`s it covers, one line of context, and the preferred resolution with ordering ("decide X once, resolves #1 and #7, do #1 first"). Groups never merge findings or change severity, route, or numbering. A finding appears in at most one group; unrelated findings stay ungrouped. Skip grouping entirely when every finding is about the same thing. Mark each group as mechanical work or a decision gate. Keep the groups outside `reconciled.json`; they are rendered, never merged.

### Pass 2: restore the mechanics

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
bash "$SKILL_DIR/scripts/review.sh" merge "$RUN_DIR" --reconciled "$RUN_DIR/reconciled.json"
```

The script re-applies the gates idempotently, honors every `bucket` you set, moves `dismissed` items into the `dismissed` array with your reason, appends `testing_gaps` and `residual_risks` items as one-line entries, renumbers `findings` then `pre_existing` from 1, and rewrites `merged.json` as pass 2. Numbers can shift between pass 1 and pass 2; the report is rendered only after the final pass, so nothing the user sees ever renumbers.

### Manual merge (no python3)

When `review.sh merge` exits 4, or fails for a reason you cannot fix in one attempt, do the eight mechanical gates yourself in the order listed under Pass 1, then the judgment steps, then sort and number by hand. Coverage says `merge: manual`. This is the slow path, taken only when the script is unavailable.

## Stage 5b: Validation pass

Independent verification of the findings most likely to waste the user's time if wrong.

1. From the pass 2 `merged.json`, select every P0 and P1 in `findings`, plus every actionable finding. Eight is the normal cap; when more than eight P0 or P1 survive, expand the batch rather than dropping any or splitting into a second batch.
2. Read `references/validator.md`, fill its slots (the same scope rules, requirements block, and review context the reviewers had, plus one block per finding in the shape that file shows), and dispatch one generic validator subagent. A blocking spawn returns its result directly. An asynchronous spawn returns an ID: retain it and collect it through the host's supported completion primitive before continuing. Do not busy-poll or sleep. Peer findings are validated like every other finding; corroboration never skips the validator.
3. Apply the return on the numbering the validator saw: copy the pass 2 `merged.json` over `reconciled.json`, and in that copy set `bucket: dismissed`, `reason: <the validator's reason>`, `stage: "validator"` on each `#` returned as `validated: false`. Never apply verdicts to the older reconciled file; its numbers predate the pass 2 renumbering. Merge the validator's `requirements` verdicts with yours from step 7: agreement stands; on disagreement, `unmet` beats `met`, and `cannot tell` is reported as such with both readings. Run pass 2 once more so numbering and counts are final. Prune triage groups after drops.
4. On malformed validator output or infrastructure failure, drop the affected P2 and P3 findings but **keep** the affected P0 and P1 marked validation-degraded, and say so in Coverage.

## Stage 6 report

Read `references/report-example.md` first. Assemble the markdown report. Sections, in order (omit any that would be empty):

1. **Header.** Scope (what is being reviewed, the PR or branch), intent, the ticket and whether it was explicit or inferred, the reviewer team by spec and job with a one-line reason per conditional reviewer, the peer by CLI and reported model when one ran, and one stamp line: `Reviewed <head_sha> against <base_sha>, patch-id <id>`. A rebase or base retarget rewrites SHAs and can invalidate a verdict without any check going red; the same patch-id means the same diff. Compute it once:

   ```bash
   BASE_SHA=$(git rev-parse "$BASE") && HEAD_SHA=$(git rev-parse HEAD) && PATCH_ID=$(git diff "$BASE" | git patch-id --stable | cut -d' ' -f1)
   ```

   `git diff "$BASE"` with no second ref is the same diff Stage 1 reviewed, working tree included, so uncommitted changes are part of the stamp. Under `pr-remote` or `branch-remote`, diff `PR_BASE_REF` against `PR_HEAD_REF` (or the branch ref) instead. When those fetches failed, write `patch-id unavailable (fetch failed)` and set `patch_id` to null.
2. **Triage Groups.** When groups exist: a compact table of `| Group | Findings | Context | Preferred resolution | Mechanical or decision |`. Every referenced `#` must appear in the findings below.
3. **Findings**, grouped by severity: `### P0: Critical`, `### P1: High`, `### P2: Moderate`, `### P3: Low`. Per finding make four things unambiguous: **what and where** (one scannable line: the symptom plus `file:line`, not the mechanism); **why it matters** (what breaks and who is hit, never a restatement of the code); **what response it needs** (a bug states its fix, a design call presents options and the tradeoff without forcing one, a coverage gap names the test and a precedent to mirror); and **how sure** (confidence, and whether more than one reviewer, the peer, or the PR's own bots corroborated it, which is the strongest signal there is). For findings that came from existing PR feedback, say so and name the source, so the user knows the point is already visible on the PR. A finding tagged with a requirement names it.
4. **Requirements.** Omit only when no ticket resolved. A header line names the ticket and whether it was explicit or inferred, then one line per `R<n>`: `met` with the `file:line` that satisfies it, `unmet` with the finding `#` or what is missing, `deferred` when the diff or PR body states the deferral, or `cannot tell` with the reason.
5. **Existing PR feedback.** The reconciliation from judgment step 6: what was harvested, what became findings, what was already addressed, what was dismissed and why. Name each bot by its actual login (`coderabbitai`, `github-actions`, and so on). Omit only when no PR was reviewed.
6. **Pre-existing.** Separate, does not count toward the verdict.
7. **Dismissed.** One line per entry in the `dismissed` array whose confidence reached 50: the title, the reviewer, and the reason with its stage (the confidence gate, the validator's reason, or the lead-judgment filter that fired). Confidence 0 and 25 suppressions stay as counts in Coverage; they are noise, not judgment calls. This section exists so the user can override a rejection they disagree with. Omit when empty.
8. **Coverage.** Reviewers that ran and any that failed; the peer's outcome (`ok`, `not started (<reason>)`, `no usable output (<reason>)`, or nothing when none was requested); the lite roster when it was used; `merge: script` or `merge: manual`; suppressed counts by anchor; fast-pass clamps and withdrawn preliminary items; quote-the-line demotions; soft-bucket demotions; validator results, drops, and any degraded blockers; dismissed counts by reason; reviewers whose whole return fell to nitpick gravity; harvested-feedback counts by outcome; the ticket line (`ticket: ENG-42 read through tickets.sh`, `ticket: none found`, `ticket: ENG-42 unreadable (exit 3)`); untracked files excluded from scope; residual risks; testing gaps; intent uncertainty; the run artifact path.
9. **Verdict.** `Ready to merge` / `Ready with fixes` / `Not ready`, plus the fix order when relevant. An `unmet` requirement on an explicit ticket forces `Not ready` and the verdict names the requirement; on an inferred ticket it is a note in the verdict, never a block on its own. When every surviving finding is P3 and every requirement is met or deferred, say the code is ready and the list is optional polish. Do not pad a clean review.
10. **Actionable recap**, last. The prioritized list of what to do, each item carrying severity, `file:line`, the terse what, and its response type.

Hard constraints:

- **ASCII-safe.** No box drawing, no per-item horizontal rules, no Unicode arrows, no middot. Use `->`. A single `---` before the verdict is fine. Em dashes are fine in the *report* (it is yours, not the user's voice); they are banned in anything posted to GitHub.
- **Stable `#`s reused across every section.** A multi-file finding is one row with one `#`.
- **Escape literal `|` as `\|`** inside table cells.
- **Do not paste file contents or reprint the diff.** Cite `file:line` and spend words only on what the diff cannot show.
- **The closing stands alone.** A reader who sees only the last screen gets the verdict and the prioritized list.
- Cover every finding. Brevity governs expression, never coverage. A nit is one line; a P1 design call earns room.

Write the rendered report to `{run_dir}/report.md`, the final `merged.json` findings to `{run_dir}/findings.json`, and `{run_dir}/metadata.json`:

```json
{
  "run_id": "<run-id>",
  "branch": "<git branch --show-current at dispatch time>",
  "head_sha": "<git rev-parse HEAD at dispatch time>",
  "base_sha": "<the resolved base commit>",
  "patch_id": "<git patch-id --stable of the base-to-head diff, or null>",
  "pr": "<url or null>",
  "scope_mode": "local-aligned | pr-remote | branch-remote | standalone",
  "ticket": {"id": "ENG-42", "url": "<url or null>", "resolution": "explicit | inferred", "source": "token | pr-body | branch | commit"},
  "requirements": [{"id": "R1", "text": "...", "status": "met | unmet | deferred | cannot_tell", "evidence": "<file:line or reason>", "finding": null}],
  "peer": {"cli": "codex", "model": "<reported model or null>", "independence_verified": true, "outcome": "ok"},
  "verdict": "<Ready to merge | Ready with fixes | Not ready>",
  "completed_at": "<ISO 8601 UTC>"
}
```

`ticket` is `null` and `requirements` is `[]` when no ticket resolved. `peer` is `null` when none was requested. After metadata is written, `rm -f "$RUN_DIR"/peer-*` removes the peer job files; the peer artifact `havoc-demon-hunter-peer.json` stays.

## The `mode:agent` contract

Under `mode:agent` the deliverable is one JSON object, emitted bare with no code fence (a leading fence breaks naive parsers) and nothing before or after it, and written to `{run_dir}/review.json` as well. Shape:

```json
{
  "status": "ok | degraded | failed",
  "verdict": "Ready to merge | Ready with fixes | Not ready",
  "run_id": "<run-id>",
  "artifact_path": "<run_dir>/report.md",
  "scope": {"base_sha": "...", "head_sha": "...", "branch": "...", "pr": "<url or null>", "scope_mode": "...", "patch_id": "<id or null>"},
  "intent": "<the Stage 2 summary>",
  "ticket": {"id": "ENG-42", "url": "...", "resolution": "explicit", "source": "pr-body"},
  "requirements": [{"id": "R1", "text": "...", "status": "met", "evidence": "src/tax.ts:88", "finding": null}],
  "reviewers": [{"name": "protection-warrior", "job": "correctness", "reason": "always", "status": "ok"},
                {"name": "havoc-demon-hunter-peer", "job": "adversarial", "reason": "peer:codex", "status": "ok", "model": "gpt-5", "independence_verified": true}],
  "findings": [{"#": 1, "title": "...", "severity": "P1", "file": "...", "line": 31, "confidence": 100, "autofix_class": "gated_auto", "owner": "downstream-resolver", "requires_verification": true, "pre_existing": false, "suggested_fix": "...", "first_evidence": "...", "why_it_matters": "...", "evidence": ["..."], "reviewers": ["protection-warrior", "marksmanship-hunter"], "corroborated": true, "requirement": "R1", "validated": true}],
  "actionable_findings": [1, 3],
  "triage_groups": [{"title": "...", "findings": [1, 3], "context": "...", "resolution": "...", "kind": "mechanical | decision"}],
  "pre_existing_findings": [],
  "dismissed": [{"title": "...", "reviewers": ["balance-druid"], "reason": "...", "stage": "merge | lead | validator"}],
  "residual_risks": ["..."],
  "testing_gaps": ["..."],
  "coverage": {"lite_roster": false, "merge": "script | manual", "suppressed": {"0": 0, "25": 2}, "clamped_fast_pass": 0, "demoted_missing_evidence": 1, "promoted": 1,
               "harvested": {"total": 3, "findings": 1, "addressed": 1, "dismissed": 1},
               "validator": {"batch": 3, "dropped": 1, "degraded": []},
               "peer": "ok | not started (<reason>) | no usable output (<reason>) | null",
               "untracked_excluded": [], "notes": []}
}
```

`status` is `ok` when every dispatched reviewer returned, the validator answered, and the merge ran through the script; `degraded` otherwise, with the reason in `coverage.notes`. `actionable_findings` is the apply queue: `#`s whose `autofix_class` is `gated_auto` or `manual` with owner `downstream-resolver`. `triage_groups` is a lens over all findings, never an apply queue; a caller batching by theme intersects each group's `findings` with `actionable_findings` first.

A run that stops before findings exist returns `{"status": "failed", "stage": "arguments | scope | dispatch | merge | validate", "reason": "<one sentence>", "run_id": "<id or null>"}` and nothing else.

## Quality gates

Before delivering, verify:

1. **Every finding is actionable.** "Consider", "might want to", "could be improved" with no concrete action means rewrite it or drop it.
2. **No skim-level false positives.** Confirm the surrounding code was read: the bug is not handled elsewhere in the same function, the "unused" import is not used in a type position, the "missing" null check is not guaranteed by the caller.
3. **Severity is calibrated.** A style nit is never P0. An auth bypass is never P3.
4. **Line numbers are accurate.** A finding pointing at the wrong line is worse than no finding.
5. **No linter duplication.** Nothing the project's formatter or linter already catches.
6. **Nothing harvested vanished.** Every Stage 2b item is in one of the three buckets.
7. **Nothing dismissed vanished.** Every gate-6 drop, validator drop, and lead-judgment dismissal has a line in Dismissed.
8. **Every requirement has a status.** No `R<n>` from the block is missing from the Requirements section, and an `unmet` one on an explicit ticket is reflected in the verdict.

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

**Verify.** Run the project's validation command: the `Validation:` line in the `## Agent skills` block of `CLAUDE.md` or `AGENTS.md` when one exists, else the test suite, typecheck, and lint the project's conventions name. Target the touched area by default and broaden when fixes span files. If a fix breaks something, revert that fix and report it as a finding instead. An unverified fix is not finished. Never leave the tree red.

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
cat > "$RUN_DIR/pr-review-payload.json" <<'JSON'
{
  "commit_id": "<PR head sha>",
  "event": "COMMENT",
  "comments": [
    {"path": "src/foo.ts", "line": 42, "side": "RIGHT", "body": "<comment text>"},
    {"path": "src/bar.ts", "start_line": 10, "line": 14, "side": "RIGHT", "body": "<comment text>"}
  ]
}
JSON
gh api --method POST repos/OWNER/REPO/pulls/PR_NUMBER/reviews --input "$RUN_DIR/pr-review-payload.json"
```

- `event` is `COMMENT`, never `APPROVE` or `REQUEST_CHANGES`. Approving or blocking is the user's call, not this skill's.
- Use a **top-level review body only** when there is something worth saying about the change as a whole, and keep it to a couple of sentences in the user's voice. An empty or missing body is fine and usually better.
- Write bodies through a heredoc or a JSON file, never `echo` with escape sequences: `\n` posted literally shows up as one run-on line with visible backslashes.
- Multi-line findings use `start_line` plus `line`. Single-line findings use `line` alone.
- If the whole-review POST fails, fall back to posting the comments individually against `repos/OWNER/REPO/pulls/PR_NUMBER/comments` with `commit_id`, `path`, `line`, and `side`. Never fall back to `gh pr review` in a way that opens an unsubmitted draft.

**Verify what landed.** Read back the posted comments and confirm each body renders with real line breaks, not literal `\n`, and that no em dash survived. Fix any that did with a `PATCH` to `repos/OWNER/REPO/pulls/comments/COMMENT_ID`.

**Report back:** the review URL, a list of `# | file:line | first line of the comment`, and any finding that could not be commented on (line outside the diff) so the user knows it is only in the report.
