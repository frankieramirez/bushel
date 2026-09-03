# Prototype

HITL. Raise the fidelity of a discussion by making a cheap, rough artifact the user can react to.

## Pick a shape from the question

- **Does this logic or state model feel right?** One HTML file the user can open. Buttons that push the state through cases that are hard to reason about on paper. Print the relevant state after every action.
- **How should this look?** Several visually different UI variations on one route, switchable from a query param or a small control. Skip polish.

If the question is ambiguous and the user is away, pick the shape that matches the surrounding code (a backend module: logic; a page or component: UI) and state that assumption at the top of the artifact.

## Rules

1. Throwaway from the first file. Put it next to the module or page it is probing, and name it so a casual reader can see it is a prototype.
2. Trivial to run. One command in the project's task runner, or a single HTML file.
3. State lives in memory. Persistence is usually the thing you are checking. If the question itself involves a database, use a scratch file with `PROTOTYPE` in the name.
4. No tests, no extra error handling, no abstractions. Learn something fast.
5. After every action (logic) or on every variant switch (UI), show the state that changed.

## After it exists

Link the artifact from the ticket. Then load `grilling.md` and walk the reaction. The prototype is evidence for the decision. It is not the destination.

When the decision lands, keep the validated answer on the ticket. Leave the prototype where it is, marked as throwaway, or on a branch that is obviously not the main line.
