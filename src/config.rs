use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub bind_addr: String,
    pub model_path: PathBuf,
    pub api_keys_path: PathBuf,
    pub admin: AdminConfig,
}

#[derive(Clone)]
pub struct AdminConfig {
    pub username: String,
    pub password: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let admin_id = std::env::var("LANGTRANS_ADMIN_ID")
            .map_err(|_| anyhow::anyhow!("LANGTRANS_ADMIN_ID environment variable is required"))?;
        let admin_password = std::env::var("LANGTRANS_ADMIN_PASSWORD")
            .map_err(|_| anyhow::anyhow!("LANGTRANS_ADMIN_PASSWORD environment variable is required"))?;

        Ok(Config {
            bind_addr: {
                let port = std::env::var("LANGTRANS_PORT").unwrap_or_else(|_| "8080".to_string());
                std::env::var("LANGTRANS_BIND_ADDR")
                    .unwrap_or_else(|_| format!("0.0.0.0:{}", port))
            },
            model_path: PathBuf::from(
                std::env::var("LANGTRANS_MODEL_PATH")
                    .unwrap_or_else(|_| "./onnx-model".to_string()),
            ),
            api_keys_path: PathBuf::from(
                std::env::var("LANGTRANS_APIKEYS_PATH")
                    .unwrap_or_else(|_| "./api_keys.json".to_string()),
            ),
            admin: AdminConfig {
                username: admin_id,
                password: admin_password,
            },
        })
    }
}
