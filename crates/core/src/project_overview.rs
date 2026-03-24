use std::path::Path;
use std::process::Command;

use chrono::{NaiveDate, Utc};

use crate::github::{self, CiStatus, GitHubError};
use crate::session::{SessionStatus, SessionStore, SessionStoreError};

#[derive(Debug, thiserror::Error)]
pub enum OverviewError {
    #[error("github error: {0}")]
    GitHub(#[from] GitHubError),
    #[error("session store error: {0}")]
    Session(#[from] SessionStoreError),
    #[error("git error: {0}")]
    Git(String),
}

#[derive(Debug, Clone)]
pub struct ProjectOverview {
    pub open_prs: Vec<PrSummary>,
    pub open_issues: Vec<IssueSummary>,
    pub active_branches: Vec<BranchInfo>,
    pub stale_branches: Vec<BranchInfo>,
    pub running_sessions: usize,
}

#[derive(Debug, Clone)]
pub struct PrSummary {
    pub number: u64,
    pub title: String,
    pub branch: String,
    pub ci_status: CiStatus,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct IssueSummary {
    pub number: u64,
    pub title: String,
    pub labels: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub last_commit_date: String,
    pub last_commit_message: String,
}

const STALE_THRESHOLD_DAYS: i64 = 14;

/// Fetch a complete project overview for the dashboard.
///
/// `repo` is the GitHub `owner/repo` slug. `work_dir` is the local git checkout
/// used to query branches. `session_store` provides running-session counts.
pub fn fetch_project_overview(
    repo: &str,
    work_dir: &Path,
    session_store: &SessionStore,
) -> Result<ProjectOverview, OverviewError> {
    let open_prs = fetch_pr_summaries(repo)?;
    let open_issues = fetch_issue_summaries(repo)?;
    let (active_branches, stale_branches) = fetch_branches(work_dir)?;
    let running_sessions = count_running_sessions(session_store)?;

    Ok(ProjectOverview {
        open_prs,
        open_issues,
        active_branches,
        stale_branches,
        running_sessions,
    })
}

fn fetch_pr_summaries(repo: &str) -> Result<Vec<PrSummary>, GitHubError> {
    let prs = github::fetch_prs(repo)?;
    Ok(prs
        .into_iter()
        .map(|pr| PrSummary {
            number: pr.number,
            title: pr.title,
            branch: pr.head_ref_name,
            ci_status: CiStatus::from_checks(&pr.status_check_rollup),
            updated_at: pr.updated_at,
        })
        .collect())
}

fn fetch_issue_summaries(repo: &str) -> Result<Vec<IssueSummary>, GitHubError> {
    let issues = github::fetch_issues(repo)?;
    Ok(issues
        .into_iter()
        .map(|issue| IssueSummary {
            number: issue.number,
            title: issue.title,
            labels: issue.labels.into_iter().map(|l| l.name).collect(),
            updated_at: issue.updated_at,
        })
        .collect())
}

fn fetch_branches(work_dir: &Path) -> Result<(Vec<BranchInfo>, Vec<BranchInfo>), OverviewError> {
    let output = Command::new("git")
        .args([
            "branch",
            "--sort=-committerdate",
            "--format=%(refname:short)\t%(committerdate:iso)\t%(subject)",
        ])
        .current_dir(work_dir)
        .output()
        .map_err(|e| OverviewError::Git(e.to_string()))?;

    if !output.status.success() {
        return Err(OverviewError::Git(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    let today = Utc::now().date_naive();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut active = Vec::new();
    let mut stale = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let name = parts[0].trim();

        if matches!(name, "main" | "master" | "HEAD") {
            continue;
        }

        let date_str = parts[1].trim();
        let message = parts[2].trim();

        let info = BranchInfo {
            name: name.to_string(),
            last_commit_date: date_str.to_string(),
            last_commit_message: message.to_string(),
        };

        if is_stale(date_str, today) {
            stale.push(info);
        } else {
            active.push(info);
        }
    }

    Ok((active, stale))
}

/// A branch is stale if its last commit is older than `STALE_THRESHOLD_DAYS`.
fn is_stale(iso_date: &str, today: NaiveDate) -> bool {
    let date_part = iso_date.split_whitespace().next().unwrap_or("");
    match NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
        Ok(d) => (today - d).num_days() >= STALE_THRESHOLD_DAYS,
        Err(_) => false,
    }
}

fn count_running_sessions(store: &SessionStore) -> Result<usize, SessionStoreError> {
    let summaries = store.list()?;
    Ok(summaries
        .iter()
        .filter(|s| s.status == SessionStatus::Running)
        .count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_stale_recent_date() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 24).unwrap();
        assert!(!is_stale("2026-03-20 10:00:00 +0000", today));
    }

    #[test]
    fn test_is_stale_old_date() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 24).unwrap();
        assert!(is_stale("2026-03-01 10:00:00 +0000", today));
    }

    #[test]
    fn test_is_stale_exactly_threshold() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 24).unwrap();
        assert!(is_stale("2026-03-10 10:00:00 +0000", today));
    }

    #[test]
    fn test_is_stale_one_day_before_threshold() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 24).unwrap();
        assert!(!is_stale("2026-03-11 10:00:00 +0000", today));
    }

    #[test]
    fn test_is_stale_invalid_date() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 24).unwrap();
        assert!(!is_stale("not-a-date", today));
    }

    #[test]
    fn test_ci_status_from_empty_checks() {
        assert_eq!(CiStatus::from_checks(&[]), CiStatus::None);
    }

    #[test]
    fn test_ci_status_all_passed() {
        use crate::github::StatusCheck;
        let checks = vec![
            StatusCheck {
                status: "completed".into(),
                conclusion: "success".into(),
            },
            StatusCheck {
                status: "completed".into(),
                conclusion: "success".into(),
            },
        ];
        assert_eq!(CiStatus::from_checks(&checks), CiStatus::Passed);
    }

    #[test]
    fn test_ci_status_one_failure() {
        use crate::github::StatusCheck;
        let checks = vec![
            StatusCheck {
                status: "completed".into(),
                conclusion: "success".into(),
            },
            StatusCheck {
                status: "completed".into(),
                conclusion: "failure".into(),
            },
        ];
        assert_eq!(CiStatus::from_checks(&checks), CiStatus::Failed);
    }

    #[test]
    fn test_ci_status_pending() {
        use crate::github::StatusCheck;
        let checks = vec![
            StatusCheck {
                status: "completed".into(),
                conclusion: "success".into(),
            },
            StatusCheck {
                status: "in_progress".into(),
                conclusion: "".into(),
            },
        ];
        assert_eq!(CiStatus::from_checks(&checks), CiStatus::Pending);
    }

    #[test]
    fn test_ci_status_error_conclusion() {
        use crate::github::StatusCheck;
        let checks = vec![StatusCheck {
            status: "completed".into(),
            conclusion: "error".into(),
        }];
        assert_eq!(CiStatus::from_checks(&checks), CiStatus::Failed);
    }

    #[test]
    fn test_ci_status_queued_is_pending() {
        use crate::github::StatusCheck;
        let checks = vec![StatusCheck {
            status: "queued".into(),
            conclusion: "".into(),
        }];
        assert_eq!(CiStatus::from_checks(&checks), CiStatus::Pending);
    }

    #[test]
    fn test_ci_status_failure_takes_priority_over_pending() {
        use crate::github::StatusCheck;
        let checks = vec![
            StatusCheck {
                status: "completed".into(),
                conclusion: "failure".into(),
            },
            StatusCheck {
                status: "in_progress".into(),
                conclusion: "".into(),
            },
        ];
        assert_eq!(CiStatus::from_checks(&checks), CiStatus::Failed);
    }

    #[test]
    fn test_count_running_sessions() {
        use tempfile::TempDir;

        use crate::session::{Session, SessionId, SessionStore};

        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());

        let running = Session::new(
            SessionId::new("running"),
            "running".into(),
            "r.yaml".into(),
            None,
        );
        store.save(&running).unwrap();

        let mut completed =
            Session::new(SessionId::new("done"), "done".into(), "d.yaml".into(), None);
        completed.status = SessionStatus::Completed;
        store.save(&completed).unwrap();

        assert_eq!(super::count_running_sessions(&store).unwrap(), 1);
    }
}
