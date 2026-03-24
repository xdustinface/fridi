use std::process::Command;

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub labels: Vec<GitHubLabel>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct GitHubPR {
    pub number: u64,
    pub title: String,
    #[serde(rename = "headRefName")]
    pub head_ref_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("failed to run gh CLI: {0}")]
    Exec(#[from] std::io::Error),
    #[error("gh CLI returned non-zero exit code: {stderr}")]
    NonZero { stderr: String },
    #[error("failed to parse gh output: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Fetch open issues for a repository using the `gh` CLI.
pub fn fetch_issues(repo: &str) -> Result<Vec<GitHubIssue>, GitHubError> {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--json",
            "number,title,labels",
            "--limit",
            "50",
        ])
        .output()?;

    if !output.status.success() {
        return Err(GitHubError::NonZero {
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let issues: Vec<GitHubIssue> = serde_json::from_slice(&output.stdout)?;
    Ok(issues)
}

/// Fetch open pull requests for a repository using the `gh` CLI.
pub fn fetch_prs(repo: &str) -> Result<Vec<GitHubPR>, GitHubError> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--json",
            "number,title,headRefName",
            "--limit",
            "50",
        ])
        .output()?;

    if !output.status.success() {
        return Err(GitHubError::NonZero {
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let prs: Vec<GitHubPR> = serde_json::from_slice(&output.stdout)?;
    Ok(prs)
}

/// Select the highest priority open issue. Prioritizes by label name containing
/// "priority", "urgent", or "critical", then falls back to the first issue in
/// the list (which gh returns sorted by creation date).
pub fn auto_pick_issue(repo: &str) -> Result<Option<GitHubIssue>, GitHubError> {
    let issues = fetch_issues(repo)?;
    if issues.is_empty() {
        return Ok(None);
    }

    let priority_issue = issues.iter().find(|issue| {
        issue.labels.iter().any(|l| {
            let lower = l.name.to_lowercase();
            lower.contains("priority") || lower.contains("urgent") || lower.contains("critical")
        })
    });

    Ok(Some(priority_issue.unwrap_or(&issues[0]).clone()))
}
