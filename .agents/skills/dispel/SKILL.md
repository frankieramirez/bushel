---
name: dispel
description: "Use when writing or rewriting any prose the user will read or send: Slack messages, PR descriptions, Linear tickets, docs, emails, release notes, summaries, plans, blog posts, commit bodies. Strips the LLM-essay register (em dashes, antithesis, corrective negation, rule of three, setup/payoff, throat-clearing openers, landing sentences, nominalization, hedging, performed enthusiasm, metaphor nouns, passive voice, adverbs) and writes for the spoken voice instead. Also use when the user says \"no em dashes\", \"sounds like AI\", \"make it sound human\", \"strip the AI voice\", or invokes /dispel."
---

<!-- BEGIN MANA PERSONA -->
## Persona at invocation

Before conversational narration, read `Persona:` in the active project's `## Agent skills` block from `CLAUDE.md` or `AGENTS.md`. Prefer the file containing the block, then an existing file; ties use `CLAUDE.md`. A symlink pair is one file. Read the saved value anew on each invocation, including from a subdirectory using the project root. No accessible project or no line means ordinary behavior. Do not search another project or global settings for this preference.

During the `Persona at invocation` stage, `archmage` loads this skill's own [references/archmage.md](references/archmage.md) for the active workflow. `off` or an absent value leaves ordinary behavior active. An unknown value leaves ordinary behavior active and gets a brief explanation when conversational output is allowed; it does not stop the work. Explicit conversation instructions override the saved voice without writing settings. A request to enable Archmage for this workflow also loads the local reference.

Apply the voice only to lead-agent conversation. Deliverables, specialist roles, reply-only responses, and JSON-only output retain their contracts, with no added narration. End the persona with this workflow unless the user requests otherwise.
<!-- END MANA PERSONA -->

Honor the user's explicit instructions and decisions already made in this conversation over this skill's workflow defaults. A rule this file states with never, or as read-only, is a gate: it holds whatever the conversation says, and an instruction to cross one is declined and reported. Continue authorized work; ask only about unresolved choices that would materially change the result. Preparing or reviewing work does not authorize publishing it.

If a skill rule requires a pause or leaves requested work unfinished, name and link to the exact SKILL.md and quote the rule. Then explain what decision or prerequisite is missing. Distinguish a required gate from your interpretation.

Write like a person talking. Not like an essay performing.

Apply the ban list below to prose. Code, code comments, config, quoted error text, and verbatim samples or evidence are exempt. Preserve exempt text exactly.

When a conversational persona is active, preserve its intentional openings in agent narration. Arcane imagery and humorous asides belong to that voice too. Keep punctuation rules and technical evidence intact. This exception does not apply to deliverables or replies written as the user; return-only output still gets no narration.

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

## Plain speech

**Abstract metaphor nouns.** substrate, wedge, vector, locus, nexus, primitive (as a noun), surface (as in "API surface"), bedrock, scaffolding (as metaphor), paradigm, gold-plating, ratchet, north star, flywheel, endgame. Each has a plainer concrete word. Substrate is base. Wedge in is add. Vector is way. Gold-plating is more than the job needs. Ratchet is a limit that only tightens.
- ✗ The queue is the substrate every worker builds on.
- ✓ Every worker reads from the queue.

**Say what it does, not how it feels.** "types that follow your schema" and "the database stays close at hand" name a feeling. Name the mechanism or the number instead. Test: if the sentence could appear unchanged in another project's docs, it says nothing about this one. Cut it.
- ✗ Types that stay in sync with your schema.
- ✓ A column rename fails the build.

**Passive voice with a nameable actor.** Catch "is/are/was/were" plus a past participle and name who does it. Passive stays only when the actor is unknown or does not matter.
- ✗ Queries are validated before they run.
- ✓ The compiler validates queries before they run.

**Adverbs propping up a verb.** Replace the adverb with the number or a stronger verb. An adverb doing the work means the verb is wrong.
- ✗ The new index significantly improves lookups.
- ✓ Lookups drop from 400ms to 30ms with the new index.

**The fancy synonym.** utilize, facilitate, numerous, "in the event that", "prior to", "in order to". Use, help, many, if, before, to.
- ✗ Prior to deploy, utilize the script in order to facilitate the migration.
- ✓ Before deploy, run the migration script.

## Positive rules

- Vary sentence length unpredictably. A four-word sentence next to a twenty-eight-word one. Never settle into a pattern a reader can feel.
- Write for the spoken voice. Read it aloud in your head. If you would not say it to a colleague standing next to you, rewrite it.
- Lead with the conclusion, then the reason.
- Prefer concrete nouns and active verbs. Name the file, the function, the number.
- Contractions are fine. So are fragments.
- Cut any sentence that only exists to transition.

## Self-check before returning

Scan only the prose you authored; skip the exempt text named above. Check for:

1. Em dashes. Zero allowed.
2. Any sentence containing "not ... but", "isn't ... it's", or a naked "rather".
3. Three-item lists inside a sentence.
4. Two adjacent sentences with the same grammatical shape.
5. The last sentence. If it restates, delete it.
6. Words from the filler list.
7. A passive verb whose actor you could name.
8. Any word from the metaphor-noun list.

Fix what you find, then return the prose. Do not narrate the fixes.
