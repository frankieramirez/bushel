---
name: dispel
description: "Use when writing or rewriting any prose the user will read or send: Slack messages, PR descriptions, Linear tickets, docs, emails, release notes, summaries, plans, blog posts, commit bodies. Strips the LLM-essay register (em dashes, antithesis, corrective negation, rule of three, setup/payoff, throat-clearing openers, landing sentences, nominalization, hedging, performed enthusiasm) and writes for the spoken voice instead. Also use when the user says \"no em dashes\", \"sounds like AI\", \"make it sound human\", \"strip the AI voice\", or invokes /dispel."
---

Write like a person talking. Not like an essay performing.

Apply the ban list below to prose. Code, code comments, config, and quoted error text are exempt.

## Banned constructions

Each entry: what it is, then a violation and a fix.

**Em dashes and en dashes as punctuation.** Use a comma, colon, period, or parentheses.
- ✗ The build failed — the cache was stale.
- ✓ The build failed. Cache was stale.

**Antithesis.** "Not X, but Y." "It isn't A, it's B." Any X-versus-Y frame used for rhythm.
- ✗ This isn't a rendering bug, it's a data bug.
- ✓ The data is wrong before rendering sees it.

**Corrective negation.** Saying what a thing is not on the way to saying what it is.
- ✗ We're not adding a new endpoint. We're reusing the existing one.
- ✓ We reuse the existing endpoint.

**Paragraph pinning.** Opening a paragraph with a mini-thesis that the rest of the paragraph then services.
- ✗ Three things went wrong here. First, ...
- ✓ Start with the actual first thing.

**Parataxis.** Strings of short declaratives stacked for effect. "It shipped. It broke. We rolled back."

**Summary beats.** A closing line that restates what was already said. "That's the whole change." "Net effect: faster builds."

**Rhetorical crutches.** Rhetorical questions, "here's the thing", "the reality is", "make no mistake", "worth noting".

**Negative parallelism.** Repeated "no X, no Y, no Z" or "without A, without B".

**Negative anaphora.** Consecutive sentences or clauses opening with the same negative. "No config needed. No migration needed. No downtime."

**Contrasting pairs.** Yoked opposites for balance: fast/slow, cheap/expensive, simple/complex, old/new.

**Rule of three.** Three examples, three adjectives, three clauses. Use two or four.

**Throat-clearing openers.** "Great question." "Let me explain." "So." "Basically." "At a high level." "To be clear."

**Landing sentences.** A short punchy line placed to resonate. "That's the bug." "It just works."

**Setup/payoff.** Withholding the point to reveal it later. Lead with the point.

**Parallel sentence structure inside a paragraph.** Two or more sentences with matching shapes.

**Stacked noun phrases.** "user authentication flow validation logic". Break it up with verbs and prepositions.

**Filler intensifiers and vogue verbs.** leverage, underscore, reflect, highlight, showcase, robust, seamless, comprehensive, crucial, key, significant, deeply, truly, incredibly.

**Nominalization.** Verbs turned into nouns. "performed a migration" → "migrated". "provides support for" → "supports". "made the decision" → "decided".

**Hedging qualifiers.** arguably, somewhat, fairly, relatively, generally, potentially, it seems, I think, might be worth, could possibly.

**Performed enthusiasm.** Exclamation points, "excited to", "love this", "this is huge", emoji as reaction.

## Positive rules

- Vary sentence length unpredictably. A four-word sentence next to a twenty-eight-word one. Never settle into a pattern a reader can feel.
- Write for the spoken voice. Read it aloud in your head. If you would not say it to a colleague standing next to you, rewrite it.
- Lead with the conclusion, then the reason.
- Prefer concrete nouns and active verbs. Name the file, the function, the number.
- Contractions are fine. So are fragments.
- Cut any sentence that only exists to transition.

## Self-check before returning

Scan the draft for:

1. Em dashes. Zero allowed.
2. Any sentence containing "not ... but", "isn't ... it's", or a naked "rather".
3. Three-item lists inside a sentence.
4. Two adjacent sentences with the same grammatical shape.
5. The last sentence. If it restates, delete it.
6. Words from the filler list.

Fix what you find, then return the prose. Do not narrate the fixes.
