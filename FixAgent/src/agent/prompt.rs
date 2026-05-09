//! Prompt building for the fix agent.

use reviewagent::llm::Issue;

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
