#!/usr/bin/env bash
# text-frame.sh: wrap stdin in a dark monospace SVG.
#   text-frame.sh OUT.svg
set -euo pipefail

usage() {
  cat <<'EOF'
usage: text-frame.sh OUT.svg

  Reads text on stdin. Writes a dark monospace SVG.
EOF
}

die() {
  echo "text-frame.sh: $*" >&2
  exit 1
}

xml_escape() {
  # sed, so a literal & in the replacement is \&. Bash ${var/a/&b} treats & as the match.
  printf '%s' "$1" | sed \
    -e 's/&/\&amp;/g' \
    -e 's/</\&lt;/g' \
    -e 's/>/\&gt;/g' \
    -e 's/"/\&quot;/g' \
    -e "s/'/\&apos;/g"
}

[ $# -eq 1 ] || { usage; exit 2; }
case "$1" in
  -h|--help|help) usage; exit 0 ;;
esac

out=$1
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

# Strip CR and CSI color sequences, expand tabs, wrap at 120.
tr -d '\r' | sed -e $'s/\x1b\\[[0-9;?]*[a-zA-Z]//g' -e $'s/\x1b\\][^\x07]*\x07//g' \
  | expand -t 4 | fold -w 120 >"$tmp" || die "failed to read stdin"

[ -s "$tmp" ] || die "stdin was empty"

maxlen=1
nlines=0
while IFS= read -r line || [ -n "$line" ]; do
  nlines=$((nlines + 1))
  [ "${#line}" -gt "$maxlen" ] && maxlen=${#line}
done <"$tmp"

if [ "$nlines" -gt 80 ]; then
  cut=$(mktemp)
  head -n 80 "$tmp" >"$cut"
  printf '%s\n' "... (truncated)" >>"$cut"
  mv "$cut" "$tmp"
  nlines=81
  maxlen=15
  while IFS= read -r line || [ -n "$line" ]; do
    [ "${#line}" -gt "$maxlen" ] && maxlen=${#line}
  done <"$tmp"
fi

pad=16
char_w=8
line_h=18
width=$((maxlen * char_w + pad * 2))
height=$((nlines * line_h + pad * 2))
[ "$width" -lt 320 ] && width=320

{
  printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
  printf '<svg xmlns="http://www.w3.org/2000/svg" width="%s" height="%s" font-family="ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace" font-size="13">\n' "$width" "$height"
  printf '  <rect width="100%%" height="100%%" fill="#1e1e1e"/>\n'
  printf '  <text fill="#d4d4d4" xml:space="preserve">\n'
  y=$((pad + 12))
  while IFS= read -r line || [ -n "$line" ]; do
    printf '    <tspan x="%s" y="%s">%s</tspan>\n' "$pad" "$y" "$(xml_escape "$line")"
    y=$((y + line_h))
  done <"$tmp"
  printf '  </text>\n'
  printf '</svg>\n'
} >"$out"
