use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;

use crate::error::AppError;

pub struct BearerToken(pub String);

impl<S> FromRequestParts<S> for BearerToken
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let header_value = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or(AppError::Unauthorized("Missing Authorization header"))?;

        let value = header_value
            .to_str()
            .map_err(|_| AppError::Unauthorized("Invalid Authorization header value"))?;

        let token = value
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized("Expected Bearer token"))?;

        Ok(BearerToken(token.to_string()))
    }
}
