use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::api::auth::BearerToken;
use crate::error::AppError;
use crate::model::language::Language;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TranslateRequest {
    pub from: String,
    pub to: String,
    pub text: String,
}

async fn do_translate(
    state: &AppState,
    token: &str,
    params: TranslateRequest,
) -> Result<String, AppError> {
    // Validate API key
    let valid = state.api_keys.read().await.validate(token);
    if !valid {
        return Err(AppError::Unauthorized("Invalid or expired API key"));
    }

    // Parse language codes
    let from_lang = Language::from_code(&params.from)?;
    let to_lang = Language::from_code(&params.to)?;

    // Run inference in blocking thread
    let inference = state.inference.clone();
    let text = params.text;
    let result = tokio::task::spawn_blocking(move || {
        inference.translate(from_lang, to_lang, &text)
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task join error: {}", e))??;

    Ok(result)
}

pub async fn translate_get(
    State(state): State<Arc<AppState>>,
    bearer: BearerToken,
    Query(params): Query<TranslateRequest>,
) -> Result<String, AppError> {
    do_translate(&state, &bearer.0, params).await
}

pub async fn translate_post(
    State(state): State<Arc<AppState>>,
    bearer: BearerToken,
    Json(params): Json<TranslateRequest>,
) -> Result<String, AppError> {
    do_translate(&state, &bearer.0, params).await
}
