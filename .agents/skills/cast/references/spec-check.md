# Spec check

Walk the ticket against the diff before you commit. The ticket wins.

## Collect the criteria

From the agent brief, spec, or issue body, list every acceptance criterion and every explicit out-of-scope line. If the ticket has no checklist, write four or fewer observable outcomes from the desired behavior (what a user or caller can see).

## For each criterion

- **Met.** Quote the file or test that shows it.
- **Unmet.** Fix it now, or stop. An unmet criterion is a failed cast.
- **Out of scope on the ticket.** Leave it. Mention it under Open in the report so it does not look forgotten.

## Extra pass

Read the diff once more for work the ticket did not ask for. Revert drive-by edits that are unrelated. Keep a change that the ticket's desired behavior required even if the checklist omitted it, and name it in the report.

## Validation

Re-run the project's suite (or the commands you already used in Stage 2) after the last edit. Classify each failure against the pre-change baseline: the same command before this session's edits, or a failure recorded at the start. A new failure: fix it or do not commit. A failure proven to have existed before this session: proceed, and add a commit footer `Note: <test> was already failing before these changes.`
