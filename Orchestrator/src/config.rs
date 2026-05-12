use serde::{Deserialize, Serialize};

/// Top-level configuration for the Orchestrator service.
///
/// Loaded from `orchestrator.toml` (searched in CWD, then `/etc/fixagent/`).
/// Individual fields can be overridden by environment variables for container
/// deployments (see `apply_env_overrides`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct OrchestratorConfig {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub llm: LlmConfig,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            server: ServerConfig::default(),
            llm: LlmConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://fixagent:fixagent@localhost:5432/fixagent".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
        }
    }
}

/// LLM configuration that gets injected into ReviewAgent / FixAgent at runtime.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            base_url: None,
            api_key: None,
        }
    }
}

impl OrchestratorConfig {
    /// Load configuration from `orchestrator.toml`.
    ///
    /// Search order:
    /// 1. `./orchestrator.toml`
    /// 2. `/etc/fixagent/orchestrator.toml`
    ///
    /// If no file is found, use defaults. After loading the file, environment
    /// variable overrides are always applied on top.
    pub fn load() -> anyhow::Result<Self> {
        let candidates = ["orchestrator.toml", "/etc/fixagent/orchestrator.toml"];
        let mut config = Self::default();

        for path in &candidates {
            let p = std::path::Path::new(path);
            if p.exists() {
                let content = std::fs::read_to_string(p)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path, e))?;
                config = toml::from_str(&content)
                    .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path, e))?;
                tracing::info!("Loaded orchestrator config from {}", path);
                break;
            }
        }

        config.apply_env_overrides();
        Ok(config)
    }

    /// Apply environment variable overrides.
    ///
    /// This keeps backward-compatibility with container deployments that pass
    /// `DATABASE_URL` etc. via `docker-compose.yml` environment.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("DATABASE_URL") {
            self.database.url = v;
        }
        if let Ok(v) = std::env::var("SERVER_HOST") {
            self.server.host = v;
        }
        if let Ok(v) = std::env::var("SERVER_PORT") {
            if let Ok(port) = v.parse::<u16>() {
                self.server.port = port;
            }
        }
        if let Ok(v) = std::env::var("LLM_PROVIDER") {
            self.llm.provider = v;
        }
        if let Ok(v) = std::env::var("LLM_MODEL") {
            self.llm.model = v;
        }
        if let Ok(v) = std::env::var("LLM_BASE_URL") {
            self.llm.base_url = Some(v);
        }
        if let Ok(v) = std::env::var("LLM_API_KEY") {
            self.llm.api_key = Some(v);
        }
        // Backward-compat: honor OPENAI_* env vars
        if let Ok(v) = std::env::var("OPENAI_BASE_URL") {
            self.llm.base_url = Some(v);
        }
        if let Ok(v) = std::env::var("OPENAI_API_KEY") {
            self.llm.api_key = Some(v);
        }
    }

    /// Build a ReviewAgent config from the Orchestrator's LLM settings.
    /// Used when ReviewAgent is invoked from within the Orchestrator process.
    pub fn to_reviewagent_llm_overrides(&self) -> Option<ReviewAgentLlmOverrides> {
        // Only produce overrides if at least one LLM field is configured
        if self.llm.api_key.is_some() || self.llm.base_url.is_some() {
            Some(ReviewAgentLlmOverrides {
                provider: self.llm.provider.clone(),
                model: self.llm.model.clone(),
                base_url: self.llm.base_url.clone(),
                api_key: self.llm.api_key.clone(),
            })
        } else {
            None
        }
    }
}

/// Subset of LLM config that gets applied to ReviewAgent/FixAgent configs.
pub struct ReviewAgentLlmOverrides {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

/// Apply Orchestrator LLM overrides to a ReviewAgent config, replacing the
/// old `apply_workspace_env_overrides` approach that read from `env` files.
pub fn apply_llm_overrides(
    config: &mut reviewagent::config::Config,
    overrides: &ReviewAgentLlmOverrides,
) {
    config.llm.provider = overrides.provider.clone();
    config.llm.model = overrides.model.clone();
    if let Some(ref url) = overrides.base_url {
        config.llm.base_url = Some(url.clone());
        config.llm_lite.base_url = Some(url.clone());
    }
    if let Some(ref key) = overrides.api_key {
        config.llm.api_key = Some(key.clone());
        config.llm_lite.api_key = Some(key.clone());
    }
}
