use std::{
    collections::{HashMap, VecDeque},
    env,
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{
        ConnectInfo, Path, Query, State, WebSocketUpgrade,
        ws::{Message as WsMessage, WebSocket},
    },
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, patch, post},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use pulldown_cmark::{Event, Parser, Tag};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{
    Row, SqlitePool,
    migrate::MigrateDatabase,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use thiserror::Error;
use tokio::sync::{Mutex, broadcast};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing::{info, warn};
use url::Url;

const SESSION_SECONDS: i64 = 30 * 24 * 60 * 60;
const VISIBLE_SECONDS: i64 = 14 * 24 * 60 * 60;
const OWNER_API_POLL_SECONDS: i64 = 60;
const MAX_MESSAGE_CHARS: usize = 8_000;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    config: Arc<Config>,
    http: Client,
    events: Arc<DashMap<i64, broadcast::Sender<String>>>,
    presence: Arc<Mutex<HashMap<i64, HashMap<i64, PresenceUser>>>>,
    rate_limits: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

struct Config {
    base_url: String,
    github_client_id: Option<String>,
    github_client_secret: Option<String>,
    token_key: Option<[u8; 32]>,
    dev_auth: bool,
}

#[derive(Debug, Error)]
enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("you do not have permission to do that")]
    Forbidden,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("posting too quickly; please wait a moment")]
    RateLimited,
    #[error("polling is limited to once per minute")]
    PollingTooQuickly(i64),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let retry_after = match &self {
            Self::PollingTooQuickly(seconds) => Some(*seconds),
            _ => None,
        };
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::Conflict(message) => (StatusCode::CONFLICT, message),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            Self::PollingTooQuickly(_) => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            Self::Internal(error) => {
                warn!(?error, "request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".into(),
                )
            }
        };
        let mut response = (status, Json(json!({ "error": message }))).into_response();
        if let Some(seconds) = retry_after {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_str(&seconds.to_string()).expect("valid retry delay"),
            );
        }
        response
    }
}

type Result<T> = std::result::Result<T, AppError>;

#[derive(Clone)]
struct AuthUser {
    id: i64,
    github_id: i64,
    login: String,
    avatar_url: String,
    token: Option<String>,
}

struct PresenceUser {
    sockets: usize,
    github_id: i64,
    login: String,
    affiliation: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PublicUser {
    id: i64,
    login: String,
    avatar_url: String,
}

impl From<&AuthUser> for PublicUser {
    fn from(user: &AuthUser) -> Self {
        Self {
            id: user.github_id,
            login: user.login.clone(),
            avatar_url: user.avatar_url.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GithubUser {
    id: i64,
    login: String,
    avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct GithubOwner {
    login: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Default, Deserialize)]
struct GithubPermissions {
    #[serde(default)]
    admin: bool,
    #[serde(default)]
    maintain: bool,
    #[serde(default)]
    push: bool,
    #[serde(default)]
    triage: bool,
    #[serde(default, rename = "pull")]
    _pull: bool,
}

#[derive(Debug, Deserialize)]
struct GithubRepository {
    id: i64,
    name: String,
    private: bool,
    html_url: String,
    description: Option<String>,
    has_issues: bool,
    owner: GithubOwner,
    #[serde(default)]
    permissions: GithubPermissions,
}

#[derive(Clone)]
struct Repository {
    id: i64,
    owner: String,
    name: String,
    html_url: String,
    description: Option<String>,
    has_issues: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Relationship {
    pill: Option<String>,
    can_manage: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RoomView {
    repository: RepositoryView,
    active: bool,
    current_user: PublicUser,
    relationship: Relationship,
    can_manage: bool,
    request_issue_url: Option<String>,
    retention_days: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryView {
    owner: String,
    name: String,
    html_url: String,
    description: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MessageView {
    id: i64,
    author: PublicUser,
    markdown: Option<String>,
    affiliation: Option<String>,
    state: String,
    created_at: String,
    edited_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostMessage {
    client_message_id: String,
    markdown: String,
}

#[derive(Deserialize)]
struct EditMessage {
    markdown: String,
}

#[derive(Deserialize)]
struct Reason {
    reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MuteRequest {
    user_id: i64,
    reason: String,
}

#[derive(Deserialize)]
struct HistoryQuery {
    before: Option<i64>,
    limit: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnerRoomUpdate {
    owner: String,
    repository: String,
    url: String,
    new_message_count: i64,
    latest_message_at: String,
    last_opened_at: String,
}

#[derive(Deserialize)]
struct LoginQuery {
    return_to: Option<String>,
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct DevLoginQuery {
    login: Option<String>,
    return_to: Option<String>,
}

#[derive(Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "knock_knock=info,tower_http=info".into()),
        )
        .init();

    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://knock-knock.db".into());
    if !sqlx::Sqlite::database_exists(&database_url)
        .await
        .unwrap_or(false)
    {
        sqlx::Sqlite::create_database(&database_url).await?;
    }
    let options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let db = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    sqlx::migrate!().run(&db).await?;

    let base_url = env::var("APP_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into());
    let github_client_id = env::var("GITHUB_CLIENT_ID").ok();
    let github_client_secret = env::var("GITHUB_CLIENT_SECRET").ok();
    let token_key = env::var("KNOCK_KNOCK_TOKEN_KEY")
        .ok()
        .map(parse_key)
        .transpose()?;
    if github_client_id.is_some() && (github_client_secret.is_none() || token_key.is_none()) {
        anyhow::bail!(
            "GITHUB_CLIENT_SECRET and KNOCK_KNOCK_TOKEN_KEY are required with GITHUB_CLIENT_ID"
        );
    }
    let config = Config {
        base_url,
        github_client_id,
        github_client_secret,
        token_key,
        dev_auth: env::var("KNOCK_KNOCK_DEV_AUTH").as_deref() == Ok("1"),
    };
    let state = AppState {
        db,
        config: Arc::new(config),
        http: Client::builder().user_agent("knock-knock/0.1").build()?,
        events: Arc::new(DashMap::new()),
        presence: Arc::new(Mutex::new(HashMap::new())),
        rate_limits: Arc::new(Mutex::new(HashMap::new())),
    };

    let api = Router::new()
        .route("/health", get(health))
        .route("/config", get(public_config))
        .route("/session", get(session))
        .route("/account/api-key", get(api_key_status).post(create_api_key))
        .route("/rooms/{owner}/{repo}", get(room))
        .route(
            "/rooms/{owner}/{repo}/messages",
            get(messages).post(post_message),
        )
        .route("/rooms/{owner}/{repo}/activate", post(activate_room))
        .route("/rooms/{owner}/{repo}/deactivate", post(deactivate_room))
        .route("/rooms/{owner}/{repo}/mutes", post(mute_user))
        .route("/rooms/{owner}/{repo}/mutes/{user_id}", delete(unmute_user))
        .route("/rooms/{owner}/{repo}/stream", get(room_stream))
        .route("/messages/{id}", patch(edit_message).delete(remove_message))
        .route("/messages/{id}/reports", post(report_message))
        .route("/messages/{id}/hide", post(hide_message))
        .route("/v1/rooms/new-messages", get(owner_room_updates));

    let app = Router::new()
        .route("/badge.svg", get(badge))
        .route("/auth/github", get(begin_login))
        .route("/auth/github/callback", get(finish_login))
        .route("/auth/dev", get(dev_login))
        .route("/auth/logout", post(logout))
        .nest("/api", api)
        .fallback_service(
            ServeDir::new("web/dist").not_found_service(ServeFile::new("web/dist/index.html")),
        )
        .layer(TraceLayer::new_for_http())
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-robots-tag"),
            HeaderValue::from_static("noindex, nofollow"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("same-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'self'; img-src 'self' https://github.com https://avatars.githubusercontent.com data:; connect-src 'self' ws: wss:; style-src 'self'; script-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self' https://github.com"),
        ))
        .with_state(state);

    let address: SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()?;
    info!(%address, "listening");
    axum::serve(
        tokio::net::TcpListener::bind(address).await?,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn badge() -> impl IntoResponse {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="132" height="20" role="img" aria-label="Chat w/Maintainer"><title>Chat w/Maintainer</title><rect width="132" height="20" fill="#28231f"/><rect x="36" width="96" height="20" fill="#c95f3d"/><g fill="#fff" font-family="Verdana,sans-serif" font-size="10"><text x="6" y="14">Chat</text><text x="43" y="14" font-weight="bold">w/Maintainer</text></g></svg>"##;
    ([(header::CONTENT_TYPE, "image/svg+xml")], svg)
}

async fn public_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "githubOAuth": state.config.github_client_id.is_some(),
        "devAuth": state.config.dev_auth,
    }))
}

async fn session(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = authenticate(&state, &headers).await?;
    Ok(Json(
        json!({ "user": PublicUser::from(&user), "devAuth": state.config.dev_auth }),
    ))
}

async fn api_key_status(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = authenticate(&state, &headers).await?;
    let created_at: Option<i64> =
        sqlx::query_scalar("SELECT created_at FROM api_keys WHERE user_id=?")
            .bind(user.id)
            .fetch_optional(&state.db)
            .await
            .map_err(internal)?;
    Ok(Json(json!({
        "exists": created_at.is_some(),
        "createdAt": created_at.map(timestamp),
    })))
}

async fn create_api_key(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    require_mutation(&state, &headers)?;
    let user = authenticate(&state, &headers).await?;
    let can_manage: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM relationship_cache WHERE user_id=? AND can_manage=1)",
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await
    .map_err(internal)?;
    if !can_manage {
        return Err(AppError::Forbidden);
    }
    let token = format!("kk_{}", random_token(32));
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO api_keys(user_id, token_hash, created_at, last_polled_at) VALUES(?, ?, ?, NULL) ON CONFLICT(user_id) DO UPDATE SET token_hash=excluded.token_hash, created_at=excluded.created_at, last_polled_at=NULL")
        .bind(user.id)
        .bind(hash_token(&token))
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(internal)?;
    let mut response = Json(json!({
        "apiKey": token,
        "createdAt": timestamp(now),
    }))
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn owner_room_updates(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    let token = bearer_token(&headers)?;
    let token_hash = hash_token(token);
    let now = Utc::now().timestamp();
    let user_id = claim_api_poll(&state.db, &token_hash, now).await?;
    let cutoff = now - VISIBLE_SECONDS;
    let rows = sqlx::query("SELECT repositories.owner, repositories.name, COUNT(messages.id) AS new_message_count, MAX(messages.created_at) AS latest_message_at, room_views.last_opened_at FROM relationship_cache JOIN rooms ON rooms.repository_id=relationship_cache.repository_id JOIN repositories ON repositories.id=rooms.repository_id JOIN room_views ON room_views.room_id=rooms.id AND room_views.user_id=relationship_cache.user_id JOIN messages ON messages.room_id=rooms.id WHERE relationship_cache.user_id=? AND relationship_cache.can_manage=1 AND rooms.active=1 AND messages.id>room_views.last_opened_message_id AND messages.author_id<>? AND messages.state='visible' AND messages.created_at>? AND messages.created_at>=rooms.visible_since GROUP BY rooms.id, repositories.owner, repositories.name, room_views.last_opened_at ORDER BY latest_message_at DESC")
        .bind(user_id)
        .bind(user_id)
        .bind(cutoff)
        .fetch_all(&state.db)
        .await
        .map_err(internal)?;
    let rooms: Vec<_> = rows
        .into_iter()
        .map(|row| {
            let owner: String = row.get("owner");
            let repository: String = row.get("name");
            OwnerRoomUpdate {
                url: format!("{}/{owner}/{repository}", state.config.base_url),
                owner,
                repository,
                new_message_count: row.get("new_message_count"),
                latest_message_at: timestamp(row.get("latest_message_at")),
                last_opened_at: timestamp(row.get("last_opened_at")),
            }
        })
        .collect();
    let mut response = Json(json!({
        "rooms": rooms,
        "polledAt": timestamp(now),
    }))
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn claim_api_poll(db: &SqlitePool, token_hash: &[u8], now: i64) -> Result<i64> {
    let user_id: Option<i64> = sqlx::query_scalar("UPDATE api_keys SET last_polled_at=? WHERE token_hash=? AND (last_polled_at IS NULL OR last_polled_at<=?) RETURNING user_id")
        .bind(now)
        .bind(token_hash)
        .bind(now - OWNER_API_POLL_SECONDS)
        .fetch_optional(db)
        .await
        .map_err(internal)?;
    if let Some(user_id) = user_id {
        return Ok(user_id);
    }
    let last_polled_at: Option<i64> =
        sqlx::query_scalar("SELECT last_polled_at FROM api_keys WHERE token_hash=?")
            .bind(token_hash)
            .fetch_optional(db)
            .await
            .map_err(internal)?;
    let last_polled_at = last_polled_at.ok_or(AppError::Unauthorized)?;
    Err(AppError::PollingTooQuickly(
        (OWNER_API_POLL_SECONDS - (now - last_polled_at)).max(1),
    ))
}

async fn begin_login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> Result<Redirect> {
    let client_id = state
        .config
        .github_client_id
        .as_ref()
        .ok_or_else(|| AppError::Conflict("GitHub OAuth is not configured".into()))?;
    let return_to = safe_return_to(query.return_to);
    let state_token = random_token(32);
    let verifier = random_token(32);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let now = Utc::now().timestamp();
    sqlx::query("DELETE FROM oauth_states WHERE expires_at <= ?")
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(internal)?;
    sqlx::query("INSERT INTO oauth_states(state_hash, code_verifier, return_to, expires_at) VALUES(?, ?, ?, ?)")
        .bind(hash_token(&state_token)).bind(&verifier).bind(&return_to).bind(now + 600)
        .execute(&state.db).await.map_err(internal)?;
    let mut url = Url::parse("https://github.com/login/oauth/authorize").expect("valid GitHub URL");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair(
            "redirect_uri",
            &format!("{}/auth/github/callback", state.config.base_url),
        )
        .append_pair("state", &state_token)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(Redirect::temporary(url.as_str()))
}

async fn finish_login(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response> {
    let row = sqlx::query("DELETE FROM oauth_states WHERE state_hash = ? AND expires_at > ? RETURNING code_verifier, return_to")
        .bind(hash_token(&query.state)).bind(Utc::now().timestamp())
        .fetch_optional(&state.db).await.map_err(internal)?
        .ok_or_else(|| AppError::BadRequest("invalid or expired OAuth state".into()))?;
    let verifier: String = row.get("code_verifier");
    let return_to: String = row.get("return_to");
    let token: OAuthTokenResponse = state
        .http
        .post("https://github.com/login/oauth/access_token")
        .header(header::ACCEPT, "application/json")
        .json(&json!({
            "client_id": state.config.github_client_id,
            "client_secret": state.config.github_client_secret,
            "code": query.code,
            "redirect_uri": format!("{}/auth/github/callback", state.config.base_url),
            "code_verifier": verifier,
        }))
        .send()
        .await
        .map_err(internal)?
        .error_for_status()
        .map_err(internal)?
        .json()
        .await
        .map_err(internal)?;
    let github_user: GithubUser =
        github_get(&state, &token.access_token, "https://api.github.com/user").await?;
    let user_id = upsert_user(&state.db, &github_user).await?;
    let encrypted = encrypt_token(
        state.config.token_key.as_ref().expect("validated config"),
        &token.access_token,
    )?;
    sqlx::query("INSERT INTO oauth_credentials(user_id, encrypted_token, updated_at) VALUES(?, ?, ?) ON CONFLICT(user_id) DO UPDATE SET encrypted_token=excluded.encrypted_token, updated_at=excluded.updated_at")
        .bind(user_id).bind(encrypted).bind(Utc::now().timestamp()).execute(&state.db).await.map_err(internal)?;
    make_session_response(&state, user_id, &return_to).await
}

async fn dev_login(
    State(state): State<AppState>,
    Query(query): Query<DevLoginQuery>,
) -> Result<Response> {
    if !state.config.dev_auth {
        return Err(AppError::NotFound("not found".into()));
    }
    let login = query.login.unwrap_or_else(|| "octocat".into());
    if login.is_empty()
        || login.len() > 39
        || !login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(AppError::BadRequest("invalid GitHub login".into()));
    }
    let digest = Sha256::digest(login.as_bytes());
    let github_id = i64::from_be_bytes(digest[..8].try_into().expect("eight bytes")) & i64::MAX;
    let user = GithubUser {
        id: github_id,
        avatar_url: format!("https://github.com/{login}.png?size=96"),
        login,
    };
    let user_id = upsert_user(&state.db, &user).await?;
    make_session_response(&state, user_id, &safe_return_to(query.return_to)).await
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    require_mutation(&state, &headers)?;
    if let Some(token) = cookie(&headers, "kk_session") {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(hash_token(token))
            .execute(&state.db)
            .await
            .map_err(internal)?;
    }
    let mut response = Redirect::to("/").into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("kk_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    Ok(response)
}

async fn make_session_response(
    state: &AppState,
    user_id: i64,
    return_to: &str,
) -> Result<Response> {
    let token = random_token(32);
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO sessions(token_hash, user_id, expires_at, last_activity_at) VALUES(?, ?, ?, ?)")
        .bind(hash_token(&token)).bind(user_id).bind(now + SESSION_SECONDS).bind(now)
        .execute(&state.db).await.map_err(internal)?;
    let secure = if state.config.base_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!(
        "kk_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_SECONDS}{secure}"
    );
    let mut response = Redirect::to(return_to).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(internal)?,
    );
    Ok(response)
}

async fn room(
    State(state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<RoomView>> {
    let user = authenticate(&state, &headers).await?;
    let (repository, relationship) = resolve_repository(&state, &user, &owner, &repo).await?;
    let room = ensure_room(&state.db, repository.id).await?;
    mark_room_opened(&state.db, user.id, room.id).await?;
    Ok(Json(RoomView {
        request_issue_url: issue_url(&repository),
        repository: repository_view(&repository),
        active: room.active,
        current_user: PublicUser::from(&user),
        can_manage: relationship.can_manage,
        relationship,
        retention_days: 14,
    }))
}

async fn activate_room(
    State(state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<RoomView>> {
    require_mutation(&state, &headers)?;
    let user = authenticate(&state, &headers).await?;
    let (repository, relationship) = resolve_repository(&state, &user, &owner, &repo).await?;
    if !relationship.can_manage {
        return Err(AppError::Forbidden);
    }
    ensure_room(&state.db, repository.id).await?;
    let now = Utc::now().timestamp();
    sqlx::query("UPDATE rooms SET active=1, visible_since=?, activated_by=?, activated_at=?, deactivated_by=NULL, deactivated_at=NULL WHERE repository_id=? AND active=0")
        .bind(now).bind(user.id).bind(now).bind(repository.id).execute(&state.db).await.map_err(internal)?;
    let room = ensure_room(&state.db, repository.id).await?;
    Ok(Json(RoomView {
        repository: repository_view(&repository),
        active: room.active,
        current_user: PublicUser::from(&user),
        relationship: relationship.clone(),
        can_manage: relationship.can_manage,
        request_issue_url: issue_url(&repository),
        retention_days: 14,
    }))
}

async fn deactivate_room(
    State(state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    require_mutation(&state, &headers)?;
    let user = authenticate(&state, &headers).await?;
    let (repository, relationship) = resolve_repository(&state, &user, &owner, &repo).await?;
    if !relationship.can_manage {
        return Err(AppError::Forbidden);
    }
    let now = Utc::now().timestamp();
    let result = sqlx::query("UPDATE rooms SET active=0, deactivated_by=?, deactivated_at=? WHERE repository_id=? AND active=1")
        .bind(user.id).bind(now).bind(repository.id).execute(&state.db).await.map_err(internal)?;
    if result.rows_affected() > 0 {
        let room = ensure_room(&state.db, repository.id).await?;
        broadcast_event(&state, room.id, json!({ "type": "room.deactivated" }));
    }
    Ok(Json(json!({ "active": false })))
}

async fn messages(
    State(state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    Query(query): Query<HistoryQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let user = authenticate(&state, &headers).await?;
    let (repository, _) = resolve_repository(&state, &user, &owner, &repo).await?;
    let room = ensure_room(&state.db, repository.id).await?;
    if !room.active {
        return Err(AppError::Conflict("this room is not active".into()));
    }
    let limit = query.limit.unwrap_or(100).clamp(1, 100);
    let before = query.before.unwrap_or(i64::MAX);
    let cutoff = Utc::now().timestamp() - VISIBLE_SECONDS;
    let rows = sqlx::query("SELECT m.id, m.markdown, m.affiliation, m.state, m.created_at, m.edited_at, u.github_id, u.login, u.avatar_url FROM messages m JOIN users u ON u.id=m.author_id WHERE m.room_id=? AND m.id < ? AND m.created_at > ? AND m.created_at >= ? ORDER BY m.id DESC LIMIT ?")
        .bind(room.id).bind(before).bind(cutoff).bind(room.visible_since.unwrap_or(i64::MAX)).bind(limit + 1)
        .fetch_all(&state.db).await.map_err(internal)?;
    let has_more = rows.len() as i64 > limit;
    let items: Vec<_> = rows
        .into_iter()
        .take(limit as usize)
        .map(message_from_row)
        .collect();
    let next_cursor = if has_more {
        items.last().map(|m| m.id)
    } else {
        None
    };
    Ok(Json(
        json!({ "messages": items, "nextCursor": next_cursor }),
    ))
}

async fn post_message(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path((owner, repo)): Path<(String, String)>,
    headers: HeaderMap,
    Json(input): Json<PostMessage>,
) -> Result<Json<MessageView>> {
    require_mutation(&state, &headers)?;
    validate_message(&input.markdown)?;
    if input.client_message_id.len() > 64 || input.client_message_id.is_empty() {
        return Err(AppError::BadRequest("invalid client message ID".into()));
    }
    let user = authenticate(&state, &headers).await?;
    enforce_post_limit(&state, user.id, peer).await?;
    let (repository, relationship) = resolve_repository(&state, &user, &owner, &repo).await?;
    let room = ensure_room(&state.db, repository.id).await?;
    if !room.active {
        return Err(AppError::Conflict("this room is not active".into()));
    }
    ensure_can_post(&state.db, room.id, user.id).await?;
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await.map_err(internal)?;
    sqlx::query("INSERT INTO messages(room_id, author_id, client_message_uuid, markdown, affiliation, created_at) VALUES(?, ?, ?, ?, ?, ?) ON CONFLICT(author_id, client_message_uuid) DO NOTHING")
        .bind(room.id).bind(user.id).bind(&input.client_message_id).bind(&input.markdown).bind(&relationship.pill).bind(now)
        .execute(&mut *tx).await.map_err(internal)?;
    let row = sqlx::query("SELECT m.id, m.markdown, m.affiliation, m.state, m.created_at, m.edited_at, u.github_id, u.login, u.avatar_url FROM messages m JOIN users u ON u.id=m.author_id WHERE m.author_id=? AND m.client_message_uuid=?")
        .bind(user.id).bind(&input.client_message_id).fetch_one(&mut *tx).await.map_err(internal)?;
    let message = message_from_row(row);
    sqlx::query("INSERT OR IGNORE INTO message_revisions(message_id, revision, markdown, editor_id, created_at) VALUES(?, 1, ?, ?, ?)")
        .bind(message.id).bind(&input.markdown).bind(user.id).bind(now).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    broadcast_event(
        &state,
        room.id,
        json!({ "type": "message.created", "message": message.clone() }),
    );
    Ok(Json(message))
}

async fn mute_user(
    State(state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    headers: HeaderMap,
    Json(input): Json<MuteRequest>,
) -> Result<Json<Value>> {
    require_mutation(&state, &headers)?;
    let actor = authenticate(&state, &headers).await?;
    let (repository, relationship) = resolve_repository(&state, &actor, &owner, &repo).await?;
    if !relationship.can_manage {
        return Err(AppError::Forbidden);
    }
    let room = ensure_room(&state.db, repository.id).await?;
    let target_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE github_id=?")
        .bind(input.user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal)?
        .ok_or_else(|| AppError::NotFound("user has not visited Knock Knock".into()))?;
    if target_id == actor.id {
        return Err(AppError::BadRequest("you cannot mute yourself".into()));
    }
    let reason = clean_reason(input.reason)?;
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await.map_err(internal)?;
    sqlx::query("INSERT INTO room_mutes(room_id, user_id, actor_id, reason, active, created_at, updated_at) VALUES(?, ?, ?, ?, 1, ?, ?) ON CONFLICT(room_id, user_id) DO UPDATE SET actor_id=excluded.actor_id, reason=excluded.reason, active=1, updated_at=excluded.updated_at")
        .bind(room.id).bind(target_id).bind(actor.id).bind(&reason).bind(now).bind(now)
        .execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("INSERT INTO moderation_actions(actor_id, room_id, target_user_id, action, reason, created_at) VALUES(?, ?, ?, 'mute_user', ?, ?)")
        .bind(actor.id).bind(room.id).bind(target_id).bind(reason).bind(now)
        .execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Json(json!({ "muted": true })))
}

async fn unmute_user(
    State(state): State<AppState>,
    Path((owner, repo, github_user_id)): Path<(String, String, i64)>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    require_mutation(&state, &headers)?;
    let actor = authenticate(&state, &headers).await?;
    let (repository, relationship) = resolve_repository(&state, &actor, &owner, &repo).await?;
    if !relationship.can_manage {
        return Err(AppError::Forbidden);
    }
    let room = ensure_room(&state.db, repository.id).await?;
    let target_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE github_id=?")
        .bind(github_user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal)?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await.map_err(internal)?;
    sqlx::query(
        "UPDATE room_mutes SET active=0, actor_id=?, updated_at=? WHERE room_id=? AND user_id=?",
    )
    .bind(actor.id)
    .bind(now)
    .bind(room.id)
    .bind(target_id)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    sqlx::query("INSERT INTO moderation_actions(actor_id, room_id, target_user_id, action, reason, created_at) VALUES(?, ?, ?, 'unmute_user', 'unmuted by maintainer', ?)")
        .bind(actor.id).bind(room.id).bind(target_id).bind(now)
        .execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Json(json!({ "muted": false })))
}

async fn edit_message(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(input): Json<EditMessage>,
) -> Result<Json<MessageView>> {
    require_mutation(&state, &headers)?;
    validate_message(&input.markdown)?;
    let user = authenticate(&state, &headers).await?;
    let (room_id, visible_since, active, author_id, created_at) =
        message_context(&state.db, id).await?;
    ensure_author_visible(&user, author_id, active, visible_since, created_at)?;
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await.map_err(internal)?;
    let revision: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(revision), 0) + 1 FROM message_revisions WHERE message_id=?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    sqlx::query("INSERT INTO message_revisions(message_id, revision, markdown, editor_id, created_at) VALUES(?, ?, ?, ?, ?)")
        .bind(id).bind(revision).bind(&input.markdown).bind(user.id).bind(now).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("UPDATE messages SET markdown=?, edited_at=? WHERE id=? AND state='visible'")
        .bind(&input.markdown)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let row = message_row(&mut tx, id).await?;
    let message = message_from_row(row);
    tx.commit().await.map_err(internal)?;
    broadcast_event(
        &state,
        room_id,
        json!({ "type": "message.updated", "message": message.clone() }),
    );
    Ok(Json(message))
}

async fn remove_message(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<MessageView>> {
    require_mutation(&state, &headers)?;
    let user = authenticate(&state, &headers).await?;
    let (room_id, visible_since, active, author_id, created_at) =
        message_context(&state.db, id).await?;
    ensure_author_visible(&user, author_id, active, visible_since, created_at)?;
    sqlx::query("UPDATE messages SET state='removed', removed_at=? WHERE id=? AND state='visible'")
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(internal)?;
    let row = sqlx::query("SELECT m.id, m.markdown, m.affiliation, m.state, m.created_at, m.edited_at, u.github_id, u.login, u.avatar_url FROM messages m JOIN users u ON u.id=m.author_id WHERE m.id=?").bind(id).fetch_one(&state.db).await.map_err(internal)?;
    let message = message_from_row(row);
    broadcast_event(
        &state,
        room_id,
        json!({ "type": "message.updated", "message": message.clone() }),
    );
    Ok(Json(message))
}

async fn report_message(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(input): Json<Reason>,
) -> Result<(StatusCode, Json<Value>)> {
    require_mutation(&state, &headers)?;
    let user = authenticate(&state, &headers).await?;
    let reason = clean_reason(input.reason)?;
    sqlx::query("INSERT INTO reports(reporter_id, message_id, reason, created_at) VALUES(?, ?, ?, ?) ON CONFLICT(reporter_id, message_id) DO UPDATE SET reason=excluded.reason, created_at=excluded.created_at")
        .bind(user.id).bind(id).bind(reason).bind(Utc::now().timestamp()).execute(&state.db).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(json!({ "reported": true }))))
}

async fn hide_message(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(input): Json<Reason>,
) -> Result<Json<MessageView>> {
    require_mutation(&state, &headers)?;
    let user = authenticate(&state, &headers).await?;
    let row = sqlx::query("SELECT m.room_id, r.owner, r.name FROM messages m JOIN rooms room ON room.id=m.room_id JOIN repositories r ON r.id=room.repository_id WHERE m.id=?").bind(id).fetch_optional(&state.db).await.map_err(internal)?.ok_or_else(|| AppError::NotFound("message not found".into()))?;
    let room_id: i64 = row.get("room_id");
    let owner: String = row.get("owner");
    let name: String = row.get("name");
    let (_, relationship) = resolve_repository(&state, &user, &owner, &name).await?;
    if !relationship.can_manage {
        return Err(AppError::Forbidden);
    }
    let reason = clean_reason(input.reason)?;
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await.map_err(internal)?;
    sqlx::query("UPDATE messages SET state='hidden' WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    sqlx::query("INSERT INTO moderation_actions(actor_id, room_id, message_id, action, reason, created_at) VALUES(?, ?, ?, 'hide_message', ?, ?)")
        .bind(user.id).bind(room_id).bind(id).bind(reason).bind(now).execute(&mut *tx).await.map_err(internal)?;
    let message = message_from_row(message_row(&mut tx, id).await?);
    tx.commit().await.map_err(internal)?;
    broadcast_event(
        &state,
        room_id,
        json!({ "type": "message.updated", "message": message.clone() }),
    );
    Ok(Json(message))
}

async fn room_stream(
    State(state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response> {
    require_origin(&state, &headers)?;
    let user = authenticate(&state, &headers).await?;
    let (repository, relationship) = resolve_repository(&state, &user, &owner, &repo).await?;
    let room = ensure_room(&state.db, repository.id).await?;
    if !room.active {
        return Err(AppError::Conflict("this room is not active".into()));
    }
    Ok(ws.on_upgrade(move |socket| websocket(state, socket, room.id, user, relationship.pill)))
}

async fn websocket(
    state: AppState,
    mut socket: WebSocket,
    room_id: i64,
    user: AuthUser,
    affiliation: Option<String>,
) {
    let mut events = sender(&state, room_id).subscribe();
    let joined = change_presence(&state, room_id, &user, affiliation.clone(), 1).await;
    if joined {
        broadcast_presence(&state, room_id).await;
    }
    let _ = socket
        .send(WsMessage::Text(
            json!({ "type": "ready", "userId": user.github_id, "affiliation": affiliation })
                .to_string()
                .into(),
        ))
        .await;
    let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
    let mut last_seen = Instant::now();
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(WsMessage::Pong(_))) | Some(Ok(WsMessage::Text(_))) => last_seen = Instant::now(),
                Some(Ok(WsMessage::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
            event = events.recv() => if let Ok(event) = event {
                if socket.send(WsMessage::Text(event.into())).await.is_err() { break; }
            },
            _ = heartbeat.tick() => {
                if last_seen.elapsed() > Duration::from_secs(45) || socket.send(WsMessage::Ping(Vec::new().into())).await.is_err() { break; }
            }
        }
    }
    let left = change_presence(&state, room_id, &user, affiliation, -1).await;
    if left {
        broadcast_presence(&state, room_id).await;
    }
}

async fn change_presence(
    state: &AppState,
    room_id: i64,
    user: &AuthUser,
    affiliation: Option<String>,
    delta: i32,
) -> bool {
    let mut presence = state.presence.lock().await;
    let room = presence.entry(room_id).or_default();
    let was_present = room.contains_key(&user.id);
    if delta > 0 {
        room.entry(user.id)
            .and_modify(|entry| entry.sockets += 1)
            .or_insert_with(|| PresenceUser {
                sockets: 1,
                github_id: user.github_id,
                login: user.login.clone(),
                affiliation,
            });
    } else if let Some(entry) = room.get_mut(&user.id) {
        entry.sockets -= 1;
        if entry.sockets == 0 {
            room.remove(&user.id);
        }
    }
    was_present != room.contains_key(&user.id)
}

async fn broadcast_presence(state: &AppState, room_id: i64) {
    let presence = state.presence.lock().await;
    let room = presence.get(&room_id);
    let count = room.map(HashMap::len).unwrap_or(0);
    let affiliated: Vec<_> = room
        .into_iter()
        .flat_map(HashMap::values)
        .filter_map(|user| {
            user.affiliation.as_ref().map(|affiliation| {
                json!({ "id": user.github_id, "login": user.login, "affiliation": affiliation })
            })
        })
        .collect();
    drop(presence);
    broadcast_event(
        state,
        room_id,
        json!({ "type": "presence", "count": count, "affiliated": affiliated }),
    );
}

fn sender(state: &AppState, room_id: i64) -> broadcast::Sender<String> {
    state
        .events
        .entry(room_id)
        .or_insert_with(|| broadcast::channel(256).0)
        .clone()
}

fn broadcast_event(state: &AppState, room_id: i64, event: Value) {
    let _ = sender(state, room_id).send(event.to_string());
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<AuthUser> {
    let token = cookie(headers, "kk_session").ok_or(AppError::Unauthorized)?;
    let now = Utc::now().timestamp();
    let row = sqlx::query("SELECT u.id, u.github_id, u.login, u.avatar_url, c.encrypted_token FROM sessions s JOIN users u ON u.id=s.user_id LEFT JOIN oauth_credentials c ON c.user_id=u.id WHERE s.token_hash=? AND s.expires_at>?")
        .bind(hash_token(token)).bind(now).fetch_optional(&state.db).await.map_err(internal)?.ok_or(AppError::Unauthorized)?;
    sqlx::query("UPDATE sessions SET expires_at=?, last_activity_at=? WHERE token_hash=?")
        .bind(now + SESSION_SECONDS)
        .bind(now)
        .bind(hash_token(token))
        .execute(&state.db)
        .await
        .map_err(internal)?;
    let encrypted: Option<Vec<u8>> = row.try_get("encrypted_token").ok();
    let access_token = match (encrypted, state.config.token_key.as_ref()) {
        (Some(value), Some(key)) => Some(decrypt_token(key, &value)?),
        _ => None,
    };
    Ok(AuthUser {
        id: row.get("id"),
        github_id: row.get("github_id"),
        login: row.get("login"),
        avatar_url: row.get("avatar_url"),
        token: access_token,
    })
}

async fn resolve_repository(
    state: &AppState,
    user: &AuthUser,
    owner: &str,
    repo: &str,
) -> Result<(Repository, Relationship)> {
    validate_slug(owner)?;
    validate_slug(repo)?;
    let github_repo = if let Some(token) = &user.token {
        let endpoint = format!("https://api.github.com/repos/{owner}/{repo}");
        github_get::<GithubRepository>(state, token, &endpoint).await?
    } else if state.config.dev_auth {
        let digest = Sha256::digest(format!("{owner}/{repo}").as_bytes());
        GithubRepository {
            id: i64::from_be_bytes(digest[..8].try_into().expect("eight bytes")) & i64::MAX,
            name: repo.into(),
            private: false,
            html_url: format!("https://github.com/{owner}/{repo}"),
            description: Some("Local development repository".into()),
            has_issues: true,
            owner: GithubOwner {
                login: owner.into(),
                kind: "User".into(),
            },
            permissions: GithubPermissions {
                admin: owner.eq_ignore_ascii_case(&user.login),
                _pull: true,
                ..Default::default()
            },
        }
    } else {
        return Err(AppError::Unauthorized);
    };
    if github_repo.private {
        return Err(AppError::NotFound(
            "private repositories are not supported".into(),
        ));
    }
    let repository = upsert_repository(&state.db, &github_repo).await?;
    let relationship = relationship_for(user, &github_repo);
    sqlx::query("INSERT INTO relationship_cache(user_id, repository_id, relationship, can_manage, verified_at) VALUES(?, ?, ?, ?, ?) ON CONFLICT(user_id, repository_id) DO UPDATE SET relationship=excluded.relationship, can_manage=excluded.can_manage, verified_at=excluded.verified_at")
        .bind(user.id).bind(repository.id).bind(relationship.pill.as_deref().unwrap_or("none")).bind(relationship.can_manage).bind(Utc::now().timestamp())
        .execute(&state.db).await.map_err(internal)?;
    Ok((repository, relationship))
}

fn relationship_for(user: &AuthUser, repo: &GithubRepository) -> Relationship {
    if repo.owner.kind == "User" && repo.owner.login.eq_ignore_ascii_case(&user.login) {
        Relationship {
            pill: Some("owner".into()),
            can_manage: true,
        }
    } else if repo.permissions.admin || repo.permissions.maintain {
        Relationship {
            pill: Some("maintainer".into()),
            can_manage: true,
        }
    } else if repo.permissions.push {
        Relationship {
            pill: Some("maintainer".into()),
            can_manage: false,
        }
    } else if repo.permissions.triage {
        Relationship {
            pill: Some("collaborator".into()),
            can_manage: false,
        }
    } else {
        Relationship {
            pill: None,
            can_manage: false,
        }
    }
}

async fn github_get<T: for<'de> Deserialize<'de>>(
    state: &AppState,
    token: &str,
    url: &str,
) -> Result<T> {
    let response = state
        .http
        .get(url)
        .bearer_auth(token)
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(internal)?;
    match response.status() {
        StatusCode::NOT_FOUND => Err(AppError::NotFound(
            "public GitHub repository not found".into(),
        )),
        StatusCode::UNAUTHORIZED => Err(AppError::Unauthorized),
        status if !status.is_success() => {
            Err(AppError::Conflict(format!("GitHub returned {status}")))
        }
        _ => response.json().await.map_err(internal),
    }
}

async fn upsert_user(db: &SqlitePool, user: &GithubUser) -> Result<i64> {
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO users(github_id, login, avatar_url, created_at, updated_at) VALUES(?, ?, ?, ?, ?) ON CONFLICT(github_id) DO UPDATE SET login=excluded.login, avatar_url=excluded.avatar_url, updated_at=excluded.updated_at")
        .bind(user.id).bind(&user.login).bind(&user.avatar_url).bind(now).bind(now).execute(db).await.map_err(internal)?;
    sqlx::query_scalar("SELECT id FROM users WHERE github_id=?")
        .bind(user.id)
        .fetch_one(db)
        .await
        .map_err(internal)
}

async fn upsert_repository(db: &SqlitePool, repo: &GithubRepository) -> Result<Repository> {
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO repositories(github_id, owner, name, html_url, description, has_issues, updated_at) VALUES(?, ?, ?, ?, ?, ?, ?) ON CONFLICT(github_id) DO UPDATE SET owner=excluded.owner, name=excluded.name, html_url=excluded.html_url, description=excluded.description, has_issues=excluded.has_issues, updated_at=excluded.updated_at")
        .bind(repo.id).bind(&repo.owner.login).bind(&repo.name).bind(&repo.html_url).bind(&repo.description).bind(repo.has_issues).bind(now)
        .execute(db).await.map_err(internal)?;
    let row = sqlx::query("SELECT id, owner, name, html_url, description, has_issues FROM repositories WHERE github_id=?").bind(repo.id).fetch_one(db).await.map_err(internal)?;
    Ok(Repository {
        id: row.get("id"),
        owner: row.get("owner"),
        name: row.get("name"),
        html_url: row.get("html_url"),
        description: row.get("description"),
        has_issues: row.get("has_issues"),
    })
}

struct Room {
    id: i64,
    active: bool,
    visible_since: Option<i64>,
}

async fn ensure_room(db: &SqlitePool, repository_id: i64) -> Result<Room> {
    sqlx::query("INSERT OR IGNORE INTO rooms(repository_id) VALUES(?)")
        .bind(repository_id)
        .execute(db)
        .await
        .map_err(internal)?;
    let row = sqlx::query("SELECT id, active, visible_since FROM rooms WHERE repository_id=?")
        .bind(repository_id)
        .fetch_one(db)
        .await
        .map_err(internal)?;
    Ok(Room {
        id: row.get("id"),
        active: row.get("active"),
        visible_since: row.get("visible_since"),
    })
}

async fn mark_room_opened(db: &SqlitePool, user_id: i64, room_id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO room_views(user_id, room_id, last_opened_at, last_opened_message_id) VALUES(?, ?, ?, COALESCE((SELECT MAX(id) FROM messages WHERE room_id=?), 0)) ON CONFLICT(user_id, room_id) DO UPDATE SET last_opened_at=excluded.last_opened_at, last_opened_message_id=excluded.last_opened_message_id")
        .bind(user_id)
        .bind(room_id)
        .bind(now)
        .bind(room_id)
        .execute(db)
        .await
        .map_err(internal)?;
    Ok(())
}

fn repository_view(repo: &Repository) -> RepositoryView {
    RepositoryView {
        owner: repo.owner.clone(),
        name: repo.name.clone(),
        html_url: repo.html_url.clone(),
        description: repo.description.clone(),
    }
}

fn issue_url(repo: &Repository) -> Option<String> {
    if !repo.has_issues {
        return None;
    }
    let mut url = Url::parse(&format!("{}/issues/new", repo.html_url)).ok()?;
    url.query_pairs_mut()
        .append_pair("title", "Activate this repository on Knock Knock")
        .append_pair("body", "Would a repository maintainer activate the Knock Knock room? It gives GitHub users a lightweight, 14-day public conversation space for this project.");
    Some(url.into())
}

fn message_from_row(row: sqlx::sqlite::SqliteRow) -> MessageView {
    let state: String = row.get("state");
    MessageView {
        id: row.get("id"),
        author: PublicUser {
            id: row.get("github_id"),
            login: row.get("login"),
            avatar_url: row.get("avatar_url"),
        },
        markdown: if state == "visible" {
            Some(row.get("markdown"))
        } else {
            None
        },
        affiliation: row.get("affiliation"),
        state,
        created_at: timestamp(row.get("created_at")),
        edited_at: row.get::<Option<i64>, _>("edited_at").map(timestamp),
    }
}

async fn message_context(db: &SqlitePool, id: i64) -> Result<(i64, Option<i64>, bool, i64, i64)> {
    let row = sqlx::query("SELECT m.room_id, room.visible_since, room.active, m.author_id, m.created_at FROM messages m JOIN rooms room ON room.id=m.room_id WHERE m.id=?")
        .bind(id).fetch_optional(db).await.map_err(internal)?.ok_or_else(|| AppError::NotFound("message not found".into()))?;
    Ok((
        row.get("room_id"),
        row.get("visible_since"),
        row.get("active"),
        row.get("author_id"),
        row.get("created_at"),
    ))
}

async fn message_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: i64,
) -> Result<sqlx::sqlite::SqliteRow> {
    sqlx::query("SELECT m.id, m.markdown, m.affiliation, m.state, m.created_at, m.edited_at, u.github_id, u.login, u.avatar_url FROM messages m JOIN users u ON u.id=m.author_id WHERE m.id=?")
        .bind(id).fetch_one(&mut **tx).await.map_err(internal)
}

fn ensure_author_visible(
    user: &AuthUser,
    author_id: i64,
    active: bool,
    visible_since: Option<i64>,
    created_at: i64,
) -> Result<()> {
    if user.id != author_id {
        return Err(AppError::Forbidden);
    }
    if !active
        || created_at <= Utc::now().timestamp() - VISIBLE_SECONDS
        || visible_since.is_none_or(|since| created_at < since)
    {
        return Err(AppError::Conflict(
            "message is outside the public viewing window".into(),
        ));
    }
    Ok(())
}

async fn ensure_can_post(db: &SqlitePool, room_id: i64, user_id: i64) -> Result<()> {
    let blocked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM platform_blocks WHERE user_id=? AND active=1)",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(internal)?;
    let muted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM room_mutes WHERE room_id=? AND user_id=? AND active=1)",
    )
    .bind(room_id)
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(internal)?;
    if blocked || muted {
        Err(AppError::Forbidden)
    } else {
        Ok(())
    }
}

async fn enforce_post_limit(state: &AppState, user_id: i64, peer: SocketAddr) -> Result<()> {
    let mut limits = state.rate_limits.lock().await;
    let now = Instant::now();
    for (key, allowance) in [
        (format!("user:{user_id}"), 20),
        (format!("ip:{}", peer.ip()), 60),
    ] {
        let attempts = limits.entry(key).or_default();
        while attempts
            .front()
            .is_some_and(|time| now.duration_since(*time) >= Duration::from_secs(60))
        {
            attempts.pop_front();
        }
        if attempts.len() >= allowance {
            return Err(AppError::RateLimited);
        }
        attempts.push_back(now);
    }
    Ok(())
}

fn validate_message(markdown: &str) -> Result<()> {
    let chars = markdown.chars().count();
    if markdown.trim().is_empty() {
        return Err(AppError::BadRequest("message cannot be empty".into()));
    }
    if chars > MAX_MESSAGE_CHARS {
        return Err(AppError::BadRequest(format!(
            "message is longer than {MAX_MESSAGE_CHARS} characters"
        )));
    }
    for event in Parser::new(markdown) {
        if let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = event {
            if dest_url.len() > 2_048 {
                return Err(AppError::BadRequest(
                    "URL is longer than 2,048 characters".into(),
                ));
            }
            let url = Url::parse(&dest_url)
                .map_err(|_| AppError::BadRequest("links must be absolute HTTP(S) URLs".into()))?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err(AppError::BadRequest("links must use HTTP or HTTPS".into()));
            }
        }
    }
    for word in markdown
        .split_whitespace()
        .filter(|word| word.starts_with("http://") || word.starts_with("https://"))
    {
        if word.len() > 2_048 {
            return Err(AppError::BadRequest(
                "URL is longer than 2,048 characters".into(),
            ));
        }
    }
    Ok(())
}

fn clean_reason(reason: String) -> Result<String> {
    let reason = reason.trim();
    if reason.is_empty() || reason.chars().count() > 500 {
        return Err(AppError::BadRequest(
            "reason must be 1–500 characters".into(),
        ));
    }
    Ok(reason.into())
}

fn require_mutation(state: &AppState, headers: &HeaderMap) -> Result<()> {
    require_origin(state, headers)?;
    if headers.get("x-knock-knock").and_then(|v| v.to_str().ok()) != Some("1") {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

fn require_origin(state: &AppState, headers: &HeaderMap) -> Result<()> {
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok())
        != Some(state.config.base_url.as_str())
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then_some(value)
        })
}

fn bearer_token(headers: &HeaderMap) -> Result<&str> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let (scheme, token) = value.split_once(' ').ok_or(AppError::Unauthorized)?;
    if !scheme.eq_ignore_ascii_case("bearer") || !token.starts_with("kk_") || token.len() != 46 {
        return Err(AppError::Unauthorized);
    }
    Ok(token)
}

fn validate_slug(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 100
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Err(AppError::BadRequest("invalid repository path".into()))
    } else {
        Ok(())
    }
}

fn safe_return_to(value: Option<String>) -> String {
    value
        .filter(|path| path.starts_with('/') && !path.starts_with("//") && path.len() <= 300)
        .unwrap_or_else(|| "/".into())
}

fn parse_key(value: String) -> anyhow::Result<[u8; 32]> {
    let decoded = STANDARD
        .decode(value.trim())
        .or_else(|_| URL_SAFE_NO_PAD.decode(value.trim()))?;
    decoded
        .try_into()
        .map_err(|_| anyhow::anyhow!("KNOCK_KNOCK_TOKEN_KEY must encode exactly 32 bytes"))
}

fn encrypt_token(key: &[u8; 32], token: &str) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(key.into());
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt((&nonce).into(), token.as_bytes())
        .map_err(|_| internal(anyhow::anyhow!("token encryption failed")))?;
    Ok([nonce.as_slice(), &ciphertext].concat())
}

fn decrypt_token(key: &[u8; 32], value: &[u8]) -> Result<String> {
    if value.len() < 13 {
        return Err(internal(anyhow::anyhow!("invalid encrypted token")));
    }
    let cipher = ChaCha20Poly1305::new(key.into());
    let plaintext = cipher
        .decrypt((&value[..12]).into(), &value[12..])
        .map_err(|_| internal(anyhow::anyhow!("token decryption failed")))?;
    String::from_utf8(plaintext).map_err(internal)
}

fn random_token(bytes: usize) -> String {
    let mut value = vec![0; bytes];
    OsRng.fill_bytes(&mut value);
    URL_SAFE_NO_PAD.encode(value)
}

fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn timestamp(value: i64) -> String {
    DateTime::<Utc>::from_timestamp(value, 0)
        .expect("database timestamp in range")
        .to_rfc3339()
}

fn internal(error: impl Into<anyhow::Error>) -> AppError {
    AppError::Internal(error.into())
}

// ponytail: keep relationship lookup write-through until GitHub rate limits show
// that serving cached authorization before refresh is necessary.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_message_links_and_limits() {
        assert!(validate_message("hello [site](https://example.com)").is_ok());
        assert!(validate_message("[bad](javascript:alert(1))").is_err());
        assert!(validate_message(&"x".repeat(8_001)).is_err());
    }

    #[test]
    fn protects_oauth_return_path() {
        assert_eq!(
            safe_return_to(Some("/rust-lang/rust".into())),
            "/rust-lang/rust"
        );
        assert_eq!(safe_return_to(Some("//evil.test".into())), "/");
        assert_eq!(safe_return_to(Some("https://evil.test".into())), "/");
    }

    #[test]
    fn public_read_access_is_not_an_affiliation() {
        let user = AuthUser {
            id: 1,
            github_id: 1,
            login: "visitor".into(),
            avatar_url: String::new(),
            token: None,
        };
        let repo = GithubRepository {
            id: 1,
            name: "project".into(),
            private: false,
            html_url: String::new(),
            description: None,
            has_issues: true,
            owner: GithubOwner {
                login: "somebody-else".into(),
                kind: "User".into(),
            },
            permissions: GithubPermissions {
                _pull: true,
                ..Default::default()
            },
        };
        assert!(relationship_for(&user, &repo).pill.is_none());
    }

    #[test]
    fn accepts_only_knock_knock_bearer_tokens() {
        let token = format!("kk_{}", "a".repeat(43));
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        assert_eq!(bearer_token(&headers).unwrap(), token);

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer not-a-knock-knock-key"),
        );
        assert!(matches!(
            bearer_token(&headers),
            Err(AppError::Unauthorized)
        ));
    }

    #[tokio::test]
    async fn owner_api_enforces_one_poll_per_minute() {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        sqlx::query("INSERT INTO users(id, github_id, login, avatar_url, created_at, updated_at) VALUES(1, 1, 'owner', '', 0, 0)")
            .execute(&db)
            .await
            .unwrap();
        let token_hash = hash_token("kk_test");
        sqlx::query("INSERT INTO api_keys(user_id, token_hash, created_at) VALUES(1, ?, 0)")
            .bind(&token_hash)
            .execute(&db)
            .await
            .unwrap();

        assert_eq!(claim_api_poll(&db, &token_hash, 1_000).await.unwrap(), 1);
        assert!(matches!(
            claim_api_poll(&db, &token_hash, 1_000).await,
            Err(AppError::PollingTooQuickly(60))
        ));
        assert!(matches!(
            claim_api_poll(&db, &token_hash, 1_059).await,
            Err(AppError::PollingTooQuickly(1))
        ));
        assert_eq!(claim_api_poll(&db, &token_hash, 1_060).await.unwrap(), 1);
    }
}
