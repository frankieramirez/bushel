#!/usr/bin/env bash
# map.sh: GitHub map operations for scry. git + gh only; JSON shaping is gh --jq.
#
#   ensure-labels
#   create-map    TITLE                 body on stdin
#   create-ticket MAP TYPE TITLE        body on stdin; TYPE is research|prototype|grilling|task
#   wire          CHILD BLOCKER
#   frontier      MAP
#   claim         NUMBER
#   view          NUMBER
#   parent        NUMBER
#   comment       NUMBER                body on stdin
#   close         NUMBER
#   update-body   NUMBER                body on stdin
#
# Labels are scry:map and scry:<type>. Reads also accept the older wayfinder:* names
# so maps filed before 0.11.0 still walk; nothing writes them.
# Prints TSV or a small field list. Exit 3 when GitHub refuses a write (403).
# Honors GH_HOST. Optional last owner/repo argument on commands that take numbers;
# without it, gh repo view in this checkout.

set -euo pipefail

ERR=""
trap '[ -n "${ERR:-}" ] && rm -f "$ERR"' EXIT

usage() {
  cat <<'EOF'
usage: map.sh <subcommand> [args]

  ensure-labels
  create-map    TITLE [owner/repo]          body on stdin; prints number<TAB>url
  create-ticket MAP TYPE TITLE [owner/repo] body on stdin; prints number<TAB>url
  wire          CHILD BLOCKER [owner/repo]
  frontier      MAP [owner/repo]            number<TAB>title<TAB>type<TAB>url
  claim         NUMBER [owner/repo]
  view          NUMBER [owner/repo]
  parent        NUMBER [owner/repo]         parent map number, or empty
  comment       NUMBER [owner/repo]         body on stdin
  close         NUMBER [owner/repo]
  update-body   NUMBER [owner/repo]         body on stdin

TYPE is research, prototype, grilling, or task.
Exit 3 means this token cannot write issues; use the scratch fallback.
EOF
}

die() {
  echo "map.sh: $*" >&2
  exit 1
}

# Runs gh. Prints stderr on failure. Returns 0, 1, or 3 (403-class). Does not exit.
try_gh() {
  local ec
  ERR=$(mktemp)
  set +e
  "$@" 2>"$ERR"
  ec=$?
  set -e
  if [ "$ec" -eq 0 ]; then
    rm -f "$ERR"
    ERR=""
    return 0
  fi
  cat "$ERR" >&2
  if grep -qiE '403|Resource not accessible|HTTP 403' "$ERR"; then
    rm -f "$ERR"
    ERR=""
    return 3
  fi
  rm -f "$ERR"
  ERR=""
  return "$ec"
}

# Runs gh. On 403-class write denial, exit 3. OWNER/REPO already set when needed.
run_gh() {
  try_gh "$@" || exit $?
}

locate_repo() {
  local spec="${1:-}"
  if [ -z "$spec" ]; then
    spec=$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null || true)
  fi
  case "$spec" in
    */*) OWNER="${spec%%/*}"; REPO="${spec#*/}" ;;
    *) die "cannot tell which repository to use; run inside a checkout or pass owner/repo" ;;
  esac
  [ -n "$OWNER" ] && [ -n "$REPO" ] || die "malformed owner/repo: '$spec'"
}

db_id() {
  local n="$1"
  gh api "repos/${OWNER}/${REPO}/issues/${n}" --jq .id
}

ticket_type() {
  local n="$1"
  gh api "repos/${OWNER}/${REPO}/issues/${n}" --jq '
    [.labels[].name]
    | map(select((startswith("scry:") or startswith("wayfinder:")) and . != "scry:map" and . != "wayfinder:map"))
    | map(sub("^(scry|wayfinder):";""))
    | first // ""
  '
}

cmd_ensure_labels() {
  local name existing
  existing=$(gh label list --repo "$OWNER/$REPO" --limit 100 --json name --jq '.[].name')
  for name in scry:map scry:research scry:prototype scry:grilling scry:task; do
    if printf '%s\n' "$existing" | grep -qxF "$name"; then
      continue
    fi
    run_gh gh label create --repo "$OWNER/$REPO" "$name" --color "6f42c1" >/dev/null
  done
}

cmd_create_map() {
  local title="$1"
  local tmp out num url
  tmp=$(mktemp)
  cat > "$tmp"
  out=$(run_gh gh issue create --repo "$OWNER/$REPO" --title "$title" --label "scry:map" --body-file "$tmp")
  rm -f "$tmp"
  url=$(printf '%s\n' "$out" | tail -n 1)
  num="${url##*/}"
  printf '%s\t%s\n' "$num" "$url"
}

# POST an API write. Return 1 on any failure, including 403, so the caller can fall back.
# Scratch (exit 3) is only for create, edit, comment, and close, via run_gh.
api_write() {
  local err ec
  err=$(mktemp)
  set +e
  gh api "$@" >/dev/null 2>"$err"
  ec=$?
  set -e
  rm -f "$err"
  [ "$ec" -eq 0 ]
}

cmd_create_ticket() {
  local map="$1" typ="$2" title="$3"
  local tmp out url num child_id body attach_ec
  case "$typ" in
    research|prototype|grilling|task) ;;
    *) die "type must be research, prototype, grilling, or task (got '$typ')" ;;
  esac
  tmp=$(mktemp)
  cat > "$tmp"
  out=$(run_gh gh issue create --repo "$OWNER/$REPO" --title "$title" --label "scry:${typ}" --body-file "$tmp")
  rm -f "$tmp"
  url=$(printf '%s\n' "$out" | tail -n 1)
  num="${url##*/}"
  child_id=$(db_id "$num")
  if ! api_write --method POST "repos/${OWNER}/${REPO}/issues/${map}/sub_issues" -F "sub_issue_id=${child_id}"; then
    body=$(gh issue view --repo "$OWNER/$REPO" "$num" --json body --jq .body)
    tmp=$(mktemp)
    printf 'Part of #%s\n\n%s\n' "$map" "$body" > "$tmp"
    attach_ec=0
    try_gh gh issue edit --repo "$OWNER/$REPO" "$num" --body-file "$tmp" || attach_ec=$?
    if [ "$attach_ec" -ne 0 ]; then
      rm -f "$tmp"
      gh issue close --repo "$OWNER/$REPO" "$num" >/dev/null 2>&1 || true
      exit "$attach_ec"
    fi
    rm -f "$tmp"
  fi
  printf '%s\t%s\n' "$num" "$url"
}

cmd_wire() {
  local child="$1" blocker="$2"
  local blocker_id body tmp
  blocker_id=$(db_id "$blocker")
  if ! api_write --method POST "repos/${OWNER}/${REPO}/issues/${child}/dependencies/blocked_by" -F "issue_id=${blocker_id}"; then
    body=$(gh issue view --repo "$OWNER/$REPO" "$child" --json body --jq .body)
    tmp=$(mktemp)
    printf 'Blocked by: #%s\n\n%s\n' "$blocker" "$body" > "$tmp"
    run_gh gh issue edit --repo "$OWNER/$REPO" "$child" --body-file "$tmp"
    rm -f "$tmp"
  fi
}

# True (exit 0) when the issue has an open blocker via API or a Blocked by: line.
is_blocked() {
  local n="$1"
  local raw body ids id st
  if raw=$(gh api "repos/${OWNER}/${REPO}/issues/${n}/dependencies/blocked_by" --jq '
    (if type == "array" then . else (.blocked_by // []) end)
    | [.[] | select(.state == "open" or .state == "OPEN")]
    | length
  ' 2>/dev/null); then
    [ "${raw:-0}" -gt 0 ]
    return
  fi
  body=$(gh issue view --repo "$OWNER/$REPO" "$n" --json body --jq .body)
  ids=$(printf '%s\n' "$body" | sed -n '1,8p' | grep -E '^Blocked by:' | sed 's/[^0-9, ]//g' | tr ',' ' ')
  for id in $ids; do
    [ -n "$id" ] || continue
    st=$(gh issue view --repo "$OWNER/$REPO" "$id" --json state --jq .state 2>/dev/null || true)
    if [ "$st" = "OPEN" ] || [ "$st" = "open" ]; then
      return 0
    fi
  done
  return 1
}

has_assignee() {
  local n="$1"
  local count
  count=$(gh issue view --repo "$OWNER/$REPO" "$n" --json assignees --jq '.assignees | length')
  [ "${count:-0}" -gt 0 ]
}

# Collect child numbers: sub-issues first, then Part of #<map> / task-list fallback.
child_numbers() {
  local map="$1"
  local nums body
  if nums=$(gh api --paginate "repos/${OWNER}/${REPO}/issues/${map}/sub_issues" --jq '.[].number' 2>/dev/null); then
    if [ -n "$nums" ]; then
      printf '%s\n' "$nums"
      return
    fi
  fi
  body=$(gh issue view --repo "$OWNER/$REPO" "$map" --json body --jq .body)
  {
    printf '%s\n' "$body" | grep -E '^[[:space:]]*[-*][[:space:]]*\[[ xX]\]' | grep -oE '#[0-9]+' | tr -d '#' || true
    gh issue list --repo "$OWNER/$REPO" --state open --limit 100 --json number,body --jq '.[] | select(.body | test("Part of #'"$map"'(\\s|$)")) | .number' 2>/dev/null || true
  } | awk 'NF && !seen[$0]++'
}

cmd_frontier() {
  local map="$1"
  local n typ title url state
  while IFS= read -r n; do
    [ -n "$n" ] || continue
    state=$(gh issue view --repo "$OWNER/$REPO" "$n" --json state --jq .state)
    case "$state" in
      OPEN|open) ;;
      *) continue ;;
    esac
    if has_assignee "$n"; then
      continue
    fi
    if is_blocked "$n"; then
      continue
    fi
    typ=$(ticket_type "$n")
    title=$(gh issue view --repo "$OWNER/$REPO" "$n" --json title --jq .title)
    url=$(gh issue view --repo "$OWNER/$REPO" "$n" --json url --jq .url)
    printf '%s\t%s\t%s\t%s\n' "$n" "$title" "$typ" "$url"
  done < <(child_numbers "$map")
}

assignees_except() {
  local n="$1" me="$2"
  gh issue view --repo "$OWNER/$REPO" "$n" --json assignees --jq '[.assignees[].login] | map(select(. != "'"$me"'")) | join(",")'
}

cmd_claim() {
  local n="$1"
  local login others
  login=$(gh api user --jq .login)
  others=$(assignees_except "$n" "$login")
  if [ -n "$others" ]; then
    die "already claimed by $others"
  fi
  run_gh gh issue edit --repo "$OWNER/$REPO" "$n" --add-assignee "$login"
  others=$(assignees_except "$n" "$login")
  if [ -n "$others" ]; then
    gh issue edit --repo "$OWNER/$REPO" "$n" --remove-assignee "$login" >/dev/null 2>&1 || true
    die "already claimed by $others"
  fi
}

cmd_view() {
  local n="$1"
  gh issue view --repo "$OWNER/$REPO" "$n" --json number,title,url,state,body,labels,assignees --jq '
    [
      "number\t\(.number)",
      "title\t\(.title)",
      "url\t\(.url)",
      "state\t\(.state)",
      "labels\t\([.labels[].name] | join(","))",
      "assignees\t\([.assignees[].login] | join(","))",
      "body",
      .body
    ] | join("\n")
  '
}

cmd_parent() {
  local n="$1"
  local p body
  p=$(gh api graphql -F owner="$OWNER" -F repo="$REPO" -F number="$n" -f query='
    query($owner:String!,$repo:String!,$number:Int!){
      repository(owner:$owner,name:$repo){
        issue(number:$number){ parent { number } }
      }
    }' --jq '.data.repository.issue.parent.number // empty' 2>/dev/null || true)
  if [ -n "$p" ]; then
    printf '%s\n' "$p"
    return
  fi
  body=$(gh issue view --repo "$OWNER/$REPO" "$n" --json body --jq .body)
  printf '%s\n' "$body" | sed -n '1,12p' | grep -E '^Part of #' | head -n 1 | grep -oE '[0-9]+' || true
}

cmd_comment() {
  local n="$1"
  local body
  body=$(cat)
  run_gh gh issue comment --repo "$OWNER/$REPO" "$n" --body "$body"
}

cmd_close() {
  local n="$1"
  run_gh gh issue close --repo "$OWNER/$REPO" "$n"
}

cmd_update_body() {
  local n="$1"
  local tmp
  tmp=$(mktemp)
  cat > "$tmp"
  run_gh gh issue edit --repo "$OWNER/$REPO" "$n" --body-file "$tmp"
  rm -f "$tmp"
}

cmd="${1:-}"
shift || true

case "$cmd" in
  -h|--help|help|"") usage; exit 0 ;;
  ensure-labels)
    locate_repo "${1:-}"
    cmd_ensure_labels
    ;;
  create-map)
    [ $# -ge 1 ] || die "create-map TITLE"
    title="$1"; shift
    locate_repo "${1:-}"
    cmd_create_map "$title"
    ;;
  create-ticket)
    [ $# -ge 3 ] || die "create-ticket MAP TYPE TITLE"
    map="$1"; typ="$2"; title="$3"; shift 3
    locate_repo "${1:-}"
    cmd_create_ticket "$map" "$typ" "$title"
    ;;
  wire)
    [ $# -ge 2 ] || die "wire CHILD BLOCKER"
    child="$1"; blocker="$2"; shift 2
    locate_repo "${1:-}"
    cmd_wire "$child" "$blocker"
    ;;
  frontier)
    [ $# -ge 1 ] || die "frontier MAP"
    map="$1"; shift
    locate_repo "${1:-}"
    cmd_frontier "$map"
    ;;
  claim|view|parent|comment|close|update-body)
    [ $# -ge 1 ] || die "$cmd NUMBER"
    number="$1"; shift
    locate_repo "${1:-}"
    case "$cmd" in
      claim) cmd_claim "$number" ;;
      view) cmd_view "$number" ;;
      parent) cmd_parent "$number" ;;
      comment) cmd_comment "$number" ;;
      close) cmd_close "$number" ;;
      update-body) cmd_update_body "$number" ;;
    esac
    ;;
  *) die "unknown subcommand: $cmd" ;;
esac
