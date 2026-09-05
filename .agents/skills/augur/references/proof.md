# The proving script

A proof is a script that calls the real code and exits non-zero when the safety fact is false. Its output is pasted into the report verbatim.

## Where it lives

```bash
SCRATCH_ROOT="/tmp/augur-$(id -u)";
if [ -L "$SCRATCH_ROOT" ]; then echo "unsafe scratch root symlink: $SCRATCH_ROOT" >&2; exit 1; fi;
install -d -m 700 "$SCRATCH_ROOT" || exit 1;
if [ -L "$SCRATCH_ROOT" ] || [ ! -O "$SCRATCH_ROOT" ]; then echo "scratch root not owned by current user" >&2; exit 1; fi;
chmod 700 "$SCRATCH_ROOT" || exit 1;
RUN_DIR="$SCRATCH_ROOT/$(date +%Y%m%d-%H%M%S)-$(head -c4 /dev/urandom | od -An -tx1 | tr -d ' ')";
(umask 077; mkdir -p "$RUN_DIR") || exit 1;
echo "$RUN_DIR"
```

The script goes in `$RUN_DIR`, never in the repo. Its path goes in the report so the user can rerun it.

## Rules

- **Same runtime, same versions.** Run it with the project's own toolchain and dependency versions: `node` from the repo's lockfile, `python` from its virtualenv, `bundle exec`, `cargo run --example`, whatever the project uses. A proof against a different version of the library proves nothing about the app.
- **Import the real thing.** Call the exact function or module the change touches, from the repo's own code or the pinned library. A reimplementation of the logic inside the script is rung 1 dressed up.
- **Fail loud.** Assert the bad case and exit non-zero when it happens. A script that prints values for you to eyeball is not a proof; a reader cannot rerun your eyes.
- **Show the bad case too.** When the fact is "X can never be null here", the strongest script constructs the input that would make it null and shows the guard fires. A proof that only exercises the happy path proves the happy path.
- **Never mutate the repo.** No edits to tracked files, no commits, no branch switches. Read-only `git` is fine. If the proof needs a fixture, write it under `$RUN_DIR`.
- **Paste verbatim.** The command, the exit code, and the output as printed. Trim only repeated lines, and say you trimmed.

## When a cheap proof is impossible

Some facts need infrastructure, a device, secrets, or a long-running service. Say so in one line, name the rung you did reach, and write the fact as unproven. Then put the cheapest real check in "Before you merge": a staging run, a specific manual step, a test the author can add. Do not round an unproven fact up to proven because the paragraph reads well.

## Shape

```bash
#!/usr/bin/env bash
# Proves: <the fact, one line>
set -euo pipefail
cd "<repo root>"
<runtime> - <<'CODE'
<import the real module>
<construct the bad case>
<assert; exit 1 on failure>
print("ok: <the fact>")
CODE
```
