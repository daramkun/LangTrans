use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::admin::brute_force::LoginTracker;
use crate::admin::session::SessionStore;
use crate::apikey::store::ApiKeyStore;
use crate::config::AdminConfig;
use crate::model::inference::InferenceEngine;

pub struct AppState {
    pub inference: Arc<InferenceEngine>,
    pub api_keys: RwLock<ApiKeyStore>,
    pub login_tracker: Mutex<LoginTracker>,
    pub admin_config: AdminConfig,
    pub sessions: Mutex<SessionStore>,
}
