# Project: Knock Knock

> Private support chat that disappears from human view.

## Vision

Developers should be able to click a button in a GitHub README, authenticate with GitHub in one click, ask a question or make a suggestion, receive an answer from maintainers, and leave.

No joining servers.

No channels.

No persistent social graph.

No installation.

No commitment.

It should feel like knocking on a maintainer's office door rather than joining a community.

See [Privacy and Retained Conversation Logs](PRIVACY.md) for the distinction
between the human viewing window and private long-term storage.

---

## Philosophy

- Ephemeral to humans; complete logs are retained privately for future paid AI
  support and AI-generated FAQs.
- GitHub-native.
- Zero friction.
- Mobile-friendly.
- The core experience never depends on AI.
- One repository = one room.
- Questions first, community second.

---

## Core User Flow

Visitor:

1. Click "Ask a question" badge in README.
2. GitHub OAuth.
3. Immediately enters the repository's room.
4. Ask question.
5. Maintainers are notified.
6. A maintainer answers.
7. User leaves.
8. Conversation closes to human access after a configurable viewing window.
9. The complete log remains privately retained for future paid AI support and
   AI-generated FAQs.

Maintainer:

1. GitHub login.
2. Claim ownership of repository.
3. See incoming questions.
4. Reply from web or mobile.
5. Promote conversation to:

- GitHub Discussion
- GitHub Issue
- Documentation TODO

---

## MVP

### GitHub Integration

- GitHub OAuth
- Repository ownership verification
- README badge generator
- Installation instructions

### Rooms

Every GitHub repository automatically maps to

```
owner/repo
```

No setup.

Visiting a room creates it if necessary.

---

### Chat

Features:

- markdown
- code fences
- syntax highlighting
- drag/drop images
- emoji reactions
- typing indicator
- read receipts optional
- presence

No threads.

No channels.

---

### Explicitly excluded: AI

AI is not built for the MVP. The MVP has no model dependency, repository
indexing, embeddings, AI responses, or placeholder AI interface.

AI will be an optional paid capability for repository accounts in a later
release. It may eventually

- answer
- cite repository README
- cite docs
- cite previous promoted discussions

If confidence is low, it should hand off:

> "I'm not sure. I'll let the maintainers answer."

---

### Human viewing window and private log retention

Default:

```text
Hidden from humans after 30 days.
```

Closing the viewing window does not delete the conversation. Knock Knock retains
the complete log privately, including messages, attachments, participants, and
timestamps, solely to support future paid AI agents and AI-generated FAQs. Those
features are not part of the MVP.

Conversation pages require authorization and are never exposed to search engines.
After the window closes, visitors and maintainers cannot reopen the conversation.

Maintainer may click

```
Promote
```

which can

- create GitHub Discussion
- create GitHub Issue
- export Markdown

---

### Notifications

Maintainers receive

- email
- GitHub notification (future)
- webhook
- optional Discord webhook
- optional Slack webhook

---

## Nice UI

Landing page:

```markdown
💬 owner/repo

37 maintainers online

────────────────────────────

Ask anything...

__________________________

```

No giant sidebar.

No servers.

No channel tree.

The interface should disappear.

---

## Badge

```
[ Ask a Question ]
```

or

```
💬 Ask
```

generated automatically.

---

## Repository Dashboard

Maintainers see

```
27 conversations

4 awaiting reply

Answered within 1 hour
61%

Median response
18 minutes

```

---

## Paid AI Analytics (post-MVP)

Weekly report

People struggled with

• installation
• auth
• Windows

Suggested docs:

README.md
docs/windows.md

---

## Pricing

Open Source

- free

Commercial

- AI
- analytics
- custom branding
- longer human viewing windows
- priority notifications

---

## Technical Goals

- absurdly fast
- websocket based
- GitHub identity only
- no passwords
- horizontal scaling
- event sourced
- searchable only by participants during the human viewing window
- human access closes automatically
- complete logs retained privately for future paid AI and FAQ generation
- conversation pages never indexed by search engines

---

## Non-goals

Not Discord.

Not Slack.

Not Matrix.

Not Teams.

Not forums.

Not social media.

Not a permanent human-browsable knowledge base.

---

## Guiding Principles

Every feature must make answering a single question easier.

If a feature primarily helps people build a community, it probably doesn't belong.

The product exists to reduce friction, not increase engagement.

Success is measured by how quickly users arrive, ask, receive an answer, and leave.

---

## Things That Would Make This Special

### 1. Zero setup

If I type

```yaml
https://knock.chat/mxcl/portal
```

it just exists.

The first maintainer who authenticates with GitHub and has admin rights automatically becomes an owner.

No project creation.

No billing until you ask for premium features.

No "Create Workspace".

---

### 2. Paid AI can become the receptionist after MVP

Not the support agent.

```yaml
User:
How do I install this on Fedora?

AI:
Looks like this project only supports macOS.

Does that answer your question?

[Yes]
[No]
```

If No:

```
Forwarding to maintainers...
```

The maintainer never even knows about the easy ones.

---

### 3. Presence matters

One thing Gitter got right:

```
🟢 Max is here
```

changes everything.

People ask more questions if they know someone is actually around.

Conversely:

```
Nobody is around.
Typical response: 12 hours.
```

sets expectations.

---

### 4. Tiny rooms

Not communities.

Not "General".

Not "Random".

Every repo gets one room.

That's it.

---

### 5. Conversations disappear

This is the killer feature.

```text
Hidden from human view after 14 days.
```

No search.

No search-engine indexing.

No embarrassment.

People are astonishingly more willing to ask "stupid" questions when they know
the transcript cannot be revisited by humans or become the first Google result
forever.

The complete log is still retained in a private, machine-only corpus for future
paid AI support and FAQ generation. That retention must be disclosed before the
visitor sends a message.

---

### 6. Promote, don't make history browsable

If a conversation is useful:

```
Promote →

○ GitHub Issue
○ GitHub Discussion
○ FAQ
```

Otherwise it disappears from every human-facing product surface when the viewing
window closes, while the private machine corpus remains retained.

---

## Paid AI Stretch Goal (post-MVP)

This is the one feature I think could be genuinely novel.

Imagine every message has an **entropy score**.

AI decides whether it's likely to help future users.

```yaml
"Thanks!"

entropy: 0

(exclude from FAQ)
```

```yaml
"Homebrew on macOS 27 fails because of SIP."

entropy: 0.96

Recommend promotion.
```

The paid AI can synthesize reusable FAQ entries without exposing the retained
source conversations to human readers.

---

## Architecture

I wouldn't build this like Discord.

I'd build it almost like GitHub Issues with live updates.

```
Repo
 ├── Conversation
 │     ├── Messages
 │     ├── Participants
 │     ├── Human View TTL
 │     └── Retained Log
 │
 └── Maintainers
```

No guilds.

No channels.

No voice.

No roles.

No bots.

Everything revolves around a **conversation**, not a room.

---

## The bit that excites me

I actually think the button belongs in the README:

```md
[![Ask a question](https://knock.chat/badge.svg)](https://knock.chat/mxcl/portal)
```

Not:

> Join our Discord

Not:

> Open a Discussion

Just:

> **💬 Ask a question**

That's such a low-friction invitation. It says, "It's okay to interrupt."

---

One paid capability that feels particularly compelling in 2026: **for paid
repositories, make AI available to their visitors while keeping human
conversations private**.

When someone arrives, they first see an AI chat trained on the repo. If that solves the problem, great. If not, the AI seamlessly says, "I'll bring a maintainer into this conversation," and the thread simply continues. From the user's perspective, it's one conversation. From the maintainer's perspective, they're only spending time where the AI genuinely got stuck.

That turns AI into a filter rather than a gimmick, and it gives commercial open source teams a very clear reason to pay while leaving the core "knock on the door" experience free.
