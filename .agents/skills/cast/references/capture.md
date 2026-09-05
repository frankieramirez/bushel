# Capture

Every pull request ships a real image or video. The file proves the change works. A failed run is discarded; fix it and recapture.

`--attach` accepts PNG, JPEG, GIF, WebP, SVG, MP4, MOV, and WebM.

## Preference

Pick the first that applies.

1. A screenshot or short recording of the user-visible change. Use the screenshot or recording tools the host already offers. Inside an Orca worktree (`ORCA_WORKTREE_ID` is set and `command -v orca` succeeds), Orca's embedded browser is one of them; see below.
2. When there is no GUI surface, an SVG of the proving command's actual output (the test run or the CLI invocation). Run `scripts/text-frame.sh` on that output.
3. Last resort: an SVG of the commit subject plus the validation command already run for this change.

## Quality

The smallest set that shows the change works. One recording of the flow beats four stills of the same screen.

Keep out:

- The editor, the file tree, and other IDE chrome
- Install steps, login waits, and spinner time
- Duplicate frames of the same state
- A run that failed

Write every file under a fresh private directory, `DIR=$(mktemp -d)`, and pass its absolute path to `--attach` and into the body. Never a fixed path under `/tmp`: another local user can create that file first and control what goes into the pull request. Never `git add` them.

## text-frame.sh

`SKILL_DIR` is the absolute directory the `SKILL.md` lives in. The Bash tool forgets variables between calls, so every block that runs the script sets `SKILL_DIR` again on its first line.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>"; DIR="<the mktemp -d directory>";
<the proving command> 2>&1 | bash "$SKILL_DIR/scripts/text-frame.sh" "$DIR/tests.svg"
```

The script reads stdin and writes a dark monospace SVG. It XML-escapes the text and strips ANSI color codes. Long lines wrap. Cap the input at what the command actually printed.

Name the file for what it shows (`tests.svg`, `cli-search.svg`).

## Orca's embedded browser

Only inside an Orca worktree. Open the page, reach the state, and capture:

```bash
orca tab create --url <url> --json      # only when screenshot reports browser_no_tab
orca goto --url <url> --json
orca snapshot --json                    # element refs like @e3 for click and fill
orca click --element @e3 --json
orca screenshot --json | python3 -c 'import base64,json,sys; open(sys.argv[1],"wb").write(base64.b64decode(json.load(sys.stdin)["result"]["data"]))' "$DIR/feature.png"
```

`full-screenshot` captures beyond the viewport. The JSON carries the image as base64 under `result.data`; the one-liner writes it to a file. `DIR` is the temp directory from `mktemp -d`. The tab has to be visible in the app: a `Screenshot timed out` error means the browser pane is hidden or another worktree is in front, and `orca tab switch --index 0 --json` is the one retry worth making. A call that still fails falls through to the next preference; it is never a stop.
