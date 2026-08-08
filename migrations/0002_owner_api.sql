CREATE TABLE api_keys (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    token_hash BLOB NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    last_polled_at INTEGER
);

CREATE TABLE room_views (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    room_id INTEGER NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    last_opened_at INTEGER NOT NULL,
    last_opened_message_id INTEGER NOT NULL,
    PRIMARY KEY(user_id, room_id)
);
