# TDD

Red, then green. One slice at a time.

Use this loop for meaningful behavior changes with a test harness. Skip it for documentation, configuration, and other changes with no behavioral effect. Those changes still run the project's required validation.

Confirm the seams (the public interfaces you will observe) before the first test. Write them in the Stage 1 intent. No test is written at a seam you did not name.

## A test worth keeping

It verifies behavior through a public interface. The name reads like a capability ("user can checkout with a valid cart"). Internals can change; the test still holds.

Expected values come from an independent source: a known-good literal, a worked example, the ticket. An assertion that recomputes the answer the way the code does will never catch a bug.

## The loop

1. Write one failing test at one seam.
2. Run it. See it fail for the reason you intended.
3. Write only enough code to pass it.
4. Run it. See it pass.
5. Repeat for the next slice.

Do not write a pile of tests and then the implementation. Each test is a tracer: it responds to what the last cycle taught you.

Skip tests that pin private methods, mock internal collaborators, or inspect a side channel (a database row the interface already returns). Those break on refactors that leave behavior alone.

Refactoring sits outside this loop. Get the behavior green, then clean up if the ticket asked for that.
