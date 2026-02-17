use std::collections::HashMap;
use std::time::{Duration, Instant};

use rand::Rng;

const SESSION_DURATION: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone)]
pub struct AdminSession {
    pub token: String,
    pub created_at: Instant,
}

impl AdminSession {
    pub fn new() -> Self {
        AdminSession {
            token: generate_session_token(),
            created_at: Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > SESSION_DURATION
    }
}

pub struct SessionStore {
    sessions: HashMap<String, AdminSession>,
}

impl SessionStore {
    pub fn new() -> Self {
        SessionStore {
            sessions: HashMap::new(),
        }
    }

    pub fn create(&mut self) -> AdminSession {
        let session = AdminSession::new();
        self.sessions.insert(session.token.clone(), session.clone());
        session
    }

    pub fn validate(&self, token: &str) -> bool {
        self.sessions
            .get(token)
            .map(|s| !s.is_expired())
            .unwrap_or(false)
    }

    pub fn remove(&mut self, token: &str) {
        self.sessions.remove(token);
    }

    pub fn _cleanup_expired(&mut self) {
        self.sessions.retain(|_, s| !s.is_expired());
    }
}

fn generate_session_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.r#gen()).collect();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
