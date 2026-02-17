use std::net::SocketAddr;
use std::sync::Arc;

use askama::Template;
use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use chrono::NaiveDateTime;
use serde::Deserialize;

use crate::apikey::store::ApiKey;
use crate::state::AppState;

// Templates

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    keys: Vec<ApiKey>,
    message: Option<String>,
}

// Form structs

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub struct AddKeyForm {
    label: String,
    expires_at: Option<String>,
}

// Cookie helpers

fn get_session_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|c| {
            let c = c.trim();
            c.strip_prefix("session=").map(|v| v.to_string())
        })
}

fn set_session_cookie(token: &str) -> String {
    format!("session={}; HttpOnly; SameSite=Strict; Path=/admin", token)
}

fn clear_session_cookie() -> String {
    "session=; HttpOnly; SameSite=Strict; Path=/admin; Max-Age=0".to_string()
}

// Handlers

pub async fn admin_login_page() -> impl IntoResponse {
    Html(
        LoginTemplate { error: None }
            .render()
            .unwrap_or_default(),
    )
}

pub async fn admin_login_submit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(form): Form<LoginForm>,
) -> Response {
    let ip = addr.ip();

    // Check brute force block
    {
        let tracker = state.login_tracker.lock().await;
        if tracker.is_blocked(&ip) {
            let html = LoginTemplate {
                error: Some("Too many failed attempts. Please try again later.".into()),
            }
            .render()
            .unwrap_or_default();
            return (StatusCode::FORBIDDEN, Html(html)).into_response();
        }
    }

    // Validate credentials
    if form.username == state.admin_config.username
        && form.password == state.admin_config.password
    {
        // Success
        state.login_tracker.lock().await.record_success(&ip);
        let session = state.sessions.lock().await.create();
        let cookie = set_session_cookie(&session.token);
        (
            [(axum::http::header::SET_COOKIE, cookie)],
            Redirect::to("/admin"),
        )
            .into_response()
    } else {
        // Failure
        state.login_tracker.lock().await.record_failure(ip);
        let html = LoginTemplate {
            error: Some("Invalid username or password.".into()),
        }
        .render()
        .unwrap_or_default();
        (StatusCode::UNAUTHORIZED, Html(html)).into_response()
    }
}

pub async fn admin_dashboard(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Check session
    let token = match get_session_token(&headers) {
        Some(t) => t,
        None => return Redirect::to("/admin/login").into_response(),
    };

    if !state.sessions.lock().await.validate(&token) {
        return Redirect::to("/admin/login").into_response();
    }

    let keys = state.api_keys.read().await.list().to_vec();
    let html = DashboardTemplate {
        keys,
        message: None,
    }
    .render()
    .unwrap_or_default();
    Html(html).into_response()
}

pub async fn admin_logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    if let Some(token) = get_session_token(&headers) {
        state.sessions.lock().await.remove(&token);
    }
    let cookie = clear_session_cookie();
    (
        [(axum::http::header::SET_COOKIE, cookie)],
        Redirect::to("/admin/login"),
    )
        .into_response()
}

pub async fn admin_add_key(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Form(form): Form<AddKeyForm>,
) -> Response {
    // Check session
    let token = match get_session_token(&headers) {
        Some(t) => t,
        None => return Redirect::to("/admin/login").into_response(),
    };
    if !state.sessions.lock().await.validate(&token) {
        return Redirect::to("/admin/login").into_response();
    }

    // Parse optional expiration
    let expires_at = form.expires_at.and_then(|s| {
        if s.is_empty() {
            None
        } else {
            NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M")
                .ok()
                .map(|dt| dt.and_utc())
        }
    });

    let mut store = state.api_keys.write().await;
    match store.add(form.label, expires_at) {
        Ok(key) => {
            let keys = store.list().to_vec();
            let html = DashboardTemplate {
                keys,
                message: Some(format!("Key created: {}", key.key)),
            }
            .render()
            .unwrap_or_default();
            Html(html).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to add key: {:?}", e);
            Redirect::to("/admin").into_response()
        }
    }
}

pub async fn admin_revoke_key(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(key_id): axum::extract::Path<String>,
) -> Response {
    let token = match get_session_token(&headers) {
        Some(t) => t,
        None => return Redirect::to("/admin/login").into_response(),
    };
    if !state.sessions.lock().await.validate(&token) {
        return Redirect::to("/admin/login").into_response();
    }

    let mut store = state.api_keys.write().await;
    let _ = store.revoke(&key_id);
    drop(store);

    Redirect::to("/admin").into_response()
}
