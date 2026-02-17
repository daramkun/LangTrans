use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod admin;
mod api;
mod apikey;
mod config;
mod error;
mod model;
mod state;

use admin::brute_force::LoginTracker;
use admin::session::SessionStore;
use apikey::store::ApiKeyStore;
use config::Config;
use model::inference::InferenceEngine;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env()?;

    let inference = Arc::new(InferenceEngine::new(&config.model_path)?);
    let api_keys = ApiKeyStore::load_or_create(&config.api_keys_path)?;

    let state = Arc::new(AppState {
        inference,
        api_keys: tokio::sync::RwLock::new(api_keys),
        login_tracker: tokio::sync::Mutex::new(LoginTracker::new()),
        admin_config: config.admin,
        sessions: tokio::sync::Mutex::new(SessionStore::new()),
    });

    let app = Router::new()
        // Translation API
        .route(
            "/api/translate",
            get(api::translate::translate_get).post(api::translate::translate_post),
        )
        // Admin routes
        .route("/admin", get(admin::routes::admin_dashboard))
        .route(
            "/admin/login",
            get(admin::routes::admin_login_page).post(admin::routes::admin_login_submit),
        )
        .route("/admin/logout", post(admin::routes::admin_logout))
        .route("/admin/keys", post(admin::routes::admin_add_key))
        .route(
            "/admin/keys/{key_id}/revoke",
            post(admin::routes::admin_revoke_key),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("LangTrans server listening on {}", config.bind_addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
