//! Fix agent: main code fix agent with tool calling.
//!
//! `FixAgent` encapsulates the LLM client and workspace configuration.
//! Its `run()` method executes a multi-turn agent session with search and read_file tools,
//! then extracts a structured `FixPatch`.

use std::path::PathBuf;

use rig::client::CompletionClient;

use crate::config::FixAgentConfig;
use crate::error::{FixError, Result};
use crate::models::FixPatch;
use crate::tools::{ReadFileTool, SearchContext, SearchTool};
use reviewagent::llm::{LlmCapabilities, LlmClient, additional_model_params, recommended_temperature};

/// Main code fix agent with search and file-reading tools.
#[derive(Clone)]
pub struct FixAgent {
    config: FixAgentConfig,
    llm: LlmClient,
    workspace_root: PathBuf,
    max_iterations: usize,
}

impl FixAgent {
    pub fn new(
        config: FixAgentConfig,
        llm: LlmClient,
        workspace_root: PathBuf,
    ) -> Result<Self> {
        let max_iterations = config.agent.max_iterations;
        Ok(Self {
            config,
            llm,
            workspace_root,
            max_iterations,
        })
    }

    /// Run the fix agent and return a structured fix patch.
    pub async fn run(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<FixPatch> {
        tracing::info!("Starting FixAgent with tool calling");

        // Create tools
        let search_ctx = SearchContext::new(self.workspace_root.clone());
        let search_tool = SearchTool::new(search_ctx);
        let read_file_tool = ReadFileTool::new(self.workspace_root.clone());

        // Agent phase: dispatch on concrete client for rig agent builder
        let agent_response = self
            .run_agent_phase(system_prompt, user_prompt, search_tool, read_file_tool)
            .await?;

        tracing::info!("Agent phase completed, extracting structured patch");

        // Extraction phase: uses LlmClient.extract() directly
        let patch = self.extract_structured_patch(&agent_response).await?;

        Ok(patch)
    }

    /// Run the fix agent in simple mode (no tools, direct extraction).
    /// This is the fallback mode when tool calling is not needed or not available.
    pub async fn run_simple(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<FixPatch> {
        tracing::info!("Starting FixAgent in simple mode (no tools)");

        self.llm
            .extract::<FixPatch>(system_prompt, user_prompt)
            .await
            .map_err(|e| FixError::Llm(format!("Failed to extract patch: {}", e)))
    }

    /// Build and run the agent phase, dispatching on the LlmClient variant.
    async fn run_agent_phase(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        search_tool: SearchTool,
        read_file_tool: ReadFileTool,
    ) -> Result<String> {
        let caps = self.llm.capabilities();

        match &self.llm {
            LlmClient::Anthropic {
                client,
                model,
                max_tokens,
            } => {
                build_and_run_agent(
                    client,
                    model,
                    Some(*max_tokens),
                    &caps,
                    system_prompt,
                    user_prompt,
                    search_tool,
                    read_file_tool,
                    self.max_iterations,
                )
                .await
            }
            LlmClient::OpenAI { client, model } => {
                build_and_run_agent(
                    client,
                    model,
                    None,
                    &caps,
                    system_prompt,
                    user_prompt,
                    search_tool,
                    read_file_tool,
                    self.max_iterations,
                )
                .await
            }
            LlmClient::Codex { client, model } => {
                build_and_run_agent(
                    client,
                    model,
                    None,
                    &caps,
                    system_prompt,
                    user_prompt,
                    search_tool,
                    read_file_tool,
                    self.max_iterations,
                )
                .await
            }
        }
    }

    /// Extract structured FixPatch from agent output.
    async fn extract_structured_patch(&self,
        agent_response: &str,
    ) -> Result<FixPatch> {
        // First try to parse the response directly as JSON
        if let Ok(patch) = serde_json::from_str::<FixPatch>(agent_response) {
            tracing::info!("Agent response was already valid JSON");
            return Ok(patch);
        }

        // Try to extract JSON from markdown code block
        if let Some(json_str) = extract_json(agent_response) {
            if let Ok(patch) = serde_json::from_str::<FixPatch>(json_str) {
                tracing::info!("Extracted JSON from markdown code block");
                return Ok(patch);
            }
        }

        // Use LlmClient extractor for structured output with retry
        tracing::info!("Using rig extractor for structured output");

        let extraction_prompt = format!(
            "Based on the following code fix analysis, provide a structured fix patch:\n\n{}",
            agent_response
        );

        let mut retries = 0u32;
        let max_retries = 3u32;

        loop {
            match self
                .llm
                .extract::<FixPatch>(EXTRACTOR_PROMPT, &extraction_prompt)
                .await
            {
                Ok(patch) => {
                    return Ok(patch);
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let is_retryable = err_msg.contains("504")
                        || err_msg.contains("502")
                        || err_msg.contains("503")
                        || err_msg.contains("timeout");

                    if is_retryable && retries < max_retries {
                        retries += 1;
                        let delay = std::time::Duration::from_secs(2u64.pow(retries));
                        tracing::warn!(
                            "Retrying after {} seconds (attempt {}/{})",
                            delay.as_secs(),
                            retries,
                            max_retries
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        return Err(FixError::Llm(format!(
                            "Failed to extract structured patch: {}",
                            e
                        )));
                    }
                }
            }
        }
    }
}

/// Generic agent builder + streaming runner.
#[allow(clippy::too_many_arguments)]
async fn build_and_run_agent<C>(
    client: &C,
    model: &str,
    max_tokens: Option<u64>,
    caps: &LlmCapabilities,
    system_prompt: &str,
    user_prompt: &str,
    search_tool: SearchTool,
    read_file_tool: ReadFileTool,
    max_iterations: usize,
) -> Result<String>
where
    C: CompletionClient,
    C::CompletionModel: rig::completion::CompletionModel + 'static,
    <C::CompletionModel as rig::completion::CompletionModel>::StreamingResponse:
        rig::completion::GetTokenUsage,
{
    tracing::info!("Building Rig agent with search + read_file tools");

    let mut builder = client.agent(model);

    if let Some(mt) = max_tokens {
        builder = builder.max_tokens(mt);
    }

    if let Some(params) = additional_model_params(model) {
        builder = builder.additional_params(params);
    }

    let prompt = if caps.is_codex {
        builder = builder.additional_params(serde_json::json!({
            "reasoning": { "effort": "medium", "summary": "auto" }
        }));
        format!("{}\n\n{}", system_prompt, user_prompt)
    } else {
        builder = builder.preamble(system_prompt);
        user_prompt.to_string()
    };

    let agent = builder
        .tool(search_tool)
        .tool(read_file_tool)
        .temperature(recommended_temperature(model))
        .build();

    let idle_timeout = std::time::Duration::from_secs(120);
    reviewagent::llm::streaming(&agent, &prompt, max_iterations, idle_timeout,
    )
    .await
    .map_err(|e| FixError::Agent(format!("Agent execution failed: {}", e)))
}

/// Extract JSON from a string that might have markdown code blocks.
fn extract_json(text: &str) -> Option<&str> {
    // Try to find JSON in code blocks
    if let Some(start) = text.find("```json") {
        let content_start = start + 7;
        if let Some(end) = text[content_start..].find("```") {
            return Some(text[content_start..content_start + end].trim());
        }
    }

    // Try plain code blocks
    if let Some(start) = text.find("```") {
        let content_start = start + 3;
        let content_start = text[content_start..]
            .find('\n')
            .map(|i| content_start + i + 1)
            .unwrap_or(content_start);
        if let Some(end) = text[content_start..].find("```") {
            return Some(text[content_start..content_start + end].trim());
        }
    }

    // Try to find raw JSON
    if let Some(start) = text.find('{')
        && let Some(end) = text.rfind('}')
    {
        return Some(&text[start..=end]);
    }

    None
}

/// Prompt for the extractor to convert analysis to structured output.
const EXTRACTOR_PROMPT: &str = r#"You are a code fix formatting assistant. Convert the given code fix analysis into a structured JSON format.

## Output Format

Your response MUST be a valid JSON object in a markdown code block:
```json
{
  "summary": "Brief description of the fix (max 100 chars)",
  "outcome": "apply",
  "file": "path/to/file.py",
  "start_line": 10,
  "end_line": 15,
  "replacement": "complete replacement code",
  "rationale": "Why this fix is correct",
  "verification_steps": ["step 1", "step 2"]
}
```

## Field Requirements

- summary: Brief description of the fix (max 100 chars)
- outcome: "apply" | "needs_human" | "invalid_candidate"
  - apply: The fix is safe and minimal
  - needs_human: The fix requires human judgment
  - invalid_candidate: The issue is not valid or cannot be fixed
- file: File path to modify
- start_line: Starting line number (1-based, must be > 0)
- end_line: Ending line number (1-based, must be >= start_line)
- replacement: The replacement code (complete, no placeholders)
- rationale: Why this fix is correct
- verification_steps: Array of strings describing how to verify the fix

## Rules

1. Return ONLY the JSON code block - no explanation or text outside the code block
2. replacement must contain COMPLETE code - no `// ...`, `// keep existing`, `// omitted` placeholders
3. Only modify the necessary lines - if only one line changes, start_line == end_line
4. Preserve original indentation exactly
5. If unsure about the fix, return outcome=needs_human
"#;
