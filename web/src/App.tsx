import {
  ArrowDown,
  ArrowRight,
  Bot,
  Circle,
  Copy,
  ExternalLink,
  Fingerprint,
  KeyRound,
  LoaderCircle,
  LogOut,
  Radio,
  Send,
  Shield,
  Trash2,
} from "lucide-react";
import { FormEvent, useCallback, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";

type User = { id: number; login: string; avatarUrl: string };
type Repository = {
  owner: string;
  name: string;
  htmlUrl: string;
  description?: string;
};
type Relationship = {
  pill?: "owner" | "maintainer" | "collaborator";
  canManage: boolean;
};
type Room = {
  repository: Repository;
  active: boolean;
  currentUser: User;
  relationship: Relationship;
  canManage: boolean;
  requestIssueUrl?: string;
  retentionDays: number;
};
type Message = {
  id: number;
  author: User;
  markdown?: string;
  affiliation?: string;
  state: "visible" | "removed" | "hidden";
  createdAt: string;
  editedAt?: string;
  clientMessageId?: string;
  delivery?: "sending" | "failed";
};
type History = { messages: Message[]; nextCursor?: number };
type PublicConfig = { githubOAuth: boolean; devAuth: boolean };
type Presence = {
  count: number;
  affiliated: Array<{ id: number; login: string; affiliation: string }>;
};
type ApiKeyStatus = { exists: boolean; createdAt?: string };
type CreatedApiKey = { apiKey: string; createdAt: string };
type ApiError = Error & { status?: number };

const mutationHeaders = {
  "content-type": "application/json",
  "x-knock-knock": "1",
};

async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, init);
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    const error = new Error(
      body.error || `Request failed (${response.status})`,
    ) as ApiError;
    error.status = response.status;
    throw error;
  }
  return response.json();
}

function repositoryPath(): [string, string] | null {
  const parts = location.pathname.split("/").filter(Boolean);
  return parts.length === 2
    ? [decodeURIComponent(parts[0]), decodeURIComponent(parts[1])]
    : null;
}

export function App() {
  const coordinates = repositoryPath();
  return (
    <div className="app-shell">
      <a className="skip-link" href="#main">
        Skip to conversation
      </a>
      {coordinates ? (
        <RoomPage owner={coordinates[0]} repo={coordinates[1]} />
      ) : (
        <Doorway />
      )}
    </div>
  );
}

function Doorway() {
  const [value, setValue] = useState("");
  const [error, setError] = useState("");

  function enter(event: FormEvent) {
    event.preventDefault();
    const cleaned = value
      .trim()
      .replace(/^https?:\/\/github\.com\//, "")
      .replace(/\.git$/, "")
      .replace(/^\/+|\/+$/g, "");
    const [owner, repo, extra] = cleaned.split("/");
    if (!owner || !repo || extra) {
      setError("Use owner/repository or paste a GitHub repository URL.");
      return;
    }
    location.assign(
      `/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`,
    );
  }

  return (
    <main id="main" className="doorway">
      <header className="brand">
        <span className="brand-mark" aria-hidden="true">
          K
        </span>
        <span>Knock Knock</span>
      </header>
      <section className="doorway-hero">
        <div className="doorway-copy">
          <p className="eyebrow">A quieter corner of open source</p>
          <h1>The room beside the repository.</h1>
          <p className="lede">
            Drop into a live conversation around any public GitHub project.
            Everyone shows ID at the door. What’s said stays visible for
            fourteen days.
          </p>
        </div>
        <form className="repo-form" onSubmit={enter}>
          <p className="form-number" aria-hidden="true">
            01
          </p>
          <label htmlFor="repository">Which repository?</label>
          <div className="repo-input-row">
            <span aria-hidden="true">github.com/</span>
            <input
              id="repository"
              value={value}
              onChange={(event) => setValue(event.target.value)}
              placeholder="owner/repository"
              autoComplete="off"
            />
            <Button size="icon" aria-label="Enter repository">
              <ArrowRight />
            </Button>
          </div>
          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}
          <p className="repo-form-hint">
            Paste a GitHub URL or type owner/repository
          </p>
        </form>
      </section>
      <footer className="doorway-notes">
        <span>GitHub ID required</span>
        <span>No anonymous spectators</span>
        <span>14-day public window</span>
        <span>Complete logs retained for future AI features</span>
      </footer>
    </main>
  );
}

function RoomPage({ owner, repo }: { owner: string; repo: string }) {
  const path = `/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`;
  const [room, setRoom] = useState<Room>();
  const [messages, setMessages] = useState<Message[]>([]);
  const [nextCursor, setNextCursor] = useState<number>();
  const [presence, setPresence] = useState<Presence>({
    count: 0,
    affiliated: [],
  });
  const [loading, setLoading] = useState(true);
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState("");

  const loadRoom = useCallback(async () => {
    setLoading(true);
    try {
      const value = await api<Room>(
        `/api/rooms/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`,
      );
      setRoom(value);
      setUnauthorized(false);
      setError("");
    } catch (caught) {
      const apiError = caught as ApiError;
      if (apiError.status === 401) setUnauthorized(true);
      else setError(apiError.message);
    } finally {
      setLoading(false);
    }
  }, [owner, repo]);

  const loadMessages = useCallback(
    async (before?: number) => {
      const query = before ? `?before=${before}` : "";
      const history = await api<History>(
        `/api/rooms/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/messages${query}`,
      );
      const chronological = [...history.messages].reverse();
      setMessages((current) =>
        before ? unique([...chronological, ...current]) : chronological,
      );
      setNextCursor(history.nextCursor);
    },
    [owner, repo],
  );

  useEffect(() => {
    void loadRoom();
  }, [loadRoom]);
  useEffect(() => {
    if (room?.active)
      void loadMessages().catch((caught: Error) => setError(caught.message));
  }, [room?.active, loadMessages]);

  useEffect(() => {
    if (!room?.active) return;
    let socket: WebSocket | undefined;
    let retry: number | undefined;
    let heartbeat: number | undefined;
    let closed = false;
    const connect = () => {
      const protocol = location.protocol === "https:" ? "wss:" : "ws:";
      socket = new WebSocket(
        `${protocol}//${location.host}/api/rooms/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/stream`,
      );
      socket.onopen = () => {
        heartbeat = window.setInterval(
          () => socket?.send('{"type":"heartbeat"}'),
          20_000,
        );
      };
      socket.onmessage = (event) => {
        const update = JSON.parse(event.data);
        if (update.type === "presence")
          setPresence({
            count: update.count,
            affiliated: update.affiliated || [],
          });
        if (
          update.type === "message.created" ||
          update.type === "message.updated"
        )
          setMessages((current) =>
            unique([
              ...current.filter((message) => message.id !== update.message.id),
              update.message,
            ]).sort((a, b) => a.id - b.id),
          );
        if (update.type === "room.deactivated") void loadRoom();
      };
      socket.onclose = () => {
        if (heartbeat) clearInterval(heartbeat);
        if (!closed) retry = window.setTimeout(connect, 1_500);
      };
    };
    connect();
    return () => {
      closed = true;
      if (retry) clearTimeout(retry);
      if (heartbeat) clearInterval(heartbeat);
      socket?.close();
    };
  }, [room?.active, owner, repo, loadRoom]);

  async function toggleRoom(active: boolean) {
    setError("");
    try {
      await api(
        `/api/rooms/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/${active ? "activate" : "deactivate"}`,
        { method: "POST", headers: mutationHeaders, body: "{}" },
      );
      await loadRoom();
      if (!active) setMessages([]);
    } catch (caught) {
      setError((caught as Error).message);
    }
  }

  async function retryMessage(pending: Message) {
    if (!pending.clientMessageId || !pending.markdown) return;
    setMessages((current) =>
      current.map((message) =>
        message.id === pending.id
          ? { ...message, delivery: "sending" }
          : message,
      ),
    );
    try {
      const message = await api<Message>(
        `/api/rooms/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/messages`,
        {
          method: "POST",
          headers: mutationHeaders,
          body: JSON.stringify({
            clientMessageId: pending.clientMessageId,
            markdown: pending.markdown,
          }),
        },
      );
      setMessages((current) =>
        unique([
          ...current.filter((item) => item.id !== pending.id),
          message,
        ]).sort((a, b) => a.id - b.id),
      );
    } catch {
      setMessages((current) =>
        current.map((message) =>
          message.id === pending.id
            ? { ...message, delivery: "failed" }
            : message,
        ),
      );
    }
  }

  if (loading)
    return (
      <Centered>
        <LoaderCircle className="spinner" />
        <p>Resolving repository…</p>
      </Centered>
    );
  if (unauthorized) return <SignIn owner={owner} repo={repo} />;
  if (!room)
    return (
      <Centered>
        <p className="eyebrow">NO SIGNAL</p>
        <h1>{error || "Repository unavailable"}</h1>
        <Button asChild>
          <a href="/">Try another repository</a>
        </Button>
      </Centered>
    );
  if (!room.active)
    return (
      <Unclaimed
        room={room}
        error={error}
        onActivate={() => toggleRoom(true)}
      />
    );

  return (
    <main id="main" className="room-layout">
      <RoomHeader
        room={room}
        presence={presence}
        onDeactivate={() => toggleRoom(false)}
      />
      {error && (
        <div className="error-banner" role="alert">
          {error}
          <button onClick={() => setError("")}>Dismiss</button>
        </div>
      )}
      <section
        className="timeline"
        aria-label="Conversation"
        aria-live="polite"
      >
        <div className="timeline-label" aria-hidden="true">
          <span>Conversation</span>
          <span>{messages.length.toString().padStart(2, "0")}</span>
        </div>
        {nextCursor && (
          <Button
            variant="secondary"
            size="sm"
            className="load-older"
            onClick={() => void loadMessages(nextCursor)}
          >
            <ArrowDown /> Load older messages
          </Button>
        )}
        {messages.length === 0 && (
          <div className="empty-room">
            <Radio />
            <h2>Quiet in here.</h2>
            <p>You have the floor.</p>
          </div>
        )}
        {messages.map((message) => (
          <MessageCard
            key={`${message.id}-${message.clientMessageId || ""}`}
            message={message}
            owner={owner}
            repo={repo}
            currentUser={room.currentUser}
            canManage={room.canManage}
            onRetry={
              message.delivery === "failed"
                ? () => retryMessage(message)
                : undefined
            }
            onChange={(changed) =>
              setMessages((current) =>
                unique([
                  ...current.filter((item) => item.id !== changed.id),
                  changed,
                ]).sort((a, b) => a.id - b.id),
              )
            }
          />
        ))}
      </section>
      <Composer
        owner={owner}
        repo={repo}
        user={room.currentUser}
        affiliation={room.relationship.pill}
        onOptimistic={(message) =>
          setMessages((current) => [...current, message])
        }
        onSettled={(pendingId, message) =>
          setMessages((current) =>
            unique([
              ...current.filter((item) => item.id !== pendingId),
              message,
            ]).sort((a, b) => a.id - b.id),
          )
        }
        onFailed={(pendingId) =>
          setMessages((current) =>
            current.map((message) =>
              message.id === pendingId
                ? { ...message, delivery: "failed" }
                : message,
            ),
          )
        }
      />
    </main>
  );
}

function SignIn({ owner, repo }: { owner: string; repo: string }) {
  const returnTo = `/${owner}/${repo}`;
  const [config, setConfig] = useState<PublicConfig>();
  useEffect(() => {
    void api<PublicConfig>("/api/config").then(setConfig);
  }, []);
  return (
    <Centered>
      <div className="knocker">
        <span />
        <span />
        <span />
      </div>
      <p className="eyebrow">ID CHECK · GITHUB</p>
      <h1>Show ID at the door.</h1>
      <p className="center-copy">
        Anyone with a GitHub account can enter. There are no anonymous
        spectators.
      </p>
      {config?.githubOAuth && (
        <Button asChild>
          <a href={`/auth/github?return_to=${encodeURIComponent(returnTo)}`}>
            <Fingerprint /> Continue with GitHub
          </a>
        </Button>
      )}
      {config?.devAuth && (
        <Button variant="secondary" asChild>
          <a href={`/auth/dev?return_to=${encodeURIComponent(returnTo)}`}>
            Local development sign-in
          </a>
        </Button>
      )}
      {config && !config.githubOAuth && !config.devAuth && (
        <p className="form-error">Authentication has not been configured.</p>
      )}
    </Centered>
  );
}

function Unclaimed({
  room,
  error,
  onActivate,
}: {
  room: Room;
  error: string;
  onActivate: () => void;
}) {
  return (
    <main id="main" className="unclaimed">
      <TopBar user={room.currentUser} canManage={room.canManage} />
      <section className="claim-panel">
        <p className="eyebrow">ROOM STATUS · UNCLAIMED</p>
        <RepositoryTitle repository={room.repository} />
        <div className="claim-rule">
          <span />
        </div>
        <h2>Nobody has opened the door yet.</h2>
        <p>
          The room starts only when a current repository admin or maintainer
          activates it. No messages are visible before then.
        </p>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        <div className="button-row">
          {room.canManage && (
            <Button onClick={onActivate}>
              Activate room <ArrowRight />
            </Button>
          )}
          {!room.canManage && room.requestIssueUrl && (
            <Button asChild>
              <a href={room.requestIssueUrl} target="_blank" rel="noreferrer">
                Ask in a GitHub issue <ExternalLink />
              </a>
            </Button>
          )}
          {!room.canManage && !room.requestIssueUrl && (
            <p className="small-note">
              GitHub Issues are disabled. Ask a maintainer through the project’s
              usual contact path.
            </p>
          )}
        </div>
      </section>
    </main>
  );
}

function RoomHeader({
  room,
  presence,
  onDeactivate,
}: {
  room: Room;
  presence: Presence;
  onDeactivate: () => void;
}) {
  const [copied, setCopied] = useState(false);
  async function copyBadge() {
    const { owner, name } = room.repository;
    await navigator.clipboard.writeText(
      `[![Knock Knock](${location.origin}/badge.svg)](${location.origin}/${encodeURIComponent(owner)}/${encodeURIComponent(name)})`,
    );
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1_500);
  }
  return (
    <header className="room-header">
      <TopBar user={room.currentUser} canManage={room.canManage} />
      <div className="room-heading">
        <RepositoryTitle repository={room.repository} />
        <div className="room-meta">
          <span className="live">
            <Circle /> LIVE
          </span>
          <span>
            {presence.count} {presence.count === 1 ? "person" : "people"} here
          </span>
          {presence.affiliated.map((user) => (
            <span className="present-affiliate" key={user.id}>
              @{user.login} · {user.affiliation}
            </span>
          ))}
          <span>visible for 14 days</span>
          <Button variant="ghost" size="sm" onClick={() => void copyBadge()}>
            {copied ? "Badge copied" : "Copy README badge"}
          </Button>
          {room.canManage && (
            <Button variant="ghost" size="sm" onClick={onDeactivate}>
              Deactivate
            </Button>
          )}
        </div>
      </div>
    </header>
  );
}

function TopBar({ user, canManage }: { user: User; canManage: boolean }) {
  async function logout() {
    await fetch("/auth/logout", { method: "POST", headers: mutationHeaders });
    location.assign("/");
  }
  return (
    <div className="topbar">
      <a className="brand" href="/">
        <span className="brand-mark" aria-hidden="true">
          K
        </span>
        <span>Knock Knock</span>
      </a>
      <div className="identity">
        {canManage && <ApiKeyControl />}
        <img src={user.avatarUrl} alt="" />
        <span>@{user.login}</span>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => void logout()}
          aria-label="Sign out"
        >
          <LogOut />
        </Button>
      </div>
    </div>
  );
}

function ApiKeyControl() {
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState<ApiKeyStatus>();
  const [created, setCreated] = useState<CreatedApiKey>();
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [promptCopied, setPromptCopied] = useState(false);
  const [error, setError] = useState("");

  async function toggle() {
    const next = !open;
    setOpen(next);
    if (!next) {
      setCreated(undefined);
      setCopied(false);
      setPromptCopied(false);
      return;
    }
    if (!status) {
      try {
        setStatus(await api<ApiKeyStatus>("/api/account/api-key"));
      } catch (caught) {
        setError((caught as Error).message);
      }
    }
  }

  async function create() {
    if (
      status?.exists &&
      !confirm("Rotate your API key? The current key will stop working.")
    )
      return;
    setBusy(true);
    setError("");
    try {
      const value = await api<CreatedApiKey>("/api/account/api-key", {
        method: "POST",
        headers: mutationHeaders,
        body: "{}",
      });
      setCreated(value);
      setStatus({ exists: true, createdAt: value.createdAt });
    } catch (caught) {
      setError((caught as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function copy() {
    if (!created) return;
    await navigator.clipboard.writeText(created.apiKey);
    setCopied(true);
  }

  async function copyAgentPrompt() {
    if (!created) return;
    await navigator.clipboard.writeText(`Integrate the Knock Knock owner polling API into this project.

Requirements:
- Send GET ${location.origin}/api/v1/rooms/new-messages with this header:
  Authorization: Bearer ${created.apiKey}
- Keep the API key in the project's secret or environment configuration. Never commit it, send it to the client, or write it to logs.
- Poll no more than once every 60 seconds.
- On HTTP 429, honor the Retry-After response header before trying again.
- For other non-success responses, use the project's existing error handling and retry conventions.
- The JSON response has this shape:
  { "rooms": [{ "owner": "string", "repository": "string", "url": "string", "newMessageCount": 0, "latestMessageAt": "ISO-8601 string", "lastOpenedAt": "ISO-8601 string" }], "polledAt": "ISO-8601 string" }
- Surface rooms with new messages using the patterns already established in this codebase. Opening a room in Knock Knock clears it from subsequent results; polling does not.
- Add focused tests for response parsing, the 60-second limit, and 429 handling.

First inspect the repository and briefly state your implementation plan, then implement and verify the integration.`);
    setPromptCopied(true);
  }

  return (
    <div className="api-key-control">
      <Button
        variant="ghost"
        size="sm"
        aria-expanded={open}
        onClick={() => void toggle()}
      >
        <KeyRound /> API key
      </Button>
      {open && (
        <section className="api-key-panel" aria-label="Owner API key">
          <p className="eyebrow">Owner API</p>
          <h2>Room updates, as JSON.</h2>
          <p>
            Poll once a minute to find managed rooms with messages since you
            last opened them.
          </p>
          {created ? (
            <>
              <code>{created.apiKey}</code>
              <div className="api-key-actions">
                <Button size="sm" onClick={() => void copy()}>
                  <Copy /> {copied ? "Copied" : "Copy key"}
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => void copyAgentPrompt()}
                >
                  <Bot /> {promptCopied ? "Prompt copied" : "Copy agent prompt"}
                </Button>
              </div>
              <small>Save it now. This key will not be shown again.</small>
            </>
          ) : status ? (
            <Button size="sm" disabled={busy} onClick={() => void create()}>
              <KeyRound /> {status.exists ? "Rotate API key" : "Create API key"}
            </Button>
          ) : (
            <span className="api-key-loading">Checking…</span>
          )}
          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}
          <code className="api-key-endpoint">
            GET /api/v1/rooms/new-messages
          </code>
        </section>
      )}
    </div>
  );
}

function RepositoryTitle({ repository }: { repository: Repository }) {
  return (
    <div>
      <p className="repo-owner">{repository.owner} /</p>
      <h1 className="repo-name">
        <a href={repository.htmlUrl} target="_blank" rel="noreferrer">
          {repository.name}
          <ExternalLink />
        </a>
      </h1>
      {repository.description && (
        <p className="repo-description">{repository.description}</p>
      )}
    </div>
  );
}

function MessageCard({
  message,
  owner,
  repo,
  currentUser,
  canManage,
  onChange,
  onRetry,
}: {
  message: Message;
  owner: string;
  repo: string;
  currentUser: User;
  canManage: boolean;
  onChange: (message: Message) => void;
  onRetry?: () => void;
}) {
  const mine = message.author.id === currentUser.id;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(message.markdown || "");
  const [busy, setBusy] = useState(false);
  async function mutate(method: "PATCH" | "DELETE", body?: object) {
    setBusy(true);
    try {
      const changed = await api<Message>(`/api/messages/${message.id}`, {
        method,
        headers: mutationHeaders,
        body: body ? JSON.stringify(body) : undefined,
      });
      onChange(changed);
      setEditing(false);
    } finally {
      setBusy(false);
    }
  }
  async function report() {
    const reason = prompt("Briefly describe the problem:");
    if (reason)
      await api(`/api/messages/${message.id}/reports`, {
        method: "POST",
        headers: mutationHeaders,
        body: JSON.stringify({ reason }),
      });
  }
  async function hide() {
    const reason = prompt("Why should this message be hidden?");
    if (reason)
      onChange(
        await api<Message>(`/api/messages/${message.id}/hide`, {
          method: "POST",
          headers: mutationHeaders,
          body: JSON.stringify({ reason }),
        }),
      );
  }
  async function mute() {
    const reason = prompt(
      `Why should @${message.author.login} be muted in this room?`,
    );
    if (reason)
      await api(
        `/api/rooms/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/mutes`,
        {
          method: "POST",
          headers: mutationHeaders,
          body: JSON.stringify({ userId: message.author.id, reason }),
        },
      );
  }
  return (
    <article
      className={`message ${mine ? "message--mine" : ""} ${message.delivery ? `message--${message.delivery}` : ""}`}
    >
      <img
        className="avatar"
        src={message.author.avatarUrl}
        alt=""
        loading="lazy"
      />
      <div className="message-body">
        <header>
          <strong>@{message.author.login}</strong>
          {message.affiliation && <Badge>{message.affiliation}</Badge>}
          <time dateTime={message.createdAt}>
            {formatTime(message.createdAt)}
          </time>
          {message.editedAt && <span>edited</span>}
          {message.delivery && <span>{message.delivery}</span>}
        </header>
        {message.state !== "visible" ? (
          <p className="tombstone">
            {message.state === "removed"
              ? "Message removed by its author."
              : "Message hidden by a project maintainer."}
          </p>
        ) : editing ? (
          <div className="edit-box">
            <textarea
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              maxLength={8000}
            />
            <div className="button-row">
              <Button
                size="sm"
                disabled={busy}
                onClick={() => void mutate("PATCH", { markdown: draft })}
              >
                Save
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setEditing(false)}
              >
                Cancel
              </Button>
            </div>
          </div>
        ) : (
          <Markdown markdown={message.markdown || ""} />
        )}
        {message.delivery === "failed" && onRetry && (
          <Button size="sm" variant="secondary" onClick={onRetry}>
            Retry delivery
          </Button>
        )}
        {message.state === "visible" && !message.delivery && (
          <footer className="message-actions">
            {mine && (
              <>
                <button onClick={() => setEditing(true)}>Edit</button>
                <button onClick={() => void mutate("DELETE")}>
                  <Trash2 /> Remove
                </button>
              </>
            )}
            {!mine && <button onClick={() => void report()}>Report</button>}
            {canManage && !mine && (
              <>
                <button onClick={() => void hide()}>
                  <Shield /> Hide
                </button>
                <button onClick={() => void mute()}>Mute user</button>
              </>
            )}
          </footer>
        )}
      </div>
    </article>
  );
}

function Markdown({ markdown }: { markdown: string }) {
  return (
    <div className="markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        disallowedElements={[
          "table",
          "thead",
          "tbody",
          "tr",
          "th",
          "td",
          "input",
          "img",
          "del",
        ]}
        unwrapDisallowed
        components={{
          a: ({ ...props }) => (
            <a {...props} target="_blank" rel="noreferrer nofollow" />
          ),
        }}
      >
        {markdown}
      </ReactMarkdown>
    </div>
  );
}

function Composer({
  owner,
  repo,
  user,
  affiliation,
  onOptimistic,
  onSettled,
  onFailed,
}: {
  owner: string;
  repo: string;
  user: User;
  affiliation?: string;
  onOptimistic: (message: Message) => void;
  onSettled: (pendingId: number, message: Message) => void;
  onFailed: (pendingId: number) => void;
}) {
  const [draft, setDraft] = useState("");
  const [notice, setNotice] = useState(false);
  const textarea = useRef<HTMLTextAreaElement>(null);
  const remaining = 8000 - [...draft].length;
  async function submit(event: FormEvent) {
    event.preventDefault();
    const markdown = draft.trim();
    if (!markdown) return;
    const clientMessageId = crypto.randomUUID();
    const pendingId = -Date.now();
    const optimistic: Message = {
      id: pendingId,
      author: user,
      markdown,
      affiliation,
      state: "visible",
      createdAt: new Date().toISOString(),
      clientMessageId,
      delivery: "sending",
    };
    onOptimistic(optimistic);
    setDraft("");
    try {
      const message = await api<Message>(
        `/api/rooms/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/messages`,
        {
          method: "POST",
          headers: mutationHeaders,
          body: JSON.stringify({ clientMessageId, markdown }),
        },
      );
      onSettled(pendingId, message);
    } catch {
      onFailed(pendingId);
    }
  }
  function keyDown(event: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey))
      textarea.current?.form?.requestSubmit();
  }
  return (
    <form className="composer" onSubmit={submit}>
      <div className="composer-notice">
        <button
          type="button"
          aria-expanded={notice}
          onClick={() => setNotice(!notice)}
        >
          Before you speak <span>{notice ? "−" : "+"}</span>
        </button>
        {notice && (
          <p>
            Messages are shown to signed-in GitHub users for 14 days. Complete
            logs are retained indefinitely for future paid AI support and
            synthesized FAQs. They are not exposed to Google or routine human
            browsing after the window.
          </p>
        )}
      </div>
      <div className="composer-box">
        <textarea
          ref={textarea}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={keyDown}
          maxLength={8000}
          placeholder="Say something useful…"
          aria-label="Message"
        />
        <div className="composer-tools">
          <span className={remaining < 500 ? "limit-warning" : ""}>
            {remaining.toLocaleString()}
          </span>
          <span>Markdown · ⌘ Enter to send</span>
          <Button type="submit" disabled={!draft.trim()}>
            Send <Send />
          </Button>
        </div>
      </div>
    </form>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <main id="main" className="centered">
      <a className="brand" href="/">
        <span className="brand-mark" aria-hidden="true">
          K
        </span>
        <span>Knock Knock</span>
      </a>
      <div>{children}</div>
    </main>
  );
}

function unique(messages: Message[]) {
  return [
    ...new Map(messages.map((message) => [message.id, message])).values(),
  ];
}
function formatTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(value));
}
