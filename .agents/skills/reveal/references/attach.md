# Attach

Ship the pull request with `scripts/open-pr.sh`. That script is the only place that runs `gh pr create` or `gh pr edit` with `--attach`.

## Body

Write the body per `references/body.md`. Then put the proof in by the same path string you pass as `--attach`. `gh` rewrites a reference only when the two match character for character, and appends a second copy when they do not, so use the absolute path of the file inside the `mktemp -d` directory in both places (`<DIR>` below stands for that expanded path):

```markdown
![the settings page after the save](<DIR>/settings.png)
```

A video that should play as a player sits alone in its paragraph:

```markdown
![](<DIR>/demo.mp4)
```

Video has no alt text.

## open-pr.sh

The body file lives in the same directory. A fixed path such as `/tmp/pr-body.md` is writable by every local user and must not be used.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>"; DIR="<the mktemp -d directory>";
bash "$SKILL_DIR/scripts/open-pr.sh" \
  --title "Title from the branch or ticket" \
  --body-file "$DIR/pr-body.md" \
  --attach "$DIR/settings.png#the settings page after the save" \
  --attach "$DIR/tests.svg"
```

`--title` is required when this branch has no pull request yet. The script looks up the current branch's PR. Existing: `gh pr edit --body-file` and `--attach`. None: `gh pr create`.

`--attach` is `path` or `path#alt`. Repeat the flag. Paths must not contain `#`.

The script checks `gh --version` against 2.99.0 and the repo host against github.com. Older CLI, GitHub Enterprise Server, or a token the upload API refuses (an installation `ghs_` token): the PR still opens and attach is skipped. The script prints `attach` as `skipped` and a `reason`. Tell the caller to upgrade `gh` or use OAuth / a classic PAT.

Need write access to the repository. Images and GIFs cap at 10 MB. Video caps at 10 MB on Free and 100 MB on paid plans. The script rejects a file over 100 MB and an image over 10 MB.

Prints:

```
action	create|edit
url	https://github.com/OWNER/REPO/pull/N
number	N
attach	yes|skipped
reason	<empty, or why attach was skipped>
```

`--dry-run` prints the same fields plus `command`, checks the files, and does not create or edit.
