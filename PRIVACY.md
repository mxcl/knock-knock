# Privacy and Retained Logs

> Product policy draft. This describes intended behavior and must be reviewed
> before public launch.

## Plain-language summary

Knock Knock is an open room for GitHub users, not a private conversation product.

- Anyone may read an active public repository room by direct URL.
- GitHub authentication is required to post or take moderation actions.
- Each message remains publicly visible for 14 days.
- After 14 days, the product stops returning the message to human-facing views.
- Knock Knock retains the complete plaintext log indefinitely by default.
- Retained logs may later power paid AI support and AI-generated FAQs. The MVP
  performs no AI processing.
- Rooms are excluded from search engines and from global discovery within Knock
  Knock.

The 14-day window controls ordinary human visibility. It is not a deletion or
confidentiality guarantee.

## The room is public to read

Anyone with an active room's direct URL may read it during the viewing window.
GitHub authentication acts like a bouncer checking the identity of people who
post; it does not make the room private to repository maintainers or existing
participants.

Knock Knock snapshots an author's repository relationship when a message is
posted and may display an `owner`, `maintainer`, or `collaborator` pill. Users
without a known relationship receive no pill.

## What remains stored

The retained record may include:

- Message Markdown, timestamps, and sanitized link destinations
- Original message bodies and every edited revision
- Messages removed by their authors or hidden by moderators
- GitHub user identity and repository-affiliation snapshots
- Reports, mutes, moderation actions, and room activation history
- Technical metadata needed for ordering, integrity, rate limits, and abuse
  controls

The MVP does not accept file or image uploads and does not retain link previews.
Presence, socket connections, typing, read position, and other transient realtime
signals are not retained as conversation history.

GitHub OAuth credentials and session secrets are operational credentials, not part
of the retained room corpus. They require separate security controls. Normal
sign-in requests no OAuth scope. If a repository maintainer chooses **Open PR**
for a README badge, Knock Knock requests GitHub's `public_repo` scope and uses it
to read that README, create a branch and commit, and open the requested pull
request. This scope can permit writes to other public repositories available to
that user, but Knock Knock does not exercise that write access without an explicit
repository action.

## Human visibility after 14 days

Human-facing message queries enforce a rolling cutoff based on each message's
posting timestamp. After the cutoff:

- The message no longer appears in the room.
- Direct product routes do not provide a transcript recovery path.
- A removed message's tombstone also leaves the visible window.
- Reactivating a deactivated room does not restore old messages.

Retained data is not cryptographically isolated from Knock Knock's operators, and
Knock Knock does not promise that no authorized operator could inspect storage.
Operational access should be limited to legitimate security, reliability, abuse,
or legal needs and should not become a general archive-browsing feature.

## Why logs remain stored

The retained room corpus exists for two intended future paid capabilities:

1. A repository support agent that can answer new questions using prior room
   knowledge.
2. Automatic generation of reusable FAQ entries from recurring questions and
   answers.

The corpus is not intended for advertising, sale, user profiling, or unrelated
general-purpose model training.

Before paid AI is enabled, this notice must identify model providers, what data is
sent to them, provider retention and training controls, and safeguards against
publishing identifying source material.

## Future FAQ publication

Future AI-generated FAQs may be public, but source room logs remain outside
human-facing history. An FAQ pipeline must:

- Synthesize guidance instead of publishing a transcript
- Remove GitHub identities, secrets, personal details, and unique context
- Avoid verbatim excerpts that could identify a source exchange
- Publish no link back to a retained source record
- Define and test its publication policy before automatic publication

## Search engines and discovery

Room content is available by direct URL. Conversation responses also send
`X-Robots-Tag: noindex, nofollow, noarchive`, are excluded from sitemaps, and are
disallowed in `robots.txt` as defense in depth.

Knock Knock provides no global room directory or room-content search in the MVP.
Rooms are reached through their exact `owner/repo` URL, README badge, or repository
URL entry.

## Retention and backups

Complete room logs are retained indefinitely by default in the application's
SQLite database. Existing infrastructure downloads an hourly database backup to
Pangolin. Backups inherit the same intended use and access expectations as the
primary database.

Knock Knock does not promise a stronger durability level for the free MVP.

## Notice shown before posting

The composer must show this disclosure, or materially equivalent language, before
a user sends their first message:

> Any GitHub user can read this room's last 14 days. Messages then leave the room,
> but Knock Knock keeps complete logs indefinitely for future paid AI support and
> FAQs. [Learn more](/privacy)
