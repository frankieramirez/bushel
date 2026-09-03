---
name: scan
description: Deep multi-reviewer code review for bugs, regressions, tests, and standards. Dispatches specialist reviewer subagents in parallel, merges their findings into one report, then asks whether to report only, fix and push, or leave inline PR comments. Use before opening a PR, when asked for a thorough review, to scan a branch, or to review a PR.
argument-hint: "[blank for current branch | PR number | PR URL | branch] [base:<ref>] [depth:full] [report|fix|comment] [mode:agent]"
---

# Scan

Reviews code changes with dynamically selected reviewer personas. Dispatches bounded specialist subagents that return structured JSON, merges and deduplicates their findings into a single report, then asks what to do with them.

## When to use

- Before opening a PR
- After finishing a task during iterative implementation
- When a thorough review of a PR or branch is wanted
- Inside a larger workflow, with `mode:agent` when the caller needs JSON

For a quick sanity pass, this is the wrong tool. Say so and offer the harness's built-in review instead.

## Execution spine

Follow these boundaries in order. References supply detail but never change the order.

1. Resolve the reviewed diff and the intent behind it (Stage 1, Stage 2).
2. **When the target is a PR, harvest existing PR feedback unconditionally** (Stage 2b). This is not a conditional lens; it always runs for a PR.
3. Select the risk-driven reviewer roster and discover applicable standards paths (Stage 3).
4. Read `references/subagent-template.md`, `references/diff-scope.md`, `references/findings-schema.json`, and the selected persona files, then dispatch the roster as one foreground concurrent batch and collect every reviewer before synthesis (Stage 4).
5. Read `references/finish-review.md` and follow it to merge, validate, and render the report (Stage 5). Never synthesize directly from raw reviewer artifacts.
6. Ask what to do with the findings, then do it (Stage 6). This is the one blocking question this skill asks.

## Operating principles

- **Review first, act second.** Nothing is edited, committed, or posted until Stage 6, and then only along the branch the user picks.
- **One blocking question, at the end.** Do not stop to ask about scope, intent, or plan. Infer those from tokens, git state, PR metadata, and conversation, and note uncertainty in Coverage. The Stage 6 choice is the only prompt.
- **Never switch branches.** Do not run `gh pr checkout`, `git checkout`, or `git switch`. Passing a PR number, URL, or branch name selects **review scope**, not permission to mutate the tree. To review uncommitted work on a feature branch, be on that branch and pass `base:` or nothing.
- **Report outcomes, not machinery.** Surface what is being reviewed, which lenses ran and the one-line reason for each conditional one, and the findings. Keep internals quiet: model tiers, scope-mode codenames, staging the diff to disk, persona file loading, dispatch bookkeeping.
- **Untracked files are out of scope** unless staged. List them in Coverage and continue on tracked changes.

## Arguments

Parse for these tokens and strip each before interpreting the remainder as a PR number, URL, or branch name.

| Token | Effect |
|-------|--------|
| `base:<sha-or-ref>` | Diff base on the current checkout; skips base auto-detection. Cannot combine with a PR number or branch target. |
| `depth:full` | Force the full roster; skip the small-diff lite path (Stage 3c). |
| `report` | Skip the Stage 6 question: report only. |
| `fix` | Skip the Stage 6 question: fix everything actionable, commit, and push. |
| `comment` | Skip the Stage 6 question: post inline PR comments for each finding. Requires a PR. |
| `mode:agent` | Return one raw JSON object instead of markdown, and skip Stage 6 entirely. The caller acts. |

Stop without dispatching when: `base:` appears with a PR or branch target; two different action tokens appear (`fix` and `comment`); `mode:agent` appears with `fix` or `comment`; or `comment` is passed with no PR. Emit a one-line reason (JSON `{"status":"failed","reason":"..."}` under `mode:agent`).

## Severity scale

| Level | Meaning | Action |
|-------|---------|--------|
| **P0** | Critical breakage, exploitable vulnerability, data loss or corruption | Must fix before merge |
| **P1** | High-impact defect likely hit in normal usage, broken contract | Should fix |
| **P2** | Moderate issue with real downside (edge case, perf regression, maintainability trap) | Fix if straightforward |
| **P3** | Low impact, narrow scope, minor improvement | Discretionary |

Severity answers urgency. `autofix_class` and `owner` describe the shape of the follow-up:

| `autofix_class` | Default owner | Meaning |
|-----------------|---------------|---------|
| `gated_auto` | `downstream-resolver` | Concrete `suggested_fix` proposed; apply after judgment |
| `manual` | `downstream-resolver` or `human` | Actionable but needs design input |
| `advisory` | `human` | Report only: residual risk, rollout note, observation |

Synthesis owns the final route. On disagreement between reviewers, take the more conservative one. The schema allows only these three classes; if a reviewer invents an "auto-apply" class anyway, treat it as `gated_auto`.

---

## Stage 1: Determine scope

Compute the diff range, file list, and diff. Combine into as few commands as possible.

**`base:` given (fast path).** The caller knows the base. Skip all detection:

```bash
BASE_ARG="<base_arg>"; BASE=$(git merge-base HEAD "$BASE_ARG" 2>/dev/null) || BASE="$BASE_ARG"
echo "BASE:$BASE" && echo "FILES:" && git diff --name-only $BASE && echo "DIFF:" && git diff -U10 $BASE && echo "UNTRACKED:" && git ls-files --others --exclude-standard
```

**PR number or URL given.** Do **not** check out the PR branch. First probe state:

```bash
gh pr view <number-or-url> --json state,title,body,files
```

Stop when `state` is `CLOSED` or `MERGED` (`PR is closed/merged; not reviewing.`). Stop for an obviously automated PR that does not warrant review (lock-file or manifest-only bumps, release commits, chore version increments). When in doubt, review it: a skipped review that should have run costs more than an unnecessary one.

Then fetch metadata without checkout:

```bash
gh pr view <number-or-url> --json title,body,baseRefName,headRefName,headRefOid,isCrossRepository,url,files --jq '{title, body, baseRefName, headRefName, headRefOid, isCrossRepository, url, files: [.files[].path]}'
```

Set `BASE:` to the marker `pr:<number>`. Classify the scope mode; a matching branch name alone is not enough, since a fork PR or stale local branch can share a name while pointing at unrelated code:

**`local-aligned`** requires all three: `git rev-parse --abbrev-ref HEAD` equals `headRefName`; `isCrossRepository` is false; and `git merge-base --is-ancestor <headRefOid> HEAD` exits 0. Then local Read, Grep, and blame are valid for changed paths. Resolve `<base-ref>` from `baseRefName` (fetch if needed), compute `BASE=$(git merge-base HEAD <base-ref>)`, and take `FILES:` / `DIFF:` from the **local** tree (`git diff --name-only $BASE`, `git diff -U10 $BASE`). Do not append `gh pr diff` hunks; when unpushed fixes exist the local tree is canonical. Note `scope: local-aligned` in Coverage.

**`pr-remote`** otherwise. `FILES:` from the PR `files` array, `DIFF:` from `gh pr diff <number-or-url> --color=never`. If that fails, stop with an actionable error; never fall back to checkout. Then best-effort fetch both ends without checkout:

```bash
git fetch --no-tags origin <headRefName>:refs/review/pr-<number>-head
git fetch --no-tags origin <baseRefName>
```

On success set `PR_HEAD_REF=refs/review/pr-<number>-head` and `PR_BASE_REF=$(git rev-parse FETCH_HEAD)` and pass both to reviewers. On failure, omit and note it in Coverage; reviewers then rely on diff hunks only and must **not** assume `main` as a base. In `pr-remote`, reviewers and validators must not Read or Grep workspace paths for changed files: use `git show <PR_HEAD_REF>:<path>` or the hunks.

**Branch name given.** Do not check it out. If it equals the current branch, use the standalone path. Otherwise: if a PR exists for it (`gh pr view <branch> --json baseRefName,url,headRefName`), prefer the PR path. Else resolve `origin/<branch>` (fetching if needed), compute `BASE=$(git merge-base <base-ref> <branch-ref>)`, and diff `$BASE <branch-ref>`. If the ref cannot be resolved locally, stop: "Cannot diff branch `<branch>` without checkout. Check out that branch, pass its PR URL, or review the current branch with `base:`." This is **`branch-remote`** scope, with the same no-workspace-inspection rule as `pr-remote`.

**No argument (standalone).** Resolve the base from `gh pr view --json baseRefName,url` for the current branch, falling back to the repo's default branch. If no base resolves, **stop**. Do not fall back to `git diff HEAD`: that shows only uncommitted changes and silently misses every committed change on the branch.

```bash
echo "BASE:$BASE" && echo "FILES:" && git diff --name-only $BASE && echo "DIFF:" && git diff -U10 $BASE && echo "UNTRACKED:" && git ls-files --others --exclude-standard
```

`git diff $BASE` without `..HEAD` diffs the merge base against the working tree, so committed, staged, and unstaged changes all appear.

Count executable changed lines (excluding pure docs, lock files, generated output, and snapshots) for the Stage 3c gate.

## Stage 2: Intent discovery

Understand what the change is trying to do. PR mode: title, body, linked issues, plus commit subjects if the body is thin. Branch mode: `git log --oneline ${BASE}..<branch-ref>`. Standalone: `git log --oneline ${BASE}..HEAD` plus the branch name.

Write a 2 to 3 line intent summary and pass it to every reviewer:

```
Intent: Replace the multi-tier tax rate lookup with a flat-rate computation.
Must not regress tax-exempt edge cases.
```

Intent shapes *how hard each reviewer looks*, never which reviewers are selected. When intent is ambiguous, write the best-effort summary and note the uncertainty in Coverage. Never block on a clarifying question.

## Stage 2b: Harvest existing PR feedback (always, when a PR exists)

**This step is unconditional whenever Stage 1 resolved a PR.** Automated reviewers post real, concrete findings that a code-only review reproduces late or not at all, and they post them on three different surfaces. Fetch all three:

```bash
gh api graphql -f owner=OWNER -f repo=REPO -F pr=NUMBER -f query='
query($owner:String!,$repo:String!,$pr:Int!){
  repository(owner:$owner,name:$repo){ pullRequest(number:$pr){
    comments(first:100){ nodes { author{login} body createdAt url } }
    reviews(first:100){ nodes { author{login} body state submittedAt } }
    reviewThreads(first:100){ nodes { id isResolved isOutdated path line
      comments(first:50){ nodes { author{login} body url } } } }
  } } }'
```

| Surface | GraphQL field | Who lands here |
|---------|---------------|----------------|
| Top-level PR comments | `comments` | Architecture and static-analysis bots, CodeRabbit walkthroughs and summaries, Copilot, Gemini Code Assist, Sonar, Codecov |
| Review bodies | `reviews[].body` | CodeRabbit "requested changes", human review summaries |
| Inline review threads | `reviewThreads` | Human line comments, CodeRabbit and Copilot inline nits |

**The failure mode this step exists to prevent:** treating a bot's top-level comment as boilerplate and dropping it. Architecture and static-analysis bots typically post a "check failed" comment as a **top-level PR comment**, not a review thread, with their findings in a `Location | Issue` table plus Why and How-to-fix sections. A review gated on review threads never sees it. So:

- Read every top-level comment body **in full**. Never classify by author identity or by the first line.
- A table or list of locations inside one bot comment is **one item per row**, not one item.
- Drop only genuine boilerplate with no ask: approvals, status badges, coverage deltas with no threshold breach, walkthrough summaries that merely restate the diff.
- A bot comment that says a check **failed** is never boilerplate.

Pass the harvested feedback to the `existing-feedback` reviewer (Stage 3, always selected when a PR exists) and keep a copy for synthesis. Every harvested item must reach one of three outcomes in the final report: it becomes a finding, it is recorded as already addressed in the current code, or it is recorded as not-a-finding with a reason. **Silently dropping a harvested item is a defect in this review.** Coverage states the count harvested and the count in each outcome.

## Stage 3: Select reviewers

Read the diff and file list. Selection is judgment about what the diff actually contains, never keyword matching. Persona prompts live in `references/personas/`.

**Always on:**

| Persona | Selected when |
|---------|---------------|
| `correctness` | Every review |
| `existing-feedback` | A PR was resolved in Stage 1. Not conditional on comment count; it verifies its own input and returns empty if there is genuinely nothing. |

**Conditional:**

| Persona | Selected when the diff touches |
|---------|-------------------------------|
| `project-standards` | At least one applicable standards file exists (Stage 3b) |
| `testing` | Test files, fixtures, mocks, or harness behavior; or meaningful runtime behavior changed with no corresponding test work. Behavioral triggers: new or changed branches, state mutation, API or control-flow behavior, error handling. Production-file presence alone does not select it. |
| `security` | Auth, permission checks, public endpoints, user input handling, secrets |
| `performance` | Query shape, algorithmic complexity, loop-heavy transforms, batching or fan-out, cache policy with real resource impact. Async code alone does not select it. |
| `api-contract` | An externally consumed boundary changes: routes, request or response shapes, serializers, published event schemas, versioning, or a public package signature with evidenced callers. A new exported symbol inside one module is not enough. |
| `reliability` | Error handling, retries, timeouts, background jobs, async handlers, health checks |
| `data-migration` | A migration or schema artifact is in the diff: `db/migrate/*`, `db/schema.rb`, `structure.sql`, Alembic / Flyway / Liquibase paths, Prisma migrations, or an explicit backfill script. **Not** model-only or query-only changes. |
| `maintainability` | Large or structural work: substantial refactor, new abstractions, file moves, coupling or type-boundary changes, or 200+ executable changed lines |
| `adversarial` | 50+ executable changed lines; or auth, payments, persistence writes, event publication, retry or concurrency semantics, external APIs; **or a silent-pass verification mechanism of any size** |
| `frontend-races` | Async UI flows, DOM event wiring, timers, animations, effect lifecycles, or state transitions with race potential |

**Silent-pass verification mechanisms.** When the change *is* a verification mechanism (CI or CD gating logic, merge-blocking checks, build or deploy steps, coverage or lint gates, test infrastructure or mocks that could mask production), its risk is not blast radius, it is fidelity: it can go green while the real thing is red. Select `adversarial` regardless of size. The question is "if this is wrong, does it fail loudly or pass silently?" This fires on the *mechanism*, not on ordinary per-feature assertions.

**Instruction-prose files** (Markdown skills, prompts, JSON config) are product code, but runtime-focused lenses add little. For a diff that only changes prose, skip `adversarial` unless the prose governs auth, payments, data mutation, or is itself a verification mechanism. Count only executable lines toward thresholds.

### Stage 3b: Discover project standards paths

Glob `**/CLAUDE.md` and `**/AGENTS.md`, then filter to those whose directory is an ancestor of at least one changed file (a root file governs the whole checkout; `packages/ui/CLAUDE.md` governs everything under it).

- One or more paths: select `project-standards` and pass the path list in a `<standards-paths>` block. The persona reads the files itself.
- Empty successful search: do not select it; record `project standards: not run (no applicable standards files)` in Coverage.
- Search failed or scope is uncertain: fail closed: select it and state the uncertainty.

### Stage 3c: Small-diff lite path

`depth:full` hard-disables this gate.

Collapse to a lite roster only when **all** hold: fewer than 40 executable changed lines; no content-based risk read from the diff (auth, payments, data mutation, external API, secrets, deserialization, crypto, concurrency, filesystem or process execution); Stage 3b completed; and no conditional persona other than `project-standards` was selected. Any uncertainty resolves to the full roster: a 12-line auth change still needs it.

Lite roster: `correctness`, `existing-feedback` when a PR exists, and `project-standards` when applicable. Announce the actual roster and note it in Coverage.

### Stage 3d: Create the run directory

```bash
SCRATCH_ROOT="/tmp/scan-$(id -u)";
if [ -L "$SCRATCH_ROOT" ]; then echo "unsafe scratch root symlink: $SCRATCH_ROOT" >&2; exit 1; fi;
install -d -m 700 "$SCRATCH_ROOT" || exit 1;
if [ -L "$SCRATCH_ROOT" ] || [ ! -O "$SCRATCH_ROOT" ]; then echo "scratch root not owned by current user" >&2; exit 1; fi;
chmod 700 "$SCRATCH_ROOT" || exit 1;
RUN_ID=$(date +%Y%m%d-%H%M%S)-$(head -c4 /dev/urandom | od -An -tx1 | tr -d ' ');
RUN_DIR="$SCRATCH_ROOT/$RUN_ID";
(umask 077; mkdir -p "$RUN_DIR") || exit 1;
echo "$RUN_DIR"
```

**Announce the team** before spawning: name the always-on reviewers plainly, and give each conditional one a one-line reason it was added (the real concern, not the keyword that matched). This is progress reporting, not a confirmation prompt.

## Stage 4: Dispatch and collect

### Inline fast pass

Immediately before the first dispatch, scan the diff you already hold for high-signal obvious problems: injection or data-safety, broken control flow, a missing `await`, a swapped argument or off-by-one, an enum or status added without updating its sibling switch, a null deref the diff makes reachable. No deep analysis, no reading beyond the diff except a quick grep for enum completeness. Quote the verbatim motivating line, same bar as a persona finding.

Show it only when it finds a P0 or P1 candidate, under a clearly preliminary header, with one line saying the items are unverified and will be deduplicated into the final report. Otherwise emit one progress line and move on. Do not assign stable `#` numbers here.

The fast pass enters synthesis as pseudo-reviewer `fast-pass` with two hard caps, because it shares your model and blind spots: **cap every `fast-pass` finding at confidence 50**, and **`fast-pass` never counts toward cross-reviewer promotion**. Never seed its candidates into persona or validator prompts; that manufactures the false agreement the cap exists to prevent. Under `mode:agent`, run the scan internally but emit no preliminary block.

Reconcile it in the final report: a preliminary item that did not survive gets a one-line "Preliminary fast-pass items withdrawn: n (reason)" note, so a user who saw a scary preliminary finding learns it was cleared.

### Model tiering

`correctness`, `security`, and `adversarial` inherit the session model with no override; they do the highest-stakes analysis. Every other persona uses the platform's mid-tier model (Sonnet class in Claude Code). Record each reviewer's tier when you select it and apply the override on **every** dispatch call. A missed override silently runs a cheap lens at the expensive tier. Do not print tiers to the user.

### Staging and spawning

Write `full.diff` and `files.txt` into `$RUN_DIR` and pass those **paths** instead of inline content when the diff is large; inline a small one. Pass `{run_id}` and `{run_dir}` to every persona so it can write `{run_dir}/{reviewer_name}.json`.

Before assembling any prompt, read these from this skill's directory, in one parallel wave along with the selected persona files: `references/subagent-template.md`, `references/diff-scope.md`, `references/findings-schema.json`.

Spawn each selected reviewer as a **generic subagent** seeded with its persona file. Do not use typed agent names. Omit the `mode` parameter so the user's permission settings apply. Dispatch the whole roster as one **foreground concurrent batch** (multiple spawns in a single message, background execution off) and let that single wait return every reviewer's JSON. Size the batch to the host's active-agent cap and refill as slots free; never hard-code a number. Treat concurrency-limit spawn errors as backpressure to retry, not reviewer failure. Where the harness does not run same-message calls concurrently, this degrades to serial, which is the correct floor.

No polling, status calls, sleeps, shell no-ops, scheduled wakeups, or "still waiting" turns. Collect **every** spawned reviewer before synthesis; synthesis on a partial roster is a defect.

Each persona subagent receives: its persona content, the shared diff-scope rules, the JSON schema, PR metadata in a `<pr-context>` block when reviewing a PR, the intent summary, the file list and diff (or staged paths), the scope mode and remote head ref when set, and its run ID and reviewer name. Plus, for specific personas: `<standards-paths>` for `project-standards`, `<review-base>` for `data-migration`, and the harvested feedback from Stage 2b for `existing-feedback`.

Persona subagents are **read-only** toward the project: non-mutating inspection only, including read-oriented `git` and `gh` (`git diff`, `git show`, `git blame`, `git log`, `gh pr view`). The one permitted write is their own artifact file. They never edit project files, switch branches, commit, push, or post anything.

## Stage 5: Merge, validate, report

Once every reviewer has returned, read `references/finish-review.md` in full and follow it. That covers dedup, the confidence and quote-the-line gates, soft-bucket demotion, the validation pass, and the report skeleton. Do not improvise a shorter synthesis path.

## Stage 6: Choose what happens next

After the report is delivered, ask **one** question, unless `report`, `fix`, `comment`, or `mode:agent` already answered it. Under `mode:agent`, emit the JSON and stop.

Use the platform's blocking question tool (`AskUserQuestion` in Claude Code; call `ToolSearch` with `select:AskUserQuestion` first if the schema is not loaded) with these three options:

| Option | Behavior |
|--------|----------|
| **Report only** | Stop. The report is the deliverable. |
| **Fix, commit, and push** | Address every actionable finding, verify, commit, push. See *Apply mode* in `references/finish-review.md`. |
| **Leave inline PR comments** | Post one inline review comment per finding on the PR, in the user's voice. See *Comment mode* in `references/finish-review.md`, and read `references/voice.md` before writing a single word of comment text. |

Offer **Leave inline PR comments** only when a PR exists. When it does not, offer Report only and Fix, and say why the third is missing.

The report is already delivered at this point, so the question is about action, not about whether the review is done.

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/subagent-template.md` | Stage 4 | Dispatch shape, confidence rubric, false-positive catalog |
| `references/diff-scope.md` | Stage 4 | Scope tiers and evidence-tool rules passed to each subagent |
| `references/findings-schema.json` | Stage 4 | JSON output contract passed to each subagent |
| `references/finish-review.md` | Stage 5 | Merge, validate, render, and the three Stage 6 action modes |
| `references/voice.md` | Stage 6, comment mode | How to write PR comments as the user, not as an agent |
| `references/personas/*.md` | Stage 4 | One file per selected reviewer |
