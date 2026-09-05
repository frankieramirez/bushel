# Issue tracker: Linear

Tracker: linear
Project: <TEAM KEY, the prefix on identifiers, like ENG>
Adapter flags: --tracker linear --project <TEAM KEY>
Auth: `LINEAR_API_KEY` in the environment (a personal API key from Linear settings), or the host's Linear connector, which `orca linear` is inside an Orca worktree. Never in a file.
Keys: `ENG-42`. A bare number is not a ticket here.
Host: https://linear.app/<workspace>

## Conventions

Every write goes through the adapter script the skills carry (`tickets.sh --tracker linear --project <TEAM KEY>`): create, wire, next, claim, label, comment, close. `view` and `list` read. Labels are Linear labels on this team; the triage states are labels too, not workflow states. "Open" means any workflow state that is not completed or canceled. `close` moves the issue to the team's first completed state.

Pull requests stay on GitHub. A pull request that resolves a Linear issue puts the key in its branch name or title and ends its body with `Closes ENG-42`, which Linear's GitHub integration reads.

## Pull requests as a request surface

No.

## External authors

Anyone who is not a member of the workspace.

## Wayfinding operations

The map is an issue on this team labelled `scry:map`. Tickets are its sub-issues (`parentId`), labelled `scry:<type>`. Blocking is a `blocks` relation from the blocker to the ticket. The frontier is every open sub-issue with no assignee and no open blocker, oldest first. Claim is assignment. Resolve is a comment, a move to the completed state, and one gist line appended to the map's description under Decisions so far.

Do these with the Linear connector the host exposes when there is one. Otherwise use the GraphQL API at `https://api.linear.app/graphql` with `LINEAR_API_KEY` as the `Authorization` header: `issueCreate`, `issueRelationCreate`, `issueUpdate`, `commentCreate`, and `issues(filter:)`.
