PRAGMA foreign_keys = ON;

CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    github_id INTEGER NOT NULL UNIQUE,
    login TEXT NOT NULL,
    avatar_url TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE oauth_credentials (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    encrypted_token BLOB NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE oauth_states (
    state_hash BLOB PRIMARY KEY,
    code_verifier TEXT NOT NULL,
    return_to TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE sessions (
    token_hash BLOB PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    last_activity_at INTEGER NOT NULL
);

CREATE TABLE repositories (
    id INTEGER PRIMARY KEY,
    github_id INTEGER NOT NULL UNIQUE,
    owner TEXT NOT NULL COLLATE NOCASE,
    name TEXT NOT NULL COLLATE NOCASE,
    html_url TEXT NOT NULL,
    description TEXT,
    has_issues INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(owner, name)
);

CREATE TABLE rooms (
    id INTEGER PRIMARY KEY,
    repository_id INTEGER NOT NULL UNIQUE REFERENCES repositories(id),
    active INTEGER NOT NULL DEFAULT 0,
    visible_since INTEGER,
    activated_by INTEGER REFERENCES users(id),
    activated_at INTEGER,
    deactivated_by INTEGER REFERENCES users(id),
    deactivated_at INTEGER
);

CREATE TABLE relationship_cache (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    repository_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    relationship TEXT NOT NULL,
    can_manage INTEGER NOT NULL,
    verified_at INTEGER NOT NULL,
    PRIMARY KEY(user_id, repository_id)
);

CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    author_id INTEGER NOT NULL REFERENCES users(id),
    client_message_uuid TEXT NOT NULL,
    markdown TEXT NOT NULL,
    affiliation TEXT,
    state TEXT NOT NULL DEFAULT 'visible',
    created_at INTEGER NOT NULL,
    edited_at INTEGER,
    removed_at INTEGER,
    UNIQUE(author_id, client_message_uuid)
);

CREATE INDEX visible_messages ON messages(room_id, id DESC, created_at)
    WHERE state IN ('visible', 'removed', 'hidden');

CREATE TABLE message_revisions (
    id INTEGER PRIMARY KEY,
    message_id INTEGER NOT NULL REFERENCES messages(id),
    revision INTEGER NOT NULL,
    markdown TEXT NOT NULL,
    editor_id INTEGER NOT NULL REFERENCES users(id),
    created_at INTEGER NOT NULL,
    UNIQUE(message_id, revision)
);

CREATE TABLE reports (
    id INTEGER PRIMARY KEY,
    reporter_id INTEGER NOT NULL REFERENCES users(id),
    message_id INTEGER NOT NULL REFERENCES messages(id),
    reason TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'open',
    created_at INTEGER NOT NULL,
    UNIQUE(reporter_id, message_id)
);

CREATE TABLE moderation_actions (
    id INTEGER PRIMARY KEY,
    actor_id INTEGER NOT NULL REFERENCES users(id),
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    message_id INTEGER REFERENCES messages(id),
    target_user_id INTEGER REFERENCES users(id),
    action TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE room_mutes (
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    actor_id INTEGER NOT NULL REFERENCES users(id),
    reason TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(room_id, user_id)
);

CREATE TABLE platform_blocks (
    user_id INTEGER PRIMARY KEY REFERENCES users(id),
    actor_id INTEGER NOT NULL REFERENCES users(id),
    reason TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
