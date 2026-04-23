use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub bot: Bot,
    pub release: Environment,
    pub dev: Option<Environment>,
}

#[derive(Debug, Deserialize)]
pub struct Bot {
    pub env: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Environment {
    pub token: String,
    pub prefix: String,
    pub database_url: String,
    pub max_connections: Option<u32>,
    pub dev_ids: Option<Vec<u64>>,
    pub whitelist_enabled: Option<bool>,
    pub whitelist: Option<Vec<u64>>,
    pub repository: Option<String>,
    pub github_token: Option<String>,
    pub webhook: Option<String>,
    pub ocr_training_data: Option<String>,
    pub ocr_character_whitelist: Option<String>,
    pub s3: S3,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct S3 {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub public_base_url: Option<String>,
}
