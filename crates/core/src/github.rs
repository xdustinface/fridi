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

/// Detect the GitHub `owner/repo` from the current directory's git remote.
pub fn detect_repo() -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_repo_from_url(&url)
}

pub(crate) fn parse_repo_from_url(url: &str) -> Option<String> {
    // ssh:// scheme: ssh://git@github.com/owner/repo.git
    if url.starts_with("ssh://") {
        let parts: Vec<&str> = url.trim_end_matches(".git").split('/').collect();
        // ssh://git@host/owner/repo => ["ssh:", "", "git@host", "owner", "repo"]
        if parts.len() >= 5 {
            let owner = parts[parts.len() - 2];
            let repo = parts[parts.len() - 1];
            if !owner.is_empty() && !repo.is_empty() {
                return Some(format!("{owner}/{repo}"));
            }
        }
        return None;
    }
    // SCP-style SSH: git@github.com:owner/repo.git
    // Only match if there is no "://" (which would indicate a scheme-based URL)
    if url.contains('@') && !url.contains("://") {
        if let Some(colon_pos) = url.rfind(':') {
            let path = &url[colon_pos + 1..];
            let cleaned = path.trim_end_matches(".git");
            if cleaned.contains('/') {
                return Some(cleaned.to_string());
            }
        }
    }
    // HTTPS: https://github.com/owner/repo.git
    if url.starts_with("http://") || url.starts_with("https://") {
        let parts: Vec<&str> = url.trim_end_matches(".git").split('/').collect();
        // https://github.com/owner/repo => ["https:", "", "github.com", "owner", "repo"]
        if parts.len() >= 5 {
            let owner = parts[parts.len() - 2];
            let repo = parts[parts.len() - 1];
            if !owner.is_empty() && !repo.is_empty() {
                return Some(format!("{owner}/{repo}"));
            }
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo_from_ssh_url() {
        assert_eq!(
            parse_repo_from_url("git@github.com:owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_repo_from_ssh_url_no_suffix() {
        assert_eq!(
            parse_repo_from_url("git@github.com:owner/repo"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_repo_from_ssh_custom_alias() {
        assert_eq!(
            parse_repo_from_url("git@github-dust:owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_repo_from_https_url() {
        assert_eq!(
            parse_repo_from_url("https://github.com/owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_repo_from_https_url_no_suffix() {
        assert_eq!(
            parse_repo_from_url("https://github.com/owner/repo"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_repo_from_ssh_scheme_url() {
        assert_eq!(
            parse_repo_from_url("ssh://git@github.com/owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_repo_from_ssh_scheme_no_suffix() {
        assert_eq!(
            parse_repo_from_url("ssh://git@github.com/owner/repo"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_repo_from_https_with_credentials() {
        assert_eq!(
            parse_repo_from_url("https://user@github.com/owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_repo_rejects_malformed_https() {
        // Too few path segments
        assert_eq!(parse_repo_from_url("https://github.com"), None);
        assert_eq!(parse_repo_from_url("https://github.com/owner"), None);
    }

    #[test]
    fn test_parse_repo_from_invalid_url() {
        assert_eq!(parse_repo_from_url("not-a-url"), None);
    }
}
