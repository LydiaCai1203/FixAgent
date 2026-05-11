use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct FixAgentConfig {
    pub fix: FixConfig,
    pub agent: AgentConfig,
}

impl Default for FixAgentConfig {
    fn default() -> Self {
        Self {
            fix: FixConfig::default(),
            agent: AgentConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct FixConfig {
    pub context_lines: usize,
    pub max_replacement_lines: usize,
}

impl Default for FixConfig {
    fn default() -> Self {
        Self {
            context_lines: 20,
            max_replacement_lines: 120,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AgentConfig {
    /// Enable agent mode with tool calling
    pub enabled: bool,
    /// Maximum iterations (LLM round-trips)
    pub max_iterations: usize,
    /// Maximum tool calls per session
    pub max_tool_calls: usize,
    /// Enable issue verification before attempting fix
    pub verify_before_fix: bool,
    /// Maximum iterations for the verification explorer subagent
    pub verify_max_iterations: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: 20,
            max_tool_calls: 20,
            verify_before_fix: true,
            verify_max_iterations: 30,
        }
    }
}

impl FixAgentConfig {
    pub fn load_or_default(repo_dir: &Path) -> anyhow::Result<Self> {
        for candidate in [".fixagent.toml", "fixagent.toml"] {
            let path = repo_dir.join(candidate);
            if path.exists() {
                return Self::load(&path);
            }
        }
        Ok(Self::default())
    }

    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file: {} - {}", path.display(), e))?;
        let config: FixAgentConfig = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file: {} - {}", path.display(), e))?;
        Ok(config)
    }
}

pub fn load_reviewagent_config(repo_dir: &Path) -> anyhow::Result<reviewagent::config::Config> {
    for candidate in [".reviewagent.toml", "reviewagent.toml"] {
        let path = repo_dir.join(candidate);
        if path.exists() {
            let mut config = reviewagent::config::Config::load(&path)?;
            apply_workspace_env_overrides(repo_dir, &mut config)?;
            return Ok(config);
        }
    }
    let mut config = reviewagent::config::Config::default();
    apply_workspace_env_overrides(repo_dir, &mut config)?;
    Ok(config)
}

fn apply_workspace_env_overrides(repo_dir: &Path, config: &mut reviewagent::config::Config) -> anyhow::Result<()> {
    let env_path = repo_dir.join("env");
    if env_path.exists() {
        let content = fs::read_to_string(&env_path)
            .map_err(|e| anyhow::anyhow!("Failed to read env file: {} - {}", env_path.display(), e))?;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
            match key.trim().to_ascii_lowercase().as_str() {
                "baseurl" => {
                    config.llm.base_url = Some(value.clone());
                    config.llm_lite.base_url = Some(value);
                }
                "apikey" => {
                    config.llm.api_key = Some(value.clone());
                    config.llm_lite.api_key = Some(value);
                }
                "model" => {
                    config.llm.model = value.clone();
                    config.llm_lite.model = value;
                }
                _ => {}
            }
        }
    }

    if let Ok(value) = std::env::var("OPENAI_BASE_URL") {
        config.llm.base_url = Some(value.clone());
        config.llm_lite.base_url = Some(value);
    }

    if let Ok(value) = std::env::var("OPENAI_API_KEY") {
        config.llm.api_key = Some(value.clone());
        config.llm_lite.api_key = Some(value);
    }

    if let Ok(value) = std::env::var("MODEL") {
        config.llm.model = value.clone();
        config.llm_lite.model = value;
    }

    Ok(())
}
