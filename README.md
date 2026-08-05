# Project: Knock Knock

> Ephemeral support chat for GitHub repositories.

## Vision

Developers should be able to click a button in a GitHub README, authenticate with GitHub in one click, ask a question or make a suggestion, receive an answer from maintainers, and leave.

No joining servers.

No channels.

No persistent social graph.

No installation.

No commitment.

It should feel like knocking on a maintainer's office door rather than joining a community.

---

## Philosophy

- Ephemeral by default.
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
8. Conversation expires after configurable retention.

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

### Ephemerality

Default:

```
delete after 30 days
```

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
- longer retention
- priority notifications

---

## Technical Goals

- absurdly fast
- websocket based
- GitHub identity only
- no passwords
- horizontal scaling
- event sourced
- searchable only by participants
- conversations deleted automatically

---

## Non-goals

Not Discord.

Not Slack.

Not Matrix.

Not Teams.

Not forums.

Not social media.

Not a permanent knowledge base.

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

```
Deleted after 14 days.
```

No search.

No indexing.

No embarrassment.

People are astonishingly more willing to ask "stupid" questions when they know they won't become the first Google result forever.

---

### 6. Promote, don't archive

If a conversation is useful:

```
Promote →

○ GitHub Issue
○ GitHub Discussion
○ FAQ
```

Otherwise...

💥 gone.

---

## Paid AI Stretch Goal (post-MVP)

This is the one feature I think could be genuinely novel.

Imagine every message has an **entropy score**.

AI decides whether it's likely to help future users.

```yaml
"Thanks!"

entropy: 0

(delete)
```

```yaml
"Homebrew on macOS 27 fails because of SIP."

entropy: 0.96

Recommend promotion.
```

Maintainers only get asked to preserve conversations that contain genuinely reusable knowledge.

---

## Architecture

I wouldn't build this like Discord.

I'd build it almost like GitHub Issues with live updates.

```
Repo
 ├── Conversation
 │     ├── Messages
 │     ├── Participants
 │     └── TTL
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
