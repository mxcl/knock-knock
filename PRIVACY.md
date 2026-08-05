# Privacy and Retained Conversation Logs

> Product policy draft. This document describes the intended data behavior and
> must be reviewed before public launch.

## Plain-language summary

Knock Knock conversations are private, but they are not deleted when they
disappear from the product.

- During a repository's configured **human viewing window**, a conversation is
  visible only to its visitor and verified repository maintainers.
- After that window closes, visitors and maintainers cannot reopen, search, or
  export the conversation through Knock Knock.
- Knock Knock retains the complete conversation log privately and indefinitely by
  default.
- Retained logs exist solely to support future paid AI support agents and
  AI-generated FAQs. Those AI features are not part of the MVP.
- Conversation pages are authenticated and excluded from search engines. They are
  never intended to appear in Google or another public index.

The viewing window controls human access. It is not a data-deletion deadline.
The length of that window may be shown publicly, but the conversation itself is
never public.

## What the complete log contains

The retained log may include:

- Message text, Markdown, code blocks, edits, and timestamps
- Uploaded images and other supported attachments
- GitHub identities of visitors and participating maintainers
- Delivery, read, promotion, and conversation-state events
- Repository identity and the conversation's external promotion links
- Technical metadata needed to preserve ordering, integrity, and abuse controls

Authentication credentials and access tokens are operational secrets, not part
of the retained conversation corpus.

## Who can view a conversation

During the human viewing window, the product may show a conversation only to:

- The GitHub user who started it
- GitHub users whose current repository permission qualifies them as maintainers

After the window closes, the product provides no visitor, maintainer, staff, or
administrator interface for retrieving the transcript or its attachments. Direct
conversation and attachment links stop working.

Privileged infrastructure access is not a conversation-browsing feature. It must
be limited to exceptional security, reliability, or legally required operations,
use least-privilege controls, and produce an audit record.

## Why logs remain stored

Knock Knock retains complete logs for two future paid repository capabilities:

1. A repository support agent that can answer new questions using prior support
   knowledge.
2. Automatic creation of reusable FAQ entries from recurring questions and
   answers.

The retained corpus is purpose-limited. It is not intended for advertising, sale,
human conversation history, or general-purpose model training.

The MVP stores the corpus but does not run models against it. Before either paid
AI capability is enabled, this notice must identify the model providers, data sent
to them, provider retention and training controls, and safeguards against leaking
private source conversations.

## FAQ publication

Future AI-generated FAQs may be public, but retained source conversations remain
private. The FAQ pipeline must:

- Synthesize reusable guidance instead of publishing a transcript
- Remove GitHub identities, secrets, personal details, and unique private context
- Avoid verbatim excerpts that could identify a conversation
- Keep no public link back to a retained source log
- Define and test a publication policy before automatic publishing is enabled

## Search engines and public discovery

Conversation routes require authentication and participant-or-maintainer
authorization. Knock Knock also sends `X-Robots-Tag: noindex, nofollow, noarchive`
for conversation responses, excludes them from sitemaps, and disallows their paths
in `robots.txt` as defense in depth. Robots directives do not replace access
control.

Promoting a conversation to GitHub or exporting Markdown is an explicit human
action. The promoted copy is governed by the destination's visibility and privacy
rules; closing Knock Knock's viewing window does not remove that external copy.

## Storage and access controls

Retained logs and attachments must be encrypted in transit and at rest. After the
human viewing window closes, decryption access is reserved for a dedicated future
machine role. The product must not provide a human archive browser, cross-log
search, or transcript recovery tool.

Backups and replicas inherit the same purpose and access restrictions. Message
content must not be copied into analytics, metrics, traces, or ordinary operational
logs.

## Retention duration

Complete logs are retained indefinitely by default. Closing a conversation to
human access does not delete it. If Knock Knock later introduces a deletion
schedule or account-level deletion controls, this document and the pre-send notice
must be updated before that behavior changes.

## Notice shown before sending

The composer must show this disclosure, or materially equivalent language, before
the visitor sends the first message:

> This conversation is visible to you and repository maintainers for the stated
> viewing window. After that, humans cannot reopen it, but Knock Knock retains the
> complete log privately to power future paid AI support and FAQs. It is never
> indexed by search engines. [Learn more](/privacy)
