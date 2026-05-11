//! Prompt building for the fix agent.

use reviewagent::llm::Issue;

/// System prompt for the issue verification phase.
pub const VERIFY_SYSTEM_PROMPT: &str = r#"
You are a code verification agent. Your task is to verify whether a reported issue actually exists in the current codebase.

## Available Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `search` | Search codebase - regex search contents or glob find files | Need to find related code, call chains, type definitions |
| `read_file` | Read file contents, single or batch | Need to view specific files for context |

## Verification Strategy

1. **Read the reported file** - Check the specific lines mentioned in the issue
2. **Search for related code** - Find call chains, type definitions, related modules
3. **Assess the issue** - Determine if the reported problem is actually present

## Output Format

Return a JSON object with these fields:
- exists: boolean - true if the issue is confirmed to exist, false otherwise
- confidence: number 0-100 - how confident you are in your assessment
- findings: string - summary of what you found (max 500 chars)
- related_files: array of strings - files you examined that are relevant

## Rules

- Be objective. Do not assume the issue exists just because it was reported.
- If the code has already been fixed or the issue description does not match reality, return exists=false.
- If you cannot determine conclusively, return exists=true but with low confidence.
- Always use the tools to verify; do not rely solely on the issue description.
"#;

/// Build the verification prompt for an issue.
pub fn build_verification_prompt(issue: &Issue) -> String {
    format!(
        r#"Verify whether the following issue actually exists in the codebase.

Issue Details:
- File: {file}
- Line: {line}
- End line: {end_line}
- Title: {title}
- Description: {description}
- Suggested fix: {suggestion}

Please:
1. Read the reported file around the mentioned lines
2. Search for related code to understand the full context
3. Determine if the issue description accurately describes the current code
4. Return your findings in the specified JSON format
"#,
        file = issue.file,
        line = issue.line,
        end_line = issue.end_line.unwrap_or(issue.line),
        title = issue.title,
        description = issue.description,
        suggestion = issue.suggestion,
    )
}

/// System prompt for the fix agent.
pub const FIX_SYSTEM_PROMPT: &str = r#"
You are a minimal code fix agent with code exploration capabilities.

Your task is to fix exactly one reported issue with the smallest safe code change.

## Available Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `search` | Search codebase - regex search contents or glob find files | Need to find related code, call chains, type definitions |
| `read_file` | Read file contents, single or batch | Need to view specific files for context |

## Exploration Strategy

Before generating a fix, you may explore the codebase to gather context:

1. **Search first** - Use `search` to find related code locations
2. **Read context** - Use `read_file` to view relevant files
3. **Understand impact** - Check call chains, type definitions, related modules
4. **Generate fix** - Apply the minimal change

**Do not explore when:**
- The issue is straightforward and the file context is sufficient
- The fix is a simple typo or obvious logic error

## Fix Rules

- Only modify the file identified by the issue.
- Only modify the code range necessary to resolve the issue.
- Do not refactor unrelated code.
- Preserve formatting style already present in the file.
- If the issue is not safe to fix automatically, return outcome=needs_human.
- If the issue appears invalid, return outcome=invalid_candidate.
- Return only structured data.
"#;

/// Build the fix prompt with issue details and file context.
pub fn build_fix_prompt(issue: &Issue, file_content: &str, context_lines: usize) -> String {
    let total_lines = file_content.lines().count();
    let issue_end = issue.end_line.unwrap_or(issue.line);
    let start = issue.line.saturating_sub(context_lines).max(1);
    let end = (issue_end + context_lines).min(total_lines.max(1));
    let snippet = slice_lines(file_content, start, end);

    format!(
        r#"Issue:
- Severity: {severity:?}
- File: {file}
- Line: {line}
- End line: {end_line}
- Title: {title}
- Description: {description}
- Suggested fix: {suggestion}
- Confidence: {confidence:?}

Constraints:
- Keep the fix minimal and localized.
- Replacement must cover only the necessary line range.
- If the safest action is to avoid automatic changes, return needs_human.

File excerpt ({start}-{end} of {total_lines}):
```text
{snippet}
```
"#,
        severity = issue.severity,
        file = issue.file,
        line = issue.line,
        end_line = issue.end_line.unwrap_or(issue.line),
        title = issue.title,
        description = issue.description,
        suggestion = issue.suggestion,
        confidence = issue.confidence,
        start = start,
        end = end,
        total_lines = total_lines,
        snippet = snippet,
    )
}

/// Build an exploration prompt when the fix agent needs to explore before fixing.
pub fn build_exploration_prompt(issue: &Issue) -> String {
    format!(
        r#"Before fixing this issue, explore the codebase to understand the context.

Issue: {title}
File: {file}
Line: {line}
Description: {description}

Please:
1. Search for related functions, types, and call chains
2. Read relevant files to understand the full context
3. Identify any dependencies or impacts of the proposed fix
4. Return a concise summary of your findings (max 500 chars)

Focus on understanding:
- What this code does and why the issue exists
- How the fix might affect other parts of the codebase
- Whether the suggested fix is appropriate
"#,
        title = issue.title,
        file = issue.file,
        line = issue.line,
        description = issue.description,
    )
}

pub const CONFIRM_SYSTEM_PROMPT: &str = r#"
You are a code fix confirmation agent. Your task is to verify that a fix was correctly applied.

## Available Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `read_file` | Read file contents | Need to view the patched file |

## Confirmation Task

After a fix is applied, you must:
1. Read the patched file around the modified lines
2. Compare the actual code with what was intended
3. Verify the issue described has been resolved

## Output Format

Return a JSON object with these fields:
- confirmed: boolean - true if the fix was correctly applied and resolves the issue
- confidence: number 0-100 - how confident you are in your assessment
- findings: string - explanation of what you found (max 500 chars)

## Rules

- Be objective and thorough
- Read the actual file content to verify, do not guess
- If the fix was partially applied or introduced new issues, return confirmed=false
- If you cannot determine conclusively, return confirmed=false with low confidence
"#;

pub fn build_confirmation_prompt(
    issue: &Issue,
    file_path: &str,
    start_line: usize,
    end_line: usize,
    expected_replacement: &str,
) -> String {
    format!(
        r#"Please verify the following fix was correctly applied.

Issue:
- File: {file_path}
- Line: {start_line} - {end_line}
- Title: {title}
- Description: {description}
- Suggested fix: {suggestion}

Expected change:
```text
{expected_replacement}
```

Please read the file around lines {start_line}-{end_line} and confirm whether the fix was applied correctly and resolves the issue.
"#,
        file_path = file_path,
        start_line = start_line,
        end_line = end_line,
        title = issue.title,
        description = issue.description,
        suggestion = issue.suggestion,
        expected_replacement = expected_replacement,
    )
}

/// Extract a slice of lines from content with line numbers.
fn slice_lines(content: &str, start: usize, end: usize) -> String {
    content
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line_no = idx + 1;
            if line_no >= start && line_no <= end {
                Some(format!("{:>4}: {}", line_no, line))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
