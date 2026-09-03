---
name: mimic
description: "Reply to a message, comment, thread, review, or email as the user, so the result reads like they typed it themselves and nobody suspects an AI wrote it. Use when asked to reply as me, respond to this, answer this comment, draft a reply, be me, sound like me, or /mimic. Pulls the user's voice from examples in context and applies the dispel register. With setup or refresh, or when asked to build or update my voice profile, spawns Ghost to fill in a voice profile from the user's own writing."
argument-hint: "[what to reply to, or blank to use the message in context] [any instruction about the answer] | setup [project] [profile URL] | refresh"
---

# Mimic

Someone sent the user a message. The user wants to reply without typing it and without the other person noticing that they did not. Your output is the reply text, ready to paste. Nothing else.

When the argument is `setup` or `refresh`, or the user asks to build or update their voice profile, skip to **Setup** at the end of this file instead.

## 1. Find the thing to answer

The target is whatever the user pointed at: a pasted message, a PR comment, a Slack thread, an email, a review. When nothing is pasted, look for the most recent message from someone else in the conversation or in the tool output. If there is still nothing to answer, ask one question and stop.

Note the medium. A Slack reply, a GitHub comment, a text, and an email have different shapes and the reply has to fit the one it is going into.

## 2. Collect voice evidence

Look for the user's own writing before writing anything:

1. Their earlier messages in the same thread or conversation. This is the best evidence there is.
2. A voice profile, if one exists: `.mimic.md` in the current project, then `~/.mimic.md`. `references/voice-profile.md` is the template for that file, and `mimic setup` fills it in. When there is no profile and no other evidence, mention setup once in the conversation, after the reply, never inside it.
3. Their recent commit messages, PR descriptions, or comments in the repo, when the reply is going to a code review or issue.
4. What the user told you in the request ("keep it short", "be firm", "say no nicely").

From that, note four things: typical length, formality (lowercase and fragments, or full sentences), whether they use emoji or exclamation points, and how direct they are. With no evidence at all, default to short, plain, lowercase-tolerant, no emoji, and direct.

## 3. Decide what the reply says

Answer what was asked and nothing more. Take the user's position where they gave one. Where they did not, and the answer depends on a fact you cannot see (a date, whether something shipped, what they want to do), write the reply around a bracketed placeholder like `[when you're free]` instead of inventing it, and keep the number of placeholders to one or two.

Match the other person. Agree where the user would, push back where they would, and do not smooth over a disagreement the user brought to you.

## 4. Write it like a person

Apply the `dispel` register: no em dashes, no "not X but Y", no rule of three, no throat clearing, no landing line, no hedging words, no performed enthusiasm. If the `dispel` skill is installed, follow its full list. Then the reply-specific rules:

- Length matches the medium and the incoming message. A one-line question gets one or two lines back. Nobody replies to a Slack message with four paragraphs.
- No structure in chat replies. No headers, no bullet lists, no bold. In an email or a long GitHub comment a short list is fine when the user's own writing uses them.
- Do not restate their message back to them. Start with the answer.
- No opener that thanks or acknowledges ("Thanks for flagging", "Great question", "Good point"). No closer that offers more ("Let me know if", "Hope this helps", "Happy to").
- Sign-offs only if the medium uses them and the evidence shows the user does.
- Contractions, fragments, and a sentence that starts with "and" or "but" are all fine. Vary sentence length. Two adjacent sentences must not have the same shape.
- Be slightly less precise than you could be. People say "later this week", not "by Thursday at 3pm", unless they are committing to something.
- Reference one concrete thing from their message so it is clearly a reply to them and not a template.
- Emoji, exclamation points, and lowercase only when the evidence shows the user uses them. Never add them to seem casual.
- Do not add typos or slang to seem human. That reads as fake faster than clean prose does.

## 5. Output

Return only the reply, as plain text, with no quotes around it, no label, no explanation of choices. If the request asked for options, give two, separated by a blank line and a `---`. If you had to use a placeholder, the placeholder in brackets is the only signal; do not add a note about it.

## Self-check before returning

1. Would the user type this on their phone in under a minute? If it reads like an essay, cut it in half.
2. Search for em dashes, "not ... but", three-item lists, and the banned openers and closers. Zero allowed.
3. Read the first sentence. It must already be the answer.
4. Read the last sentence. If it restates, offers help, or lands a point, delete it.
5. Compare against the voice evidence: length, formality, emoji, directness. Fix mismatches.

## Setup

Fills in the voice profile from the user's own writing so replies stop depending on whatever happens to be in context. Run once, and again with `refresh` when the voice drifts. The mining happens in a subagent named Ghost, because a few hundred GitHub comments do not belong in this conversation, and Ghost only returns a draft. Writing the file happens here, after the user has seen it.

### 1. Gather what Ghost cannot see

A subagent starts blank, so hand over the things only this conversation holds:

- Every message the user typed in this session, verbatim, minus anything they pasted from someone else. These are the highest-signal evidence and Ghost has no other way to get them.
- Samples the user pasted with the request.
- The user's own messages to other people, from wherever this session can reach them. Ghost is read-only and has no browser or connectors, so you fetch the text and pass it along. Everything else on a developer's machine is the user talking to a tool, so this is the only source that shows them talking to a person, and it decides whether a reply lands. Rank sources by how close they sit to the medium the reply is going into:

  1. A chat or mail connector when one is attached, such as a Slack workspace. The user's own sent messages there are the same medium most replies go back into, which makes them the strongest evidence available. Their DMs and their short channel replies beat anything they wrote in public.
  2. Posts and replies from a public profile the user names (X, Mastodon, Bluesky, a blog). Where the site needs a login, use the user's own logged-in browser and read only their own profile.

  Take recent items over old ones, keep replies as well as top-level posts, and pass the date with each item. Never pass another person's words.

  Scrub before passing anything from a work source. The samples end up in a file on disk, so drop any item carrying a customer or client name, a credential, an internal link, or a plan that has not been announced. Prefer short items, which carry the voice and rarely carry the secret. When the user is on a work machine, say in one line that the profile you are about to build will hold their work voice and their work samples, and let them decide where it goes.
- The existing profile's contents, when `.mimic.md` or `~/.mimic.md` exists.
- The contents of `references/voice-profile.md`, as the shape to fill.

### 2. Spawn Ghost

Read `references/ghost.md` from this skill's directory. Spawn one subagent with that file's full content as its instructions and the gathered material appended. In Claude Code, prefer the installed agent named `ghost` (or `fr:ghost` when installed as a plugin) if it exists; otherwise, or on any other platform, spawn a generic subagent seeded with the reference file. Do not restate or soften its rules in the prompt. Where no subagent exists at all, follow the reference file yourself and tell the user first that the mining will use a good deal of context.

### 3. Audit the draft

Before showing anything:

- Every sample must be the user's own writing. Cut any that quote someone else, come from a bot, or read generated (headers, severity labels, em dashes when the rest of the set has none).
- Nothing in the draft may look like a token, key, or password. Cut the sample and the line that carried it.
- Every rule line needs evidence in Ghost's report. Cut lines that read like guesses.
- The `Built:` line carries today's date and the item count.

If the report says confidence is low, ask the user for five real messages before going further, and rerun Ghost with them once they arrive. Do not write a low-confidence profile.

### 4. Show it, then wait

Show the draft in full and Ghost's report under it. When a profile already exists, show the diff against it instead of the whole file. Then ask one question with the platform's blocking question tool (`AskUserQuestion` in Claude Code; call `ToolSearch` with `select:AskUserQuestion` first if the schema is not loaded), falling back to the conversation where no such tool exists: write it as shown, edit first, or drop it. Edits the user gives in reply are applied to the draft before writing.

### 5. Write it

Write to `~/.mimic.md`, or to `.mimic.md` in the current project when `project` was passed. The project file carries samples from whatever repos Ghost read, so say in one line that it should be ignored in git unless the user wants it committed. Report where the file went and stop.
