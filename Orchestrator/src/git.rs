use crate::error::{OrchestratorError, Result};
use std::path::PathBuf;

/// Commit all staged changes and push to remote.
/// Returns the commit SHA on success, or None if there was nothing to commit.
pub async fn git_commit_and_push(
    repo_dir: &PathBuf,
    issue_id: i64,
    title: &str,
) -> std::result::Result<Option<String>, String> {
    configure_git_user(repo_dir).await?;

    run_git(repo_dir, &["add", "-A"]).await?;

    let commit_msg = format!(
        "fix: {} (Issue #{})\n\nAutomated fix by FixAgent",
        title, issue_id
    );
    let output = tokio::process::Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(repo_dir)
        .output()
        .await
        .map_err(|e| format!("git commit failed: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("nothing to commit") {
            tracing::info!("No changes to commit for issue {}", issue_id);
            return Ok(None);
        }
        return Err(format!("git commit failed: {}", stderr));
    }

    tracing::info!("Committed fix for issue {}: {}", issue_id, title);

    let commit_sha = get_head_sha(repo_dir).await;

    run_git(repo_dir, &["push"]).await?;

    tracing::info!("Pushed fix for issue {} to remote", issue_id);
    Ok(commit_sha)
}

/// Squash all FixAgent commits on the current branch into a single commit.
///
/// Strategy:
/// 1. Find the merge-base between current HEAD and the upstream default branch
///    (tries origin/main, then origin/master).
/// 2. Soft-reset to the merge-base (keeps all changes staged).
/// 3. Create a single squashed commit.
/// 4. Force-push to the remote tracking branch.
pub async fn squash_fix_commits(repo_dir: &PathBuf) -> Result<()> {
    configure_git_user(repo_dir)
        .await
        .map_err(|e| OrchestratorError::Git(e))?;

    let base_ref = find_upstream_base_ref(repo_dir)
        .await
        .map_err(|e| OrchestratorError::Git(e))?;

    let merge_base = get_merge_base(repo_dir, &base_ref)
        .await
        .map_err(|e| OrchestratorError::Git(e))?;

    let commit_count = count_commits_ahead(repo_dir, &merge_base)
        .await
        .map_err(|e| OrchestratorError::Git(e))?;

    if commit_count <= 1 {
        tracing::info!(
            "Only {} commit(s) ahead of base -- nothing to squash",
            commit_count
        );
        return Ok(());
    }

    tracing::info!(
        "Squashing {} commits (merge-base: {}, base_ref: {})",
        commit_count,
        &merge_base,
        &base_ref
    );

    // Soft-reset to the merge-base -- keeps all changes staged
    run_git(repo_dir, &["reset", "--soft", &merge_base])
        .await
        .map_err(|e| OrchestratorError::Git(e))?;

    // Create squashed commit
    let squash_msg = format!(
        "fix: squashed {} FixAgent commits\n\nAutomated squash by FixAgent before merge.",
        commit_count
    );
    let output = tokio::process::Command::new("git")
        .args(["commit", "-m", &squash_msg])
        .current_dir(repo_dir)
        .output()
        .await
        .map_err(|e| OrchestratorError::Git(format!("git commit (squash) failed: {}", e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("nothing to commit") {
            tracing::info!("Nothing to commit after squash reset -- branch may already be clean");
            return Ok(());
        }
        return Err(OrchestratorError::Git(format!(
            "git commit (squash) failed: {}",
            stderr
        )));
    }

    // Force-push to remote
    run_git(repo_dir, &["push", "--force-with-lease"])
        .await
        .map_err(|e| OrchestratorError::Git(e))?;

    tracing::info!(
        "Successfully squashed {} commits and force-pushed",
        commit_count
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn configure_git_user(repo_dir: &PathBuf) -> std::result::Result<(), String> {
    run_git(repo_dir, &["config", "user.email", "fixagent@monkeycode.ai"]).await?;
    run_git(repo_dir, &["config", "user.name", "FixAgent"]).await?;
    Ok(())
}

async fn run_git(repo_dir: &PathBuf, args: &[&str]) -> std::result::Result<(), String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .await
        .map_err(|e| format!("git {} failed: {}", args.join(" "), e))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

async fn get_head_sha(repo_dir: &PathBuf) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .await
        .ok()?;
    if output.status.success() {
        Some(
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string(),
        )
    } else {
        None
    }
}

async fn find_upstream_base_ref(repo_dir: &PathBuf) -> std::result::Result<String, String> {
    let check_main = tokio::process::Command::new("git")
        .args(["rev-parse", "--verify", "origin/main"])
        .current_dir(repo_dir)
        .output()
        .await
        .map_err(|e| format!("git rev-parse origin/main failed: {}", e))?;
    if check_main.status.success() {
        return Ok("origin/main".to_string());
    }

    let check_master = tokio::process::Command::new("git")
        .args(["rev-parse", "--verify", "origin/master"])
        .current_dir(repo_dir)
        .output()
        .await
        .map_err(|e| format!("git rev-parse origin/master failed: {}", e))?;
    if check_master.status.success() {
        return Ok("origin/master".to_string());
    }

    Err("Could not find origin/main or origin/master as base branch".to_string())
}

async fn get_merge_base(
    repo_dir: &PathBuf,
    base_ref: &str,
) -> std::result::Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(["merge-base", "HEAD", base_ref])
        .current_dir(repo_dir)
        .output()
        .await
        .map_err(|e| format!("git merge-base failed: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "git merge-base failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string())
}

async fn count_commits_ahead(
    repo_dir: &PathBuf,
    merge_base: &str,
) -> std::result::Result<usize, String> {
    let output = tokio::process::Command::new("git")
        .args(["rev-list", "--count", &format!("{}..HEAD", merge_base)])
        .current_dir(repo_dir)
        .output()
        .await
        .map_err(|e| format!("git rev-list failed: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0))
}
