use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct FixAgentConfig {
    pub fix: FixConfig,
}

impl Default for FixAgentConfig {
    fn default() -> Self {
        Self {
            fix: FixConfig::default(),
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

impl FixAgentConfig {
    pub fn load_or_default(repo_dir: &Path) -> Result<Self> {
        for candidate in [".fixagent.toml", "fixagent.toml"] {
            let path = repo_dir.join(candidate);
            if path.exists() {
                return Self::load(&path);
            }
        }
        Ok(Self::default())
    }

    pub fn load(path: &PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }
}

pub fn load_reviewagent_config(repo_dir: &Path) -> Result<reviewagent::config::Config> {
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

fn apply_workspace_env_overrides(repo_dir: &Path, config: &mut reviewagent::config::Config) -> Result<()> {
    let env_path = repo_dir.join("env");
    if !env_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&env_path)
        .with_context(|| format!("Failed to read env file: {}", env_path.display()))?;

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

    Ok(())
}
