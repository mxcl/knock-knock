# Knock Knock

> A temporary, GitHub-native room for every public repository.

Knock Knock gives a public GitHub repository one small, unnamed chat room. Anyone
may read an active room; a GitHub account is required to participate. Messages
remain visible for 14 days and then leave every normal human-facing view, while
complete logs remain stored for future paid AI support and FAQ generation.

Think of it as a public conversation with a bouncer beside the microphone: anyone
may listen, but speakers must show GitHub identity. Rooms are not indexed by
search engines.

See [Privacy and Retained Logs](PRIVACY.md) for the exact storage and visibility
policy. See [Product and Technical Plan](PLAN.md) for implementation details.

## Why

Developers should be able to click a button in a README, authenticate with GitHub,
say something to the people around a repository, and leave.

No server to join. No channel tree. No permanent public history. No social graph.
No commitment.

The room should feel like briefly stepping into a repository's hallway—not joining
another community platform.

## MVP product contract

### One repository, one room

Every public GitHub repository maps to:

```text
https://knock-knock.mxcl.dev/owner/repo
```

The room has one unnamed linear timeline. The MVP has no channels, threads,
direct messages, categories, or global room directory.

The home page accepts either a GitHub repository URL or `owner/repository`.
After sign-in, it also shows rooms already known to Knock Knock that the user can
manage and up to eight rooms where they have posted, ordered by recent activity.
These are private, account-specific shelves rather than global discovery.

### GitHub identity is required to post

Anyone with the direct URL may see messages in an active room. Users must
authenticate with GitHub before posting, reporting, or managing content. Any
authenticated GitHub account may participate in an active room.

Messages snapshot the author's relationship to the repository at posting time:

- `owner` — owner of a personal repository
- `maintainer` — GitHub `admin`, `maintain`, or `write`
- `collaborator` — GitHub `triage`
- no pill — no repository affiliation

GitHub remains the authority. Knock Knock does not invent its own ownership or
role model.

### Room activation

An unclaimed repository URL resolves to an introduction with a copyable link
for the maintainer and a GitHub sign-in button for maintainers who want to claim
it. After sign-in, unaffiliated visitors can also open a prefilled GitHub Issue
asking an affiliated person to activate the room, when the repository has Issues
enabled. Knock Knock does not create the issue itself.

A GitHub user with `admin` or `maintain` permission may activate, configure,
moderate, or deactivate the room. Activation requires no GitHub write permission.

Deactivation immediately removes every message from human-facing views and blocks
new posts. Reactivation starts with an empty visible timeline; old messages never
reappear, though their retained records remain stored.

### Messages

The MVP supports:

- A restricted Markdown subset
- Inline code and fenced code blocks
- Sanitized clickable URLs
- Optimistic sending with retry
- Editing and user removal
- Cursor-based history pagination

The MVP does not support image or file uploads, link previews, embedded media,
reactions, typing indicators, read receipts, unread markers, or search.

Messages are limited to 8,000 Unicode characters. URLs are links only; Knock
Knock does not fetch, proxy, preview, or persist their targets.

Edits show an `edited` marker. Removing a message leaves a small tombstone so the
surrounding exchange remains understandable. Original bodies and every revision
remain in the retained log.

### Presence

Presence answers the product's most important ambient question: is anyone here to
hear me?

The room displays unique GitHub accounts currently connected and highlights
affiliated users who are present. Multiple tabs or devices count as one person.
Presence is ephemeral and is never stored as history.

### Fourteen-day human window

Each message is visible for 14 days from its own posting time. Visibility is
derived directly from the timestamp, so there is no expiration job that can fall
behind.

After 14 days, normal product queries no longer return the message. The complete
plaintext record remains stored indefinitely by default. This distinction is
disclosed before a user posts.

Conversation routes are publicly readable but send search-engine exclusion
headers. Anyone with the direct room URL can see the current room window;
`robots.txt` disallows room routes from ordinary crawlers.

### Moderation

Any participant may report a message. GitHub `admin` and `maintain` users may:

- Hide a message from the visible room
- Mute or unmute a GitHub account in that repository
- Deactivate the room

Knock Knock operators may apply platform-wide blocks for cross-repository abuse.
Moderation changes affect human visibility but do not erase retained records.

### Badge

Activated rooms can generate README Markdown such as:

```md
[![Knock Knock](https://knock-knock.mxcl.dev/badge.svg)](https://knock-knock.mxcl.dev/mxcl/portal)
```

The **README Badge** dialog can copy this Markdown or prepare a pull request.
For pull requests, Knock Knock reads the preferred README, adds the badge after
the first `#` heading text (or after the first paragraph when there is no such
heading), creates a branch, commits the update, and opens the pull request.

The invitation is deliberately small:

> **Come say something.**

### Owner polling API

Repository admins and maintainers can create one API key from the account control
on the signed-in home page or in any room they manage. Creating another key
immediately replaces the current key. Keys are shown only when created and stored
as hashes.

Poll the owner endpoint with a bearer token:

```sh
curl -H 'Authorization: Bearer <key>' \
  https://knock-knock.mxcl.dev/api/v1/rooms/new-messages
```

It returns managed rooms with visible messages from other people posted since
that room was last opened in the browser:

```json
{
  "rooms": [
    {
      "owner": "mxcl",
      "repository": "portal",
      "url": "https://knock-knock.mxcl.dev/mxcl/portal",
      "newMessageCount": 3,
      "latestMessageAt": "2026-08-08T19:30:00Z",
      "lastOpenedAt": "2026-08-08T18:00:00Z"
    }
  ],
  "polledAt": "2026-08-08T19:31:00Z"
}
```

Polling does not clear the result; opening the room in a browser does. The
endpoint accepts one request per key per 60 seconds. Faster requests receive
`429 Too Many Requests` with a `Retry-After` header.

## Explicitly outside the MVP

- AI of any kind
- FAQ generation
- Paid accounts or billing
- Email or other notifications
- Automatic GitHub Issues or Discussions
- Markdown export or conversation promotion
- Private repositories
- Multiple channels or threads
- Global room discovery or search
- Attachments and media embeds
- Redis, distributed workers, or multi-instance realtime fanout

## Future paid AI

AI is a later paid repository capability, not dormant MVP code. Future work may:

- Answer questions using retained room history and repository documentation
- Identify recurring questions
- Synthesize public FAQ entries without publishing source transcripts

The MVP contains no model SDK, prompts, embeddings, retrieval pipeline,
entitlement checks, or placeholder AI interface. The retained logs are the only
foundation deliberately created for that future work.

## Implementation

- Rust backend using Axum, Tokio, and SQLx
- One SQLite database in WAL mode
- TypeScript, React, Vite, and shadcn/ui frontend
- Static frontend assets served by the Rust process
- Durable commands over HTTP and realtime events over WebSockets
- One Atlas process
- In-memory realtime fanout and presence
- Hourly database backups downloaded to Pangolin by existing infrastructure

The service intentionally uses SQLite and in-process fanout while it runs as a
single instance.

## Run it locally

You need Rust and Cargo with Rust 2024 edition support, plus Node.js and npm. The
production Docker build currently uses Rust 1.96 and Node.js 26.

The local launcher runs Vite on `http://localhost:5173` and the Rust API on port
3001. It enables an explicit development-only GitHub identity shortcut; this
shortcut is off unless `KNOCK_KNOCK_DEV_AUTH=1` is set.

```sh
./scripts/dev
```

Open `http://localhost:5173/mxcl/knock-knock`, choose the local development
sign-in, and activate the room. Check Rust formatting, run every backend test,
type-check the frontend, and build the production assets with:

```sh
./scripts/check
```

### Production configuration

Register a GitHub OAuth app with this callback URL:

```text
https://your-host.example/auth/github/callback
```

Normal sign-in requests no OAuth scope. Choosing **Open PR** separately requests
GitHub's `public_repo` scope, which is used only for the requested README branch,
commit, and pull request. Configure the process with:

| Variable | Purpose |
| --- | --- |
| `APP_BASE_URL` | Exact public origin, without a trailing slash |
| `GITHUB_CALLBACK_URL` | Optional registered OAuth callback URL; defaults to `APP_BASE_URL` plus `/auth/github/callback` |
| `DATABASE_URL` | SQLite URL, such as `sqlite:///data/knock-knock.db` |
| `GITHUB_CLIENT_ID` | GitHub OAuth app client ID |
| `GITHUB_CLIENT_SECRET` | GitHub OAuth app client secret |
| `KNOCK_KNOCK_TOKEN_KEY` | Base64-encoded 32-byte key used only for stored OAuth tokens |
| `BIND_ADDR` | Listen address; defaults to `127.0.0.1:3000` |
| `KNOCK_KNOCK_DEV_AUTH` | Development-only identity shortcut; set to `1` only for local development |

Generate the token key with `openssl rand -base64 32`. Never enable
`KNOCK_KNOCK_DEV_AUTH` in production.

The included `Dockerfile` builds the React assets and single Rust binary. On
Atlas, mount persistent storage at `/data`, set `DATABASE_URL` accordingly, and
leave the existing hourly Pangolin download responsible for backups.

Deploy an atomic ARM64 release to Atlas with:

```sh
./scripts/deploy-atlas
```

The script installs the systemd unit, preserves `/etc/knock-knock.env` and the
SQLite data directory, restarts the service, and checks its private health
endpoint. The public nginx cutover and certificate are separate operations so a
release can be verified before DNS changes.

## Success

The MVP succeeds when:

1. An affiliated GitHub user activates a public repository room.
2. Another GitHub user enters from a direct link or README badge.
3. Both users see each other's messages and presence without refreshing.
4. Affiliation pills accurately convey who speaks for the project.
5. Moderation works without destroying retained records.
6. Messages leave human view exactly 14 days after posting.
7. Room content is available by direct URL but not through global discovery or
   ordinary search-engine crawling.
