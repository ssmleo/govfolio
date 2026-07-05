//! Query/body extraction that rejects with the consistent error envelope
//! (design §6.1) instead of axum's plain-text defaults.

use axum::extract::{FromRequest, FromRequestParts, Json, Query, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// [`Query`] wrapper whose rejection is the `/v1` error envelope.
pub struct ApiQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Query::<T>::from_request_parts(parts, state).await {
            Ok(Query(value)) => Ok(Self(value)),
            Err(rejection) => Err(ApiError::bad_request(
                "invalid_query",
                rejection.body_text(),
            )),
        }
    }
}

/// [`Json`] body wrapper whose rejection is the `/v1` error envelope.
pub struct ApiJson<T>(pub T);

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(ApiError::bad_request("invalid_body", rejection.body_text())),
        }
    }
}
