# Research

AFK. A subagent investigates one question against primary sources and writes a cited Markdown file.

## Job

1. Answer the ticket's Question from official docs, source code, specs, or first-party APIs. Follow every claim back to the source that owns it. A blog post about the API is a pointer; the API reference is the source.
2. Write one file. Default path: `docs/research/YYYY-MM-DD-<slug>.md`. If the repo already keeps research notes somewhere else, match that convention.
3. Cite each claim (URL, path, or spec section).
4. Return the file path, a five-line gist, and any fact later tickets will need.

The subagent does not close the ticket. The parent session posts the gist, links the file, and closes.

## File shape

```markdown
# <question in a line>

## Findings

<the answer, lead with it>

## Sources

- <url or path>: <what it established>
```

Stay on the current branch. Do not switch branches to isolate the note. Commit only if the parent session asked you to.

## Dispatch

Spawn a generic subagent. Seed it with this file and the ticket's Question, number, and title. Give it read access to the repo and the network. It writes the research file and returns. It does not edit the map, comment on the ticket, or start other tickets.
