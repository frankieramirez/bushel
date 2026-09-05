# Issue tracker: Jira

Tracker: jira
Project: <PROJECT KEY, like PLAT>
Adapter flags: --tracker jira --project <PROJECT KEY>
Auth: `JIRA_BASE_URL` (https://<site>.atlassian.net), `JIRA_EMAIL`, and `JIRA_API_TOKEN` in the environment. Optional `JIRA_ISSUE_TYPE`, default `Task`. Never in a file.
Keys: `PLAT-42`. A bare number is not a ticket here.
Host: <JIRA_BASE_URL>

## Conventions

Every write goes through the adapter script the skills carry (`tickets.sh --tracker jira --project <PROJECT KEY>`): create, wire, next, claim, label, comment, close. `view` and `list` read. Labels are Jira labels, free text with no registry, so `ensure-labels` is a no-op. "Open" means a status whose category is not Done. `close` takes the first available transition into a Done status. Bodies are written as plain paragraphs; Jira does not render Markdown.

Pull requests stay on GitHub. A pull request that resolves a Jira issue puts the key in its branch name and title. Ending the body with `Closes PLAT-42` closes the issue only when a Jira automation rule reads it; otherwise the merge leaves the issue for a person to transition.

## Pull requests as a request surface

No.

## External authors

Anyone without a Jira account on this site.

## Wayfinding operations

The map is an issue in this project labelled `scry:map`, of a type that can have children (an Epic, or a Task under a project with sub-tasks). Tickets are its children, labelled `scry:<type>`. Blocking is a `Blocks` issue link from the blocker to the ticket. The frontier is every child not in a Done status, with no assignee and no open blocker, oldest first. Claim is assignment. Resolve is a comment, a transition to Done, and one gist line appended to the map's description under Decisions so far.

Do these with the Jira connector the host exposes when there is one. Otherwise use the REST API under `JIRA_BASE_URL/rest/api/3` with basic auth from `JIRA_EMAIL` and `JIRA_API_TOKEN`: `POST /issue`, `POST /issueLink`, `PUT /issue/{key}/assignee`, `POST /issue/{key}/comment`, `POST /issue/{key}/transitions`, `POST /search/jql`.
