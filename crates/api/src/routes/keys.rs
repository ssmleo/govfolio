//! Account bootstrap + API key management (goal 050, design §6.2/§6.4).
//!
//! Real self-serve signup is DEFERRED: until it ships, accounts are created
//! by the operator through `POST /v1/users` behind the `X-Admin-Token`
//! bootstrap gate (env `ADMIN_TOKEN`; unset = surface disabled, fail
//! closed). Keys can then be minted either by the admin (naming the user)
//! or by the user themselves with an existing key.
//!
//! Secrets discipline: the plaintext key appears exactly once — in the
//! creation response. At rest and in every listing only the hash / metadata
//! exist; nothing recoverable is ever returned again.

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use govfolio_core::domain::enums::Tier;

use crate::AppState;
use crate::auth::{AuthContext, generate_key, hash_key, require_admin};
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;

/// One account (Gold-side `user_account`).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct UserAccount {
    /// User ULID.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Unique account email.
    pub email: String,
    /// Tier (decides freshness delay + daily quota, design §6.2).
    #[sqlx(try_from = "String")]
    #[schema(value_type = Tier)]
    pub tier: TierToken,
    /// When the account was created.
    pub created_at: DateTime<Utc>,
}

/// `tier` column token re-typed through the core vocabulary on the way out.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TierToken(Tier);

impl TryFrom<String> for TierToken {
    type Error = anyhow::Error;

    fn try_from(token: String) -> Result<Self, Self::Error> {
        let tier: Tier = serde_json::from_value(serde_json::Value::String(token))
            .map_err(|e| anyhow::anyhow!("stored tier is outside the core vocabulary: {e}"))?;
        Ok(Self(tier))
    }
}

/// Body of `POST /v1/users` (admin bootstrap).
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUser {
    /// Account email (unique).
    pub email: String,
    /// Tier; defaults to `free`.
    #[serde(default)]
    pub tier: Option<Tier>,
}

/// Creates an account (bootstrap; real signup is deferred — see module docs).
///
/// # Errors
/// `401` without a valid `X-Admin-Token`; `400` on a blank email; `409` on a
/// duplicate email; `500` on backend failure.
#[utoipa::path(
    post,
    path = "/v1/users",
    tag = "account",
    request_body = CreateUser,
    responses(
        (status = 201, description = "The created account", body = UserAccount),
        (status = 400, description = "Blank email", body = ErrorBody),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 409, description = "Email already registered", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<CreateUser>,
) -> Result<(StatusCode, Json<UserAccount>), ApiError> {
    require_admin(&state, &headers)?;
    let email = body.email.trim();
    if email.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_email",
            "email must be non-empty",
        ));
    }
    let tier = govfolio_core::query::wire_token(&body.tier.unwrap_or(Tier::Free))
        .map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
    let row: Result<UserAccount, sqlx::Error> = sqlx::query_as(
        "insert into user_account (id, email, tier) values ($1, $2, $3) \
         returning id, email, tier, created_at",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(email)
    .bind(tier)
    .fetch_one(&state.pool)
    .await;
    match row {
        Ok(user) => Ok((StatusCode::CREATED, Json(user))),
        Err(err) if is_unique_violation(&err) => Err(ApiError::Conflict {
            code: "email_exists",
            message: format!("an account for {email} already exists"),
        }),
        Err(err) => Err(err.into()),
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")
    )
}

/// Body of `POST /v1/keys`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateKey {
    /// Target user — REQUIRED with admin auth, FORBIDDEN with key auth
    /// (a key only mints keys for its own account).
    #[serde(default)]
    pub user_id: Option<String>,
    /// Human label for the key (shown in listings).
    pub label: String,
}

/// The creation response — the ONLY place the plaintext key ever appears.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedKey {
    /// Key ULID (use for revocation).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// The bearer token (`gfk_...`). Shown once; store it now — only its
    /// hash is retained.
    pub key: String,
    /// Human label.
    pub label: String,
    /// When the key was created.
    pub created_at: DateTime<Utc>,
}

/// One key as listed — metadata only, nothing secret-derived.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ApiKeyInfo {
    /// Key ULID.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Human label.
    pub label: String,
    /// When the key was created.
    pub created_at: DateTime<Utc>,
    /// When the key was revoked; `null` while active.
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Creates an API key. With key auth: for the caller's own account. With
/// `X-Admin-Token`: for the user named in the body (bootstrap).
///
/// # Errors
/// `401` without any valid credential; `400` on a blank label or a body/auth
/// mismatch; `404` for an unknown user (admin path); `500` on backend
/// failure.
#[utoipa::path(
    post,
    path = "/v1/keys",
    tag = "account",
    request_body = CreateKey,
    responses(
        (status = 201, description = "The created key, plaintext INCLUDED — shown exactly once", body = CreatedKey),
        (status = 400, description = "Blank label or user_id/auth mismatch", body = ErrorBody),
        (status = 401, description = "No valid API key or admin token", body = ErrorBody),
        (status = 404, description = "Unknown user (admin path)", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn create_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<CreateKey>,
) -> Result<(StatusCode, Json<CreatedKey>), ApiError> {
    let label = body.label.trim();
    if label.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_label",
            "label must be non-empty",
        ));
    }
    let user_id = match (&auth.principal, require_admin(&state, &headers)) {
        // Admin bootstrap path: mint for the named user.
        (_, Ok(())) => body.user_id.clone().ok_or_else(|| {
            ApiError::bad_request("invalid_user_id", "user_id is required with admin auth")
        })?,
        // Key auth path: only for the caller's own account.
        (Some(principal), Err(_)) => {
            if body
                .user_id
                .as_deref()
                .is_some_and(|id| id != principal.user_id)
            {
                return Err(ApiError::bad_request(
                    "invalid_user_id",
                    "a key can only create keys for its own account (omit user_id)",
                ));
            }
            principal.user_id.clone()
        }
        (None, Err(err)) => return Err(err),
    };
    let exists: bool =
        sqlx::query_scalar("select exists(select 1 from user_account where id = $1)")
            .bind(&user_id)
            .fetch_one(&state.pool)
            .await?;
    if !exists {
        return Err(ApiError::NotFound {
            message: format!("user {user_id} not found"),
        });
    }
    let key = generate_key();
    let (id, created_at): (String, DateTime<Utc>) = sqlx::query_as(
        "insert into api_key (id, user_id, key_hash, label) values ($1, $2, $3, $4) \
         returning id, created_at",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&user_id)
    .bind(hash_key(&key))
    .bind(label)
    .fetch_one(&state.pool)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedKey {
            id,
            key,
            label: label.to_owned(),
            created_at,
        }),
    ))
}

/// Lists the caller's keys (metadata only — the plaintext is unrecoverable
/// by design).
///
/// # Errors
/// `401` without a valid API key; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/keys",
    tag = "account",
    responses(
        (status = 200, description = "The caller's keys, metadata only", body = [ApiKeyInfo]),
        (status = 401, description = "No valid API key", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_keys(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ApiKeyInfo>>, ApiError> {
    let principal = auth
        .principal
        .as_ref()
        .ok_or_else(|| ApiError::unauthorized("key_required", "authenticate with an API key"))?;
    let keys: Vec<ApiKeyInfo> = sqlx::query_as(
        "select id, label, created_at, revoked_at from api_key \
         where user_id = $1 order by id",
    )
    .bind(&principal.user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(keys))
}

/// Revokes a key (immediate — the very next request with it is `401`).
/// Key auth revokes own keys; `X-Admin-Token` revokes any.
///
/// # Errors
/// `401` without any valid credential; `404` for an unknown (or not owned)
/// key; `500` on backend failure.
#[utoipa::path(
    delete,
    path = "/v1/keys/{id}",
    tag = "account",
    params(("id" = String, Path, description = "Key ULID")),
    responses(
        (status = 204, description = "Revoked (idempotent)"),
        (status = 401, description = "No valid API key or admin token", body = ErrorBody),
        (status = 404, description = "Unknown or not owned key", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn revoke_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let owner_scope = match (&auth.principal, require_admin(&state, &headers)) {
        (_, Ok(())) => None, // admin: any key
        (Some(principal), Err(_)) => Some(principal.user_id.clone()),
        (None, Err(err)) => return Err(err),
    };
    let result = sqlx::query(
        "update api_key set revoked_at = coalesce(revoked_at, now()) \
         where id = $1 and ($2::text is null or user_id = $2)",
    )
    .bind(&id)
    .bind(&owner_scope)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound {
            message: format!("api key {id} not found"),
        });
    }
    Ok(StatusCode::NO_CONTENT)
}
