# Capture

Every pull request ships a real image or video. The file proves the change works. A failed run is discarded; fix it and recapture.

`--attach` accepts PNG, JPEG, GIF, WebP, SVG, MP4, MOV, and WebM.

## Preference

Pick the first that applies.

1. A screenshot or short recording of the user-visible change. Use the screenshot or recording tools the host already offers.
2. When there is no GUI surface, an SVG of the proving command's actual output (the test run or the CLI invocation). Run `scripts/text-frame.sh` on that output.
3. Last resort: an SVG of the commit subject plus the validation command already run for this change.

## Quality

The smallest set that shows the change works. One recording of the flow beats four stills of the same screen.

Keep out:

- The editor, the file tree, and other IDE chrome
- Install steps, login waits, and spinner time
- Duplicate frames of the same state
- A run that failed

Write files under a temp directory (`mktemp -d`). Never `git add` them.

## text-frame.sh

`SKILL_DIR` is the absolute directory the `SKILL.md` lives in. The Bash tool forgets variables between calls, so every block that runs the script sets `SKILL_DIR` again on its first line.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
<the proving command> 2>&1 | bash "$SKILL_DIR/scripts/text-frame.sh" /tmp/proof.svg
```

The script reads stdin and writes a dark monospace SVG. It XML-escapes the text and strips ANSI color codes. Long lines wrap. Cap the input at what the command actually printed.

Name the file for what it shows (`tests.svg`, `cli-search.svg`).
