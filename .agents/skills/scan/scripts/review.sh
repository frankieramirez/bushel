#!/usr/bin/env bash
# review.sh: the deterministic parts of a scan review.
#
#   signals --base REF [--head REF]
#       Classify the diff: executable versus prose lines, excluded files, path signals,
#       risk words on added lines, and whether the small-diff lite path is even eligible.
#       Prints one JSON object. Omit --head to diff the working tree, as Stage 1 does.
#
#   merge RUN_DIR [--reconciled FILE] [--out FILE] [--roster a,b,c]
#       Pass 1: read every reviewer artifact in RUN_DIR (plus RUN_DIR/returns/*.json for a
#       reviewer whose artifact is missing) and apply the mechanical gates in order: fast-pass
#       clamp, suppression at 0 and 25, quote-the-line demotion, exact dedup, cross-reviewer
#       promotion, the confidence gate, partition, sort, stable numbering. Writes merged.json.
#       Pass 2 (--reconciled): take the model's edited copy of merged.json and restore the
#       gates, numbering, and counts. --roster lists the reviewers that were dispatched so a
#       missing artifact is reported.
#
#   peer --cli NAME --run-dir DIR --brief FILE --constraints FILE --host FAMILY
#        [--timeout SECONDS] [--check] [--named-by-user]
#       Run one other CLI as the adversarial reviewer, read-only, and write
#       DIR/havoc-demon-hunter-peer.json. --check only verifies the CLI and the family rule
#       and prints the disclosure line. Exit 0 usable, 2 could not start, 3 no usable output.
#
# Exit 4 from signals or merge means python3 is missing: follow the manual path in
# references/finish-review.md instead. Everything here is python3 standard library inside
# bash; there is no jq, no node, no pip package.

set -euo pipefail

TMP=""
trap '[ -n "${TMP:-}" ] && rm -rf "$TMP"' EXIT

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA="$HERE/../references/findings-schema.json"

usage() {
  sed -n '2,24p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

die() {
  echo "review.sh: $*" >&2
  exit 1
}

need_python() {
  command -v python3 >/dev/null 2>&1 || { echo "review.sh: python3 not found; use the manual merge path" >&2; exit 4; }
}

# ---------------------------------------------------------------- signals

cmd_signals() {
  need_python
  local base="" head=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --base) base="${2:-}"; shift 2 ;;
      --head) head="${2:-}"; shift 2 ;;
      -h|--help) usage; exit 0 ;;
      *) die "signals: unknown argument $1" ;;
    esac
  done
  [ -n "$base" ] || die "signals: --base is required"
  local base_sha head_sha
  base_sha=$(git rev-parse --verify "$base^{commit}" 2>/dev/null) || die "signals: cannot resolve base $base"
  TMP=$(mktemp -d)
  if [ -n "$head" ]; then
    head_sha=$(git rev-parse --verify "$head^{commit}" 2>/dev/null) || die "signals: cannot resolve head $head"
    base_sha=$(git merge-base "$base_sha" "$head_sha") || die "signals: no merge base between $base and $head"
    git diff --numstat "$base_sha" "$head_sha" > "$TMP/numstat"
    git diff -U0 --no-color "$base_sha" "$head_sha" > "$TMP/diff"
  else
    head_sha="worktree"
    git diff --numstat "$base_sha" > "$TMP/numstat"
    git diff -U0 --no-color "$base_sha" > "$TMP/diff"
  fi
  REVIEW_BASE="$base_sha" REVIEW_HEAD="$head_sha" REVIEW_NUMSTAT="$TMP/numstat" REVIEW_DIFF="$TMP/diff" \
    python3 - <<'PY'
import json, os, re, sys
from collections import defaultdict

base, head = os.environ["REVIEW_BASE"], os.environ["REVIEW_HEAD"]

LOCKS = {"package-lock.json", "yarn.lock", "pnpm-lock.yaml", "bun.lockb", "bun.lock", "Cargo.lock",
         "Gemfile.lock", "poetry.lock", "uv.lock", "Pipfile.lock", "go.sum", "composer.lock",
         "Podfile.lock", "flake.lock", "mix.lock", "packages.lock.json"}
DOC_EXT = (".md", ".mdx", ".rst", ".txt", ".adoc")
GENERATED_DIRS = ("generated/", "dist/", "build/", "vendor/", "node_modules/", "__generated__/")
GENERATED_EXT = (".min.js", ".min.css", ".map", ".pb.go", "_pb2.py", "_pb2_grpc.py")
SNAPSHOT_MARKS = ("__snapshots__/", ".snap", ".golden")
AGENT_FILES = {"SKILL.md", "CLAUDE.md", "AGENTS.md", "GEMINI.md", ".mcp.json", "AGENT.md"}
AGENT_DIRS = ("skills/", "agents/", "prompts/", "mcp/", ".claude/", ".claude-plugin/", ".cursor/", ".codex/", ".github/copilot/")
CONFIG_EXT = (".json", ".yaml", ".yml", ".toml")
RISK_WORDS = ["auth", "password", "passwd", "token", "secret", "session", "webhook", "payment", "billing", "crypto",
              "exec", "spawn", "eval", "deserialize", "pickle", "subprocess", "unlink", "chmod"]
# substring match on purpose: isAuthenticated, password_hash, getToken, execSync all count
RISK_RE = re.compile("(" + "|".join(RISK_WORDS) + ")", re.IGNORECASE)


def final_path(p):
    # numstat renders a rename as "dir/{old => new}/file" or "old => new"
    m = re.search(r"\{(.*?) => (.*?)\}", p)
    if m:
        return p[:m.start()] + m.group(2) + p[m.end():]
    if " => " in p:
        return p.split(" => ", 1)[1]
    return p


def segments(p):
    return p.split("/")


def is_agent_surface(p):
    name = os.path.basename(p)
    if name in AGENT_FILES or name.endswith(".prompt.md"):
        return True
    low = p.lower()
    return any(low.startswith(d) or ("/" + d) in low for d in AGENT_DIRS)


def path_signals(p):
    s = set()
    low = p.lower()
    name = os.path.basename(low)
    segs = set(segments(low))
    if (any(x in low for x in ("db/migrate/", "/migrations/", "prisma/migrations/", "alembic/versions/", "drizzle/"))
            or low.startswith("migrations/") or name in ("schema.rb", "structure.sql")
            or (low.endswith(".sql") and low.startswith("db/"))):
        s.add("migrations")
    if low.endswith((".tsx", ".jsx", ".vue", ".svelte", ".css", ".scss", ".less")) or \
       (segs & {"components", "hooks", "pages", "app", "views"} and low.endswith((".ts", ".js"))):
        s.add("frontend")
    if (segs & {"routes", "api", "controllers", "serializers", "graphql", "handlers", "endpoints"}
            or name.startswith("openapi") or low.endswith(".proto") or name == "schema.graphql"):
        s.add("api")
    if (segs & {"test", "tests", "spec", "specs", "__tests__", "fixtures", "mocks", "__mocks__"}
            or re.search(r"\.(test|spec)\.[a-z]+$", low) or low.endswith("_test.go") or name == "conftest.py"):
        s.add("tests")
    if is_agent_surface(p):
        s.add("agent_surface")
    if (low.startswith(".github/workflows/") or name in ("jenkinsfile", ".gitlab-ci.yml", "makefile", "justfile", "dockerfile")
            or low.startswith("ci/") or name.startswith("dockerfile.")
            or (segs & {"scripts", "bin", "ci", ".github", "hooks", ".husky"}
                and re.search(r"(validate|verify|check|lint|test|ci|precommit|pre-commit|gate)", name))):
        s.add("verification")
    return sorted(s)


# ---- parse the -U0 diff for added lines, with line numbers
added = defaultdict(list)   # path -> [(lineno, text)]
cur = None
new_line = 0
with open(os.environ["REVIEW_DIFF"], errors="replace") as fh:
    for raw in fh:
        line = raw.rstrip("\n")
        if line.startswith("+++ "):
            cur = line[4:].strip().strip('"')
            cur = cur[2:] if cur.startswith("b/") else cur
            if cur == "/dev/null":
                cur = None
            continue
        if line.startswith("@@"):
            m = re.match(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@", line)
            new_line = int(m.group(1)) if m else 0
            continue
        if cur is None or line.startswith("---"):
            continue
        if line.startswith("+"):
            added[cur].append((new_line, line[1:]))
            new_line += 1
        elif line.startswith(" "):
            new_line += 1

files = []
unreadable = []
with open(os.environ["REVIEW_NUMSTAT"]) as fh:
    for raw in fh:
        parts = raw.rstrip("\n").split("\t")
        if len(parts) < 3:
            continue
        a, d, p = parts[0], parts[1], final_path("\t".join(parts[2:]).strip().strip('"'))
        if a == "-" or d == "-":
            unreadable.append(p)
            files.append({"path": p, "added": 0, "deleted": 0, "class": "binary", "signals": path_signals(p)})
            continue
        a, d = int(a), int(d)
        low = p.lower()
        name = os.path.basename(p)
        sig = path_signals(p)
        head_lines = [t for _, t in added.get(p, [])[:5]]
        if name in LOCKS:
            cls = "lock"
        elif (any(seg in low for seg in GENERATED_DIRS) or low.endswith(GENERATED_EXT) or ".generated." in low
              or any(("@generated" in t or "DO NOT EDIT" in t) for t in head_lines)):
            cls = "generated"
        elif any(m in low for m in SNAPSHOT_MARKS) or (low.startswith("testdata/") and low.endswith(".out")):
            cls = "snapshot"
        elif "agent_surface" in sig and low.endswith(DOC_EXT + CONFIG_EXT):
            cls = "prose"
        elif low.endswith(DOC_EXT) or low.startswith("docs/") or name.upper().startswith(("LICENSE", "CHANGELOG", "README")):
            cls = "docs"
        else:
            cls = "executable"
        files.append({"path": p, "added": a, "deleted": d, "class": cls, "signals": sig})

executable_lines = sum(f["added"] + f["deleted"] for f in files if f["class"] == "executable")
prose_lines = sum(f["added"] + f["deleted"] for f in files if f["class"] == "prose")
excluded = {k: sum(1 for f in files if f["class"] == k) for k in ("docs", "lock", "generated", "snapshot")}

signals = {k: [] for k in ("migrations", "frontend", "api", "tests", "agent_surface", "verification")}
for f in files:
    if f["class"] in ("lock", "generated", "snapshot"):
        continue
    for s in f["signals"]:
        signals[s].append(f["path"])

risk = []
for f in files:
    if f["class"] != "executable":
        continue
    for ln, text in added.get(f["path"], []):
        m = RISK_RE.search(text)
        if m:
            risk.append({"path": f["path"], "line": ln, "word": m.group(1).lower()})
signals["risk_words"] = risk

blockers = []
if executable_lines >= 40:
    blockers.append(f"executable_lines {executable_lines} >= 40")
if executable_lines == 0 and prose_lines == 0:
    blockers.append("no reviewable lines")
for k in ("migrations", "frontend", "api", "tests", "verification"):
    if signals[k]:
        blockers.append(f"signal {k}")
for r in risk[:5]:
    blockers.append(f"risk word {r['word']} at {r['path']}:{r['line']}")
for p in unreadable:
    blockers.append(f"unreadable {p}")

json.dump({
    "base": base, "head": head,
    "changed_files": len(files),
    "executable_lines": executable_lines,
    "prose_lines": prose_lines,
    "excluded": excluded,
    "files": files,
    "signals": signals,
    "lite_eligible": not blockers,
    "lite_blockers": blockers,
}, sys.stdout, indent=1)
print()
PY
}

# ---------------------------------------------------------------- merge

cmd_merge() {
  need_python
  local run_dir="" reconciled="" out="" roster=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --reconciled) reconciled="${2:-}"; shift 2 ;;
      --out) out="${2:-}"; shift 2 ;;
      --roster) roster="${2:-}"; shift 2 ;;
      -h|--help) usage; exit 0 ;;
      -*) die "merge: unknown option $1" ;;
      *) [ -z "$run_dir" ] || die "merge: one run dir only"; run_dir="$1"; shift ;;
    esac
  done
  [ -n "$run_dir" ] && [ -d "$run_dir" ] || die "merge: run dir missing"
  [ -z "$reconciled" ] || [ -f "$reconciled" ] || die "merge: reconciled file missing: $reconciled"
  REVIEW_RUN_DIR="$run_dir" REVIEW_RECONCILED="$reconciled" REVIEW_OUT="${out:-$run_dir/merged.json}" REVIEW_ROSTER="$roster" \
    python3 - <<'PY'
import glob, json, os, re, sys

run_dir = os.environ["REVIEW_RUN_DIR"]
reconciled = os.environ.get("REVIEW_RECONCILED") or ""
out_path = os.environ["REVIEW_OUT"]
roster = [r for r in (os.environ.get("REVIEW_ROSTER") or "").split(",") if r]

SKIP = {"merged.json", "reconciled.json", "review.json", "findings.json", "metadata.json",
        "pr-review-payload.json", "peer-schema.json", "peer-opencode.json"}
SEVERITY = {"P0": 0, "P1": 1, "P2": 2, "P3": 3}
ANCHORS = (0, 25, 50, 75, 100)
CLASS_RANK = {"gated_auto": 0, "manual": 1, "advisory": 2}
OWNER_RANK = {"downstream-resolver": 0, "release": 1, "human": 2}
PROMOTE = {50: 75, 75: 100, 100: 100}
TESTING_BUCKET = {"marksmanship-hunter"}
RISK_BUCKET = {"balance-druid", "restoration-shaman", "windwalker-monk", "havoc-demon-hunter",
               "augmentation-evoker", "discipline-priest", "havoc-demon-hunter-peer"}
HARVEST = {"lore-bard"}


def warn(msg):
    print("review.sh merge: " + msg, file=sys.stderr)


def fingerprint(f):
    path = os.path.normpath(str(f.get("file", ""))).lower()
    title = " ".join(str(f.get("title", "")).lower().split())
    return (path, str(f.get("line", "")), title)


def load_json(path):
    with open(path) as fh:
        return json.load(fh)


def reviewer_independent(name, artifact):
    if name == "fast-pass":
        return False
    if name.endswith("-peer"):
        return bool(artifact.get("independence_verified") is True)
    return True


def normalize(f, reviewer, hydration):
    required = ("title", "severity", "file", "line", "confidence")
    if not isinstance(f, dict) or any(k not in f or f[k] in (None, "") for k in required):
        return None
    sev = str(f["severity"]).upper()
    if sev not in SEVERITY:
        return None
    try:
        conf = int(f["confidence"])
    except (TypeError, ValueError):
        return None
    if conf not in ANCHORS:
        conf = max(a for a in ANCHORS if a <= conf) if conf > 0 else 0
    evidence = [e for e in f.get("evidence", [])] if isinstance(f.get("evidence"), list) else []
    first = f.get("first_evidence") if isinstance(f.get("first_evidence"), str) else None
    first = first or (evidence[0] if evidence and isinstance(evidence[0], str) else None)
    return {
        "title": str(f["title"]).strip(),
        "severity": sev,
        "file": str(f["file"]),
        "line": int(f["line"]) if str(f["line"]).isdigit() else f["line"],
        "confidence": conf,
        "autofix_class": f.get("autofix_class") if f.get("autofix_class") in CLASS_RANK else "manual",
        "owner": f.get("owner") if f.get("owner") in OWNER_RANK else "downstream-resolver",
        "requires_verification": bool(f.get("requires_verification", False)),
        "pre_existing": bool(f.get("pre_existing", False)),
        "suggested_fix": f.get("suggested_fix"),
        "first_evidence": first,
        "why_it_matters": f.get("why_it_matters"),
        "evidence": [str(e) for e in evidence],
        "requirement": f.get("requirement"),
        "reviewers": [reviewer],
        "independent_reviewers": [],
        "contribs": [{"reviewer": reviewer, "confidence": conf}],
        "corroborated": False,
        "promoted": False,
        "gate": "primary",
        "hydration": hydration,
        "soft_candidate": None,
        "bucket": None,
    }


counts = {"suppressed": {"0": 0, "25": 0}, "clamped_fast_pass": 0, "demoted_missing_evidence": 0,
          "dedup_merged": 0, "promoted": 0, "dropped_confidence_gate": 0, "soft_candidates": 0,
          "pre_existing": 0, "primary": 0, "malformed": 0, "reviewers_missing": []}
reviewers = []
dismissed = []
residual_risks = []
testing_gaps = []
independent_names = {}
work = []

if reconciled:
    doc = load_json(reconciled)
    passno = int(doc.get("pass", 1)) + 1
    reviewers = doc.get("reviewers", [])
    independent_names = {r["name"]: bool(r.get("independent")) for r in reviewers if "name" in r}
    dismissed = list(doc.get("dismissed", []))
    residual_risks = list(doc.get("residual_risks", []))
    testing_gaps = list(doc.get("testing_gaps", []))
    prior = doc.get("counts", {})
    for k in ("suppressed", "clamped_fast_pass", "demoted_missing_evidence", "dedup_merged", "malformed",
              "promoted", "dropped_confidence_gate"):
        if k in prior:
            counts[k] = prior[k]
    counts["reviewers_missing"] = prior.get("reviewers_missing", [])
    for f in doc.get("findings", []) + doc.get("soft_candidates", []) + doc.get("pre_existing", []):
        if not isinstance(f, dict):
            continue
        # the reconciled file was edited by hand: re-apply the type and enum rules before any gate
        if str(f.get("severity", "")).upper() not in SEVERITY or not f.get("title") or not f.get("file"):
            counts["malformed"] += 1
            dismissed.append({"title": str(f.get("title", "?"))[:80], "reviewers": f.get("reviewers", []),
                              "reason": "malformed after reconciliation", "stage": "merge"})
            continue
        f["severity"] = str(f["severity"]).upper()
        try:
            c = int(f.get("confidence", 50))
        except (TypeError, ValueError):
            c = 50
        f["confidence"] = c if c in ANCHORS else (max(a for a in ANCHORS if a <= c) if c > 0 else 0)
        if f.get("autofix_class") not in CLASS_RANK:
            f["autofix_class"] = "manual"
        if f.get("owner") not in OWNER_RANK:
            f["owner"] = "downstream-resolver"
        if not isinstance(f.get("first_evidence"), str):
            ev = f.get("evidence") if isinstance(f.get("evidence"), list) else []
            f["first_evidence"] = ev[0] if ev and isinstance(ev[0], str) else None
        if not isinstance(f.get("evidence"), list):
            f["evidence"] = []
        f["pre_existing"] = bool(f.get("pre_existing", False))
        f["requires_verification"] = bool(f.get("requires_verification", False))
        f.setdefault("reviewers", [])
        f.setdefault("independent_reviewers", [])
        f.setdefault("contribs", [{"reviewer": r, "confidence": 50} for r in f["reviewers"]])
        for k in ("corroborated", "promoted"):
            f.setdefault(k, False)
        f.setdefault("gate", "primary")
        f.setdefault("hydration", "artifact")
        f.setdefault("soft_candidate", None)
        f.setdefault("bucket", None)
        f.setdefault("requirement", None)
        f.pop("#", None)
        work.append(f)
else:
    passno = 1
    loaded = {}
    for path in sorted(glob.glob(os.path.join(run_dir, "*.json"))):
        name = os.path.basename(path)
        if name in SKIP or name.startswith("peer-"):
            continue
        stem = name[:-5]
        try:
            art = load_json(path)
        except Exception as e:
            warn(f"{name} does not parse: {e}")
            loaded[stem] = {"status": "malformed", "artifact": None}
            counts["malformed"] += 1
            continue
        if not isinstance(art, dict) or "reviewer" not in art or not isinstance(art.get("findings"), list):
            warn(f"{name} is not a reviewer artifact")
            loaded[stem] = {"status": "malformed", "artifact": None}
            counts["malformed"] += 1
            continue
        loaded[stem] = {"status": "ok", "artifact": art, "source": "artifact"}
    for path in sorted(glob.glob(os.path.join(run_dir, "returns", "*.json"))):
        stem = os.path.basename(path)[:-5]
        if loaded.get(stem, {}).get("status") == "ok":
            continue
        try:
            ret = load_json(path)
            if not isinstance(ret, dict) or not isinstance(ret.get("findings"), list):
                raise ValueError("not a compact return")
        except Exception as e:
            warn(f"returns/{stem}.json does not parse: {e}")
            continue
        loaded[stem] = {"status": "ok", "artifact": ret, "source": "return"}
    for stem in sorted(set(loaded) | set(roster)):
        entry = loaded.get(stem)
        if not entry or entry["status"] != "ok":
            reviewers.append({"name": stem, "status": entry["status"] if entry else "missing",
                              "source": None, "independent": stem != "fast-pass", "findings_in": 0})
            if stem in roster:
                counts["reviewers_missing"].append(stem)
            continue
        art, source = entry["artifact"], entry["source"]
        indep = reviewer_independent(stem, art)
        independent_names[stem] = indep
        row = {"name": stem, "status": "ok", "source": source, "independent": indep,
               "findings_in": len(art.get("findings", []))}
        if stem.endswith("-peer"):
            row["model"] = art.get("model")
            row["peer_cli"] = art.get("peer_cli")
        reviewers.append(row)
        hydration = "artifact" if source == "artifact" else "return-only"
        for f in art.get("findings", []):
            n = normalize(f, stem, hydration)
            if n is not None and hydration == "artifact" and not (n["why_it_matters"] and n["evidence"]):
                n["hydration"] = "thin"
            if n is None:
                counts["malformed"] += 1
                dismissed.append({"title": str((f or {}).get("title", "?"))[:80], "reviewers": [stem],
                                  "reason": "malformed finding", "stage": "merge"})
                continue
            work.append(n)
        for r in art.get("residual_risks", []) or []:
            residual_risks.append({"reviewer": stem, "text": str(r)})
        for t in art.get("testing_gaps", []) or []:
            testing_gaps.append({"reviewer": stem, "text": str(t)})

# ---- gate 1: fast-pass clamp
for f in work:
    if f["reviewers"] == ["fast-pass"] and f["confidence"] > 50:
        f["confidence"] = 50
        for c in f["contribs"]:
            c["confidence"] = min(c["confidence"], 50)
        counts["clamped_fast_pass"] += 1

# ---- gate 2: suppress 0 and 25
kept = []
for f in work:
    if f["confidence"] in (0, 25):
        counts["suppressed"][str(f["confidence"])] += 1
    else:
        kept.append(f)
work = kept

# ---- gate 3: quote the line
for f in work:
    if f["confidence"] >= 75 and not (f.get("first_evidence") or "").strip():
        f["confidence"] = 50
        for c in f["contribs"]:
            c["confidence"] = min(c["confidence"], 50)
        counts["demoted_missing_evidence"] += 1

# ---- gate 4: exact dedup
merged = {}
order = []
for f in work:
    key = fingerprint(f)
    if key not in merged:
        merged[key] = f
        order.append(key)
        continue
    m = merged[key]
    counts["dedup_merged"] += 1
    for r in f["reviewers"]:
        if r not in m["reviewers"]:
            m["reviewers"].append(r)
    for r in f.get("independent_reviewers", []):
        if r not in m["independent_reviewers"]:
            m["independent_reviewers"].append(r)
    m["contribs"].extend(f["contribs"])
    for e in f["evidence"]:
        if e not in m["evidence"]:
            m["evidence"].append(e)
    if len(f.get("why_it_matters") or "") > len(m.get("why_it_matters") or ""):
        m["why_it_matters"] = f["why_it_matters"]
    if CLASS_RANK[f["autofix_class"]] > CLASS_RANK[m["autofix_class"]]:
        m["autofix_class"] = f["autofix_class"]
    if OWNER_RANK[f["owner"]] > OWNER_RANK[m["owner"]]:
        m["owner"] = f["owner"]
    m["requires_verification"] = m["requires_verification"] or f["requires_verification"]
    m["pre_existing"] = m["pre_existing"] and f["pre_existing"]
    if SEVERITY[f["severity"]] < SEVERITY[m["severity"]]:
        m["severity"] = f["severity"]
    if f["confidence"] > m["confidence"]:
        m["confidence"] = f["confidence"]
        if f.get("suggested_fix"):
            m["suggested_fix"] = f["suggested_fix"]
    elif not m.get("suggested_fix") and f.get("suggested_fix"):
        m["suggested_fix"] = f["suggested_fix"]
    if not m.get("first_evidence") and f.get("first_evidence"):
        m["first_evidence"] = f["first_evidence"]
    if not m.get("requirement") and f.get("requirement"):
        m["requirement"] = f["requirement"]
    m["promoted"] = m["promoted"] or f["promoted"]
    m["corroborated"] = m["corroborated"] or f["corroborated"]
    m["bucket"] = m["bucket"] or f["bucket"]
    if m["hydration"] != "artifact" and f["hydration"] == "artifact":
        m["hydration"] = "artifact"
work = [merged[k] for k in order]

# ---- gate 5: promotion across independent reviewers
for f in work:
    indep = list(f.get("independent_reviewers", []))
    for c in f["contribs"]:
        r = c["reviewer"]
        if r in indep:
            continue
        if not independent_names.get(r, reviewer_independent(r, {})):
            continue
        if r in HARVEST and c["confidence"] < 75:
            continue
        indep.append(r)
    f["independent_reviewers"] = indep
    if len(indep) >= 2:
        f["corroborated"] = True
        if not f["promoted"]:
            f["confidence"] = PROMOTE.get(f["confidence"], f["confidence"])
            f["promoted"] = True
            counts["promoted"] += 1

# ---- gate 6: confidence gate at 50, soft-candidate tagging
kept = []
for f in work:
    if f["bucket"]:
        kept.append(f)
        continue
    src = f["reviewers"][0] if f["reviewers"] else ""
    single = len(f["reviewers"]) == 1
    if single and f["autofix_class"] == "advisory" and f["severity"] in ("P2", "P3"):
        if src in TESTING_BUCKET:
            f["soft_candidate"] = "testing_gaps"
        elif src in RISK_BUCKET:
            f["soft_candidate"] = "residual_risks"
    if f["confidence"] > 50:
        kept.append(f)
        continue
    if f["severity"] == "P0":
        f["gate"] = "p0_escape"
        kept.append(f)
    elif src in TESTING_BUCKET:
        f["soft_candidate"] = "testing_gaps"
        kept.append(f)
    elif src in RISK_BUCKET:
        f["soft_candidate"] = "residual_risks"
        kept.append(f)
    elif src in HARVEST:
        f["soft_candidate"] = "harvested"
        kept.append(f)
    else:
        counts["dropped_confidence_gate"] += 1
        dismissed.append({"title": f["title"], "reviewers": f["reviewers"], "file": f["file"], "line": f["line"],
                          "reason": "confidence gate (50, %s)" % f["severity"], "stage": "merge"})
work = kept

# ---- gate 7: partition
findings, soft, pre = [], [], []
for f in work:
    b = f["bucket"]
    if b == "dismissed":
        dismissed.append({"title": f["title"], "reviewers": f["reviewers"], "file": f["file"], "line": f["line"],
                          "reason": f.get("reason") or "lead judgment", "stage": f.get("stage") or "lead"})
        continue
    if b in ("testing_gaps", "residual_risks"):
        target = testing_gaps if b == "testing_gaps" else residual_risks
        target.append({"reviewer": f["reviewers"][0] if f["reviewers"] else "synthesis",
                       "text": "%s (%s:%s)" % (f["title"], f["file"], f["line"])})
        continue
    if b == "primary":
        (pre if f["pre_existing"] else findings).append(f)
        continue
    if f["pre_existing"]:
        pre.append(f)
    elif f["soft_candidate"]:
        soft.append(f)
    else:
        findings.append(f)


# ---- gate 8: sort and number
def sort_key(f):
    return (SEVERITY[f["severity"]], -int(f["confidence"]), str(f["file"]), int(f["line"]) if str(f["line"]).isdigit() else 0, f["title"])


findings.sort(key=sort_key)
pre.sort(key=sort_key)
n = 0
for f in findings + pre:
    n += 1
    f["#"] = n
    f.setdefault("bucket", "primary")
    f["bucket"] = "primary"

counts["primary"] = len(findings)
counts["pre_existing"] = len(pre)
counts["soft_candidates"] = len(soft)

outdoc = {
    "pass": passno,
    "run_dir": run_dir,
    "reviewers": reviewers,
    "findings": findings,
    "soft_candidates": soft,
    "pre_existing": pre,
    "dismissed": dismissed,
    "residual_risks": residual_risks,
    "testing_gaps": testing_gaps,
    "counts": counts,
}
with open(out_path, "w") as fh:
    json.dump(outdoc, fh, indent=1)
    fh.write("\n")
print("merged pass %d: %d primary, %d soft, %d pre-existing, %d dismissed, %d promoted -> %s" % (
    passno, len(findings), len(soft), len(pre), len(dismissed), counts["promoted"], out_path))
PY
}

# ---------------------------------------------------------------- peer

cmd_peer() {
  need_python
  local cli="" run_dir="" brief="" constraints="" host="" timeout="600" check=0 named=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --cli) cli="${2:-}"; shift 2 ;;
      --run-dir) run_dir="${2:-}"; shift 2 ;;
      --brief) brief="${2:-}"; shift 2 ;;
      --constraints) constraints="${2:-}"; shift 2 ;;
      --host) host="${2:-}"; shift 2 ;;
      --timeout) timeout="${2:-}"; shift 2 ;;
      --check) check=1; shift ;;
      --named-by-user) named=1; shift ;;
      -h|--help) usage; exit 0 ;;
      *) die "peer: unknown argument $1" ;;
    esac
  done
  [ -n "$cli" ] || die "peer: --cli is required"
  [ -n "$run_dir" ] || die "peer: --run-dir is required"
  [ -n "$host" ] || die "peer: --host is required"
  if [ "$check" -eq 0 ]; then
    [ -f "$brief" ] || die "peer: --brief file missing"
    [ -f "$constraints" ] || die "peer: --constraints file missing"
    [ -d "$run_dir" ] || die "peer: run dir missing"
    [ -f "$SCHEMA" ] || die "peer: schema missing at $SCHEMA"
  fi
  PEER_CLI="$cli" PEER_RUN_DIR="$run_dir" PEER_BRIEF="$brief" PEER_CONSTRAINTS="$constraints" PEER_HOST="$host" \
  PEER_TIMEOUT="$timeout" PEER_CHECK="$check" PEER_NAMED="$named" PEER_SCHEMA="$SCHEMA" \
    python3 - <<'PY'
import json, os, re, shutil, subprocess, sys, time

cli = os.environ["PEER_CLI"]
run_dir = os.environ["PEER_RUN_DIR"]
brief = os.environ["PEER_BRIEF"]
constraints = os.environ["PEER_CONSTRAINTS"]
host = os.environ["PEER_HOST"].lower()
timeout = int(os.environ["PEER_TIMEOUT"])
check = os.environ["PEER_CHECK"] == "1"
named = os.environ["PEER_NAMED"] == "1"
schema_src = os.environ["PEER_SCHEMA"]

CLI_FAMILY = {"codex": "openai", "claude": "anthropic", "gemini": "google", "grok": "xai",
              "cursor-agent": "unknown", "opencode": "unknown"}
REVIEWER = "havoc-demon-hunter-peer"
SEVERITY = ("P0", "P1", "P2", "P3")
CLASSES = ("gated_auto", "manual", "advisory")
OWNERS = ("downstream-resolver", "human", "release")
ANCHORS = (0, 25, 50, 75, 100)


def out(msg, code):
    print(msg)
    sys.exit(code)


def family_of(model):
    m = (model or "").lower()
    if m.startswith("claude"):
        return "anthropic"
    if m.startswith(("gpt", "o1", "o3", "o4", "codex")):
        return "openai"
    if m.startswith("gemini"):
        return "google"
    if m.startswith("grok"):
        return "xai"
    return "unknown"


if cli not in CLI_FAMILY:
    out(f"peer: {cli} is not a supported route ({', '.join(sorted(CLI_FAMILY))})", 2)
if not shutil.which(cli):
    out(f"peer: {cli} is not installed", 2)
if CLI_FAMILY[cli] == host and not named:
    out(f"peer: {cli} serves the host's own family ({host}); pass peer:{cli} explicitly to use it anyway", 2)
disclosure = f"peer review: sending the diff and brief to {cli}; the diff leaves this machine."
if check:
    out(disclosure, 0)

prompt = (f"Read {constraints} first and obey every rule in it. Then read {brief} and follow its numbered steps. "
          f"Output nothing except the single JSON object the brief asks for.")

with open(schema_src) as fh:
    schema = json.load(fh)
schema.pop("$schema", None)
schema_path = os.path.join(run_dir, "peer-schema.json")
with open(schema_path, "w") as fh:
    json.dump(schema, fh)

env = dict(os.environ)
stdin_text = None
last_msg = None
if cli == "codex":
    last_msg = os.path.join(run_dir, f"peer-{cli}.last")
    argv = ["codex", "exec", "--sandbox", "read-only", "--ephemeral", "--skip-git-repo-check", "--json",
            "--output-last-message", last_msg, "--output-schema", schema_path, prompt]
elif cli == "claude":
    with open(constraints) as fh:
        sysprompt = fh.read()
    argv = ["claude", "-p", "--output-format", "json", "--allowedTools", "Read", "Grep", "Glob",
            "--disallowedTools", "Bash", "Edit", "Write", "MultiEdit", "NotebookEdit", "WebFetch", "WebSearch",
            "--strict-mcp-config", "--system-prompt", sysprompt, prompt]
elif cli == "gemini":
    with open(constraints) as fh:
        stdin_text = fh.read()
    argv = ["gemini", "-p", prompt, "--approval-mode", "plan", "--output-format", "json"]
elif cli == "cursor-agent":
    argv = ["cursor-agent", "-p", "--mode", "ask", "--output-format", "json", prompt]
elif cli == "opencode":
    overlay = os.path.join(run_dir, "peer-opencode.json")
    with open(overlay, "w") as fh:
        json.dump({"permission": {"edit": "deny", "bash": "deny", "webfetch": "deny"}}, fh)
    env["OPENCODE_CONFIG"] = overlay
    argv = ["opencode", "run", "--format", "json", prompt]
else:  # grok
    argv = ["grok", "--output-format", "json", "--json-schema", json.dumps(schema),
            "--disallowed-tools", "bash,edit,write,shell,run_command,create_file,edit_file", "--no-plan", prompt]

raw_path = os.path.join(run_dir, f"peer-{cli}.out")
t0 = time.time()
try:
    proc = subprocess.run(argv, input=stdin_text, capture_output=True, text=True, timeout=timeout, env=env)
    raw = (proc.stdout or "") + "\n" + (proc.stderr or "")
    rc = proc.returncode
except subprocess.TimeoutExpired as e:
    raw = ((e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or "")) + "\n[timeout]"
    rc = None
except OSError as e:
    out(f"peer: could not start {cli}: {e}", 2)
duration = round(time.time() - t0, 1)
with open(raw_path, "w") as fh:
    fh.write(raw)
if rc is None:
    out(f"peer: {cli} timed out after {timeout}s (raw output in {raw_path})", 3)
if rc != 0 and not re.search(r'"reviewer"\s*:', raw):
    tail = " ".join(raw.split())[-200:]
    out(f"peer: {cli} exited {rc} with no output: {tail} (raw output in {raw_path})", 2)

texts = [raw]
if last_msg and os.path.exists(last_msg):
    with open(last_msg) as fh:
        texts.insert(0, fh.read())


def objects_in(text):
    dec = json.JSONDecoder()
    i = 0
    while True:
        i = text.find("{", i)
        if i < 0:
            return
        try:
            obj, end = dec.raw_decode(text, i)
        except ValueError:
            i += 1
            continue
        yield obj
        i = end


def walk(obj, depth=0):
    # yield every dict reachable through envelopes: result/response strings, nested dicts, lists
    if depth > 4:
        return
    if isinstance(obj, dict):
        yield obj
        for v in obj.values():
            if isinstance(v, str) and "{" in v:
                for o in objects_in(v):
                    yield from walk(o, depth + 1)
            elif isinstance(v, (dict, list)):
                yield from walk(v, depth + 1)
    elif isinstance(obj, list):
        for v in obj:
            yield from walk(v, depth + 1)


found = None
envelopes = []
for text in texts:
    for o in objects_in(text):
        for d in walk(o):
            envelopes.append(d)
            if "reviewer" in d and isinstance(d.get("findings"), list):
                found = d
if found is None:
    out(f"peer: {cli} returned no findings object (raw output in {raw_path})", 3)

model = found.get("model") if isinstance(found.get("model"), str) else None
if not model:
    for d in envelopes:
        for key in ("modelUsage", "models"):
            v = d.get(key)
            if isinstance(v, dict) and v:
                model = next(iter(v.keys()))
                break
        if model:
            break
        if isinstance(d.get("model"), str) and d["model"]:
            model = d["model"]
            break
if not model:
    m = re.search(r'"model"\s*:\s*"([^"]+)"', raw)
    model = m.group(1) if m else None

valid = []
dropped = 0
for f in found["findings"]:
    ok = (isinstance(f, dict) and all(k in f for k in ("title", "severity", "file", "line", "why_it_matters",
                                                        "autofix_class", "owner", "requires_verification",
                                                        "confidence", "evidence", "pre_existing"))
          and f["severity"] in SEVERITY and f["autofix_class"] in CLASSES and f["owner"] in OWNERS
          and f["confidence"] in ANCHORS and isinstance(f["evidence"], list) and f["evidence"])
    if ok:
        f.setdefault("first_evidence", f["evidence"][0])
        valid.append(f)
    else:
        dropped += 1

if found["findings"] and not valid:
    out(f"peer: {cli} returned {dropped} findings, every one malformed (raw output in {raw_path})", 3)
fam = family_of(model)
artifact = {
    "reviewer": REVIEWER,
    "findings": valid,
    "residual_risks": [str(r) for r in found.get("residual_risks", []) or []],
    "testing_gaps": [str(t) for t in found.get("testing_gaps", []) or []],
    "model": model,
    "independence_verified": bool(model) and fam != "unknown" and fam != host,
    "peer_cli": cli,
    "peer_duration_s": duration,
    "peer_dropped_findings": dropped,
}
with open(os.path.join(run_dir, REVIEWER + ".json"), "w") as fh:
    json.dump(artifact, fh, indent=1)
    fh.write("\n")
print("peer: %s returned %d findings (%d dropped as malformed), model %s, independence %s, %.0fs" % (
    cli, len(valid), dropped, model or "unverified", "verified" if artifact["independence_verified"] else "not verified", duration))
sys.exit(0)
PY
}

# ---------------------------------------------------------------- dispatch

case "${1:-}" in
  signals) shift; cmd_signals "$@" ;;
  merge) shift; cmd_merge "$@" ;;
  peer) shift; cmd_peer "$@" ;;
  -h|--help|"") usage; [ -n "${1:-}" ] && exit 0 || exit 1 ;;
  *) die "unknown subcommand $1" ;;
esac
