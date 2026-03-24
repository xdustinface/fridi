use std::collections::HashMap;
use std::path::PathBuf;
use std::{fmt, fs};

use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::engine::StepStatus;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(workflow_name: &str) -> Self {
        let now = Utc::now();
        let date = now.format("%Y%m%d");
        let hash: u16 = rand::thread_rng().gen();
        let safe_name: String = workflow_name
            .chars()
            .map(|c| {
                if c == '/' || c == '\\' || c == '.' || c == '\0' {
                    '-'
                } else {
                    c
                }
            })
            .collect();
        Self(format!("{}-{}-{:04x}", safe_name, date, hash))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepSessionId(String);

impl StepSessionId {
    pub fn new(step_name: &str, attempt: u32) -> Self { Self(format!("{}-{}", step_name, attempt)) }

    pub fn step_name(&self) -> &str {
        match self.0.rfind('-') {
            Some(pos) => &self.0[..pos],
            None => &self.0,
        }
    }

    pub fn attempt(&self) -> u32 {
        match self.0.rfind('-') {
            Some(pos) => self.0[pos + 1..].parse().unwrap_or(0),
            None => 0,
        }
    }
}

impl fmt::Display for StepSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepSession {
    pub step_name: String,
    pub attempt: u32,
    pub status: StepStatus,
    pub claude_session_id: Option<String>,
    pub output_summary: Option<JsonValue>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub workflow_name: String,
    pub workflow_file: String,
    pub repo: Option<String>,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub steps: HashMap<StepSessionId, StepSession>,
}

impl Session {
    pub fn new(
        id: SessionId,
        workflow_name: String,
        workflow_file: String,
        repo: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            workflow_name,
            workflow_file,
            repo,
            status: SessionStatus::Running,
            created_at: now,
            updated_at: now,
            steps: HashMap::new(),
        }
    }

    pub fn update_step(&mut self, step_id: StepSessionId, step: StepSession) {
        self.steps.insert(step_id, step);
        self.updated_at = Utc::now();
        self.status = self.derive_status();
    }

    /// Mark any steps that were `Running` at shutdown as `Failed("interrupted")`,
    /// and set the session status to `Interrupted`.
    pub fn mark_interrupted(&mut self) {
        let mut had_running = false;
        for step in self.steps.values_mut() {
            if step.status == StepStatus::Running {
                step.status = StepStatus::Failed("interrupted".into());
                had_running = true;
            }
        }
        if had_running || self.status == SessionStatus::Running {
            self.status = SessionStatus::Interrupted;
            self.updated_at = Utc::now();
        }
    }

    pub fn derive_status(&self) -> SessionStatus {
        let mut has_running = false;
        let mut has_failed = false;
        let mut all_done = true;

        for step in self.steps.values() {
            match &step.status {
                StepStatus::Running => {
                    has_running = true;
                    all_done = false;
                }
                StepStatus::Failed(_) => {
                    has_failed = true;
                }
                StepStatus::Completed | StepStatus::Skipped => {}
                StepStatus::Pending => {
                    all_done = false;
                }
            }
        }

        if has_running {
            SessionStatus::Running
        } else if has_failed {
            SessionStatus::Failed
        } else if all_done && !self.steps.is_empty() {
            SessionStatus::Completed
        } else {
            SessionStatus::Running
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub workflow_name: String,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&Session> for SessionSummary {
    fn from(session: &Session) -> Self {
        Self {
            id: session.id.clone(),
            workflow_name: session.workflow_name.clone(),
            status: session.status.clone(),
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionStoreError {
    #[error("session not found: {0}")]
    NotFound(SessionId),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    base_dir: PathBuf,
}

impl SessionStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn session_dir(&self, id: &SessionId) -> PathBuf { self.base_dir.join(id.as_str()) }

    fn session_file(&self, id: &SessionId) -> PathBuf { self.session_dir(id).join("session.json") }

    pub fn save(&self, session: &Session) -> Result<(), SessionStoreError> {
        let dir = self.session_dir(&session.id);
        fs::create_dir_all(&dir)?;

        let json = serde_json::to_string_pretty(session)?;
        let tmp_name = format!("session.json.{:016x}.tmp", rand::thread_rng().gen::<u64>());
        let tmp_path = dir.join(tmp_name);
        fs::write(&tmp_path, &json)?;
        fs::rename(&tmp_path, self.session_file(&session.id))?;
        Ok(())
    }

    pub fn load(&self, id: &SessionId) -> Result<Session, SessionStoreError> {
        let path = self.session_file(id);
        if !path.exists() {
            return Err(SessionStoreError::NotFound(id.clone()));
        }
        let content = fs::read_to_string(&path)?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }

    pub fn list(&self) -> Result<Vec<SessionSummary>, SessionStoreError> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let session_file = entry.path().join("session.json");
            if session_file.exists() {
                let content = fs::read_to_string(&session_file)?;
                let session: Session = serde_json::from_str(&content)?;
                summaries.push(SessionSummary::from(&session));
            }
        }
        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(summaries)
    }

    /// Load all sessions, marking any that were `Running` as `Interrupted`.
    /// Interrupted sessions are persisted back to disk.
    pub fn load_all_and_recover(&self) -> Result<Vec<Session>, SessionStoreError> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let session_file = entry.path().join("session.json");
            if session_file.exists() {
                let content = fs::read_to_string(&session_file)?;
                let mut session: Session = serde_json::from_str(&content)?;
                if session.status == SessionStatus::Running {
                    session.mark_interrupted();
                    self.save(&session)?;
                }
                sessions.push(session);
            }
        }
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    pub fn delete(&self, id: &SessionId) -> Result<(), SessionStoreError> {
        let dir = self.session_dir(id);
        if !dir.exists() {
            return Err(SessionStoreError::NotFound(id.clone()));
        }
        fs::remove_dir_all(&dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_session_id_format() {
        let id = SessionId::new("my-workflow");
        let s = id.as_str();
        assert!(s.starts_with("my-workflow-"));
        // Format: <name>-<YYYYMMDD>-<4hex>
        let parts: Vec<&str> = s.rsplitn(3, '-').collect();
        assert_eq!(parts.len(), 3);
        // parts[0] = 4-char hex, parts[1] = YYYYMMDD, parts[2] = rest of name
        assert_eq!(parts[0].len(), 4);
        assert!(parts[0].chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(parts[1].len(), 8);
        assert!(parts[1].chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_session_id_display() {
        let id = SessionId::new("test");
        let display = format!("{}", id);
        assert_eq!(display, id.as_str());
    }

    #[test]
    fn test_step_session_id_format() {
        let id = StepSessionId::new("build-code", 3);
        assert_eq!(id.to_string(), "build-code-3");
        assert_eq!(id.step_name(), "build-code");
        assert_eq!(id.attempt(), 3);
    }

    #[test]
    fn test_step_session_id_simple_name() {
        let id = StepSessionId::new("build", 1);
        assert_eq!(id.step_name(), "build");
        assert_eq!(id.attempt(), 1);
    }

    #[test]
    fn test_session_new() {
        let id = SessionId::new("wf");
        let session = Session::new(
            id.clone(),
            "wf".into(),
            "wf.yaml".into(),
            Some("owner/repo".into()),
        );
        assert_eq!(session.id, id);
        assert_eq!(session.workflow_name, "wf");
        assert_eq!(session.workflow_file, "wf.yaml");
        assert_eq!(session.repo.as_deref(), Some("owner/repo"));
        assert_eq!(session.status, SessionStatus::Running);
        assert!(session.steps.is_empty());
    }

    #[test]
    fn test_session_update_step() {
        let mut session = Session::new(SessionId::new("wf"), "wf".into(), "wf.yaml".into(), None);
        let before = session.updated_at;

        let step_id = StepSessionId::new("step1", 1);
        let step = StepSession {
            step_name: "step1".into(),
            attempt: 1,
            status: StepStatus::Running,
            claude_session_id: None,
            output_summary: None,
            started_at: Some(Utc::now()),
            finished_at: None,
        };
        session.update_step(step_id.clone(), step);

        assert_eq!(session.steps.len(), 1);
        assert!(session.steps.contains_key(&step_id));
        assert!(session.updated_at >= before);
    }

    #[test]
    fn test_derive_status_running() {
        let mut session = Session::new(SessionId::new("wf"), "wf".into(), "wf.yaml".into(), None);
        session.update_step(
            StepSessionId::new("a", 1),
            StepSession {
                step_name: "a".into(),
                attempt: 1,
                status: StepStatus::Running,
                claude_session_id: None,
                output_summary: None,
                started_at: None,
                finished_at: None,
            },
        );
        assert_eq!(session.derive_status(), SessionStatus::Running);
    }

    #[test]
    fn test_derive_status_completed() {
        let mut session = Session::new(SessionId::new("wf"), "wf".into(), "wf.yaml".into(), None);
        session.update_step(
            StepSessionId::new("a", 1),
            StepSession {
                step_name: "a".into(),
                attempt: 1,
                status: StepStatus::Completed,
                claude_session_id: None,
                output_summary: None,
                started_at: None,
                finished_at: None,
            },
        );
        session.update_step(
            StepSessionId::new("b", 1),
            StepSession {
                step_name: "b".into(),
                attempt: 1,
                status: StepStatus::Skipped,
                claude_session_id: None,
                output_summary: None,
                started_at: None,
                finished_at: None,
            },
        );
        assert_eq!(session.derive_status(), SessionStatus::Completed);
    }

    #[test]
    fn test_derive_status_failed() {
        let mut session = Session::new(SessionId::new("wf"), "wf".into(), "wf.yaml".into(), None);
        session.update_step(
            StepSessionId::new("a", 1),
            StepSession {
                step_name: "a".into(),
                attempt: 1,
                status: StepStatus::Completed,
                claude_session_id: None,
                output_summary: None,
                started_at: None,
                finished_at: None,
            },
        );
        session.update_step(
            StepSessionId::new("b", 1),
            StepSession {
                step_name: "b".into(),
                attempt: 1,
                status: StepStatus::Failed("oops".into()),
                claude_session_id: None,
                output_summary: None,
                started_at: None,
                finished_at: None,
            },
        );
        assert_eq!(session.derive_status(), SessionStatus::Failed);
    }

    #[test]
    fn test_store_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());

        let id = SessionId::new("wf");
        let mut session = Session::new(
            id.clone(),
            "wf".into(),
            "wf.yaml".into(),
            Some("repo".into()),
        );
        session.update_step(
            StepSessionId::new("s1", 1),
            StepSession {
                step_name: "s1".into(),
                attempt: 1,
                status: StepStatus::Completed,
                claude_session_id: Some("claude-123".into()),
                output_summary: Some(serde_json::json!({"result": true})),
                started_at: Some(Utc::now()),
                finished_at: Some(Utc::now()),
            },
        );

        store.save(&session).unwrap();
        let loaded = store.load(&id).unwrap();

        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.workflow_name, session.workflow_name);
        assert_eq!(loaded.workflow_file, session.workflow_file);
        assert_eq!(loaded.repo, session.repo);
        assert_eq!(loaded.status, session.status);
        assert_eq!(loaded.steps.len(), 1);
    }

    #[test]
    fn test_store_list() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());

        let s1 = Session::new(SessionId::new("wf-a"), "wf-a".into(), "a.yaml".into(), None);
        let s2 = Session::new(SessionId::new("wf-b"), "wf-b".into(), "b.yaml".into(), None);

        store.save(&s1).unwrap();
        store.save(&s2).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_store_delete() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());

        let id = SessionId::new("wf");
        let session = Session::new(id.clone(), "wf".into(), "wf.yaml".into(), None);
        store.save(&session).unwrap();

        store.delete(&id).unwrap();
        assert!(matches!(
            store.load(&id),
            Err(SessionStoreError::NotFound(_))
        ));
    }

    #[test]
    fn test_store_load_not_found() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let id = SessionId::new("nonexistent");

        assert!(matches!(
            store.load(&id),
            Err(SessionStoreError::NotFound(_))
        ));
    }

    #[test]
    fn test_session_serialization() {
        let mut session = Session::new(
            SessionId::new("wf"),
            "wf".into(),
            "wf.yaml".into(),
            Some("owner/repo".into()),
        );
        session.update_step(
            StepSessionId::new("build", 2),
            StepSession {
                step_name: "build".into(),
                attempt: 2,
                status: StepStatus::Failed("compile error".into()),
                claude_session_id: Some("sess-abc".into()),
                output_summary: Some(serde_json::json!({"errors": 3})),
                started_at: Some(Utc::now()),
                finished_at: Some(Utc::now()),
            },
        );

        let json = serde_json::to_string(&session).unwrap();
        let restored: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.id, session.id);
        assert_eq!(restored.workflow_name, session.workflow_name);
        assert_eq!(restored.repo, session.repo);
        assert_eq!(restored.steps.len(), 1);

        let step_id = StepSessionId::new("build", 2);
        let step = &restored.steps[&step_id];
        assert_eq!(step.step_name, "build");
        assert_eq!(step.attempt, 2);
        assert!(matches!(&step.status, StepStatus::Failed(msg) if msg == "compile error"));
    }

    #[test]
    fn test_atomic_write() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());

        let id = SessionId::new("wf");
        let session = Session::new(id.clone(), "wf".into(), "wf.yaml".into(), None);
        store.save(&session).unwrap();

        // No .tmp files should remain after a successful save
        let session_dir = tmp.path().join(id.as_str());
        let tmp_files: Vec<_> = fs::read_dir(&session_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(tmp_files.is_empty());

        let session_file = session_dir.join("session.json");
        assert!(session_file.exists());
    }

    #[test]
    fn test_session_summary_from_session() {
        let session = Session::new(SessionId::new("wf"), "wf".into(), "wf.yaml".into(), None);
        let summary = SessionSummary::from(&session);
        assert_eq!(summary.id, session.id);
        assert_eq!(summary.workflow_name, session.workflow_name);
        assert_eq!(summary.status, session.status);
        assert_eq!(summary.created_at, session.created_at);
    }

    #[test]
    fn test_store_list_empty() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let list = store.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_store_list_nonexistent_dir() {
        let store = SessionStore::new("/tmp/fridi-test-nonexistent-dir-xyz");
        let list = store.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_session_id_sanitizes_path_separators() {
        let id = SessionId::new("../evil/path");
        assert!(!id.as_str().contains('/'));
        assert!(!id.as_str().contains('\\'));
        assert!(!id.as_str().contains(".."));
    }

    #[test]
    fn test_interrupted_session_detection() {
        let mut session = Session::new(
            SessionId::new("wf"),
            "wf".into(),
            "wf.yaml".into(),
            Some("owner/repo".into()),
        );

        // Add a completed step and a running step
        session.update_step(
            StepSessionId::new("done", 1),
            StepSession {
                step_name: "done".into(),
                attempt: 1,
                status: StepStatus::Completed,
                claude_session_id: Some("sess-1".into()),
                output_summary: None,
                started_at: Some(Utc::now()),
                finished_at: Some(Utc::now()),
            },
        );
        session.update_step(
            StepSessionId::new("active", 1),
            StepSession {
                step_name: "active".into(),
                attempt: 1,
                status: StepStatus::Running,
                claude_session_id: Some("sess-2".into()),
                output_summary: None,
                started_at: Some(Utc::now()),
                finished_at: None,
            },
        );

        assert_eq!(session.status, SessionStatus::Running);

        session.mark_interrupted();

        assert_eq!(session.status, SessionStatus::Interrupted);

        // The running step should be marked as failed with "interrupted"
        let active_step = &session.steps[&StepSessionId::new("active", 1)];
        assert!(matches!(&active_step.status, StepStatus::Failed(msg) if msg == "interrupted"));

        // The completed step should remain unchanged
        let done_step = &session.steps[&StepSessionId::new("done", 1)];
        assert_eq!(done_step.status, StepStatus::Completed);
    }

    #[test]
    fn test_mark_interrupted_no_running_steps() {
        let mut session = Session::new(SessionId::new("wf"), "wf".into(), "wf.yaml".into(), None);
        session.update_step(
            StepSessionId::new("s1", 1),
            StepSession {
                step_name: "s1".into(),
                attempt: 1,
                status: StepStatus::Completed,
                claude_session_id: None,
                output_summary: None,
                started_at: None,
                finished_at: None,
            },
        );
        // Force status to completed
        session.status = SessionStatus::Completed;

        session.mark_interrupted();

        // Should not change status if already completed with no running steps
        assert_eq!(session.status, SessionStatus::Completed);
    }

    #[test]
    fn test_load_all_and_recover() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());

        // Save a running session
        let mut running = Session::new(
            SessionId::new("running"),
            "running".into(),
            "r.yaml".into(),
            None,
        );
        running.update_step(
            StepSessionId::new("step", 1),
            StepSession {
                step_name: "step".into(),
                attempt: 1,
                status: StepStatus::Running,
                claude_session_id: Some("sess-abc".into()),
                output_summary: None,
                started_at: Some(Utc::now()),
                finished_at: None,
            },
        );
        store.save(&running).unwrap();

        // Save a completed session
        let mut completed =
            Session::new(SessionId::new("done"), "done".into(), "d.yaml".into(), None);
        completed.update_step(
            StepSessionId::new("step", 1),
            StepSession {
                step_name: "step".into(),
                attempt: 1,
                status: StepStatus::Completed,
                claude_session_id: None,
                output_summary: None,
                started_at: None,
                finished_at: None,
            },
        );
        store.save(&completed).unwrap();

        let sessions = store.load_all_and_recover().unwrap();
        assert_eq!(sessions.len(), 2);

        let recovered = sessions
            .iter()
            .find(|s| s.workflow_name == "running")
            .unwrap();
        assert_eq!(recovered.status, SessionStatus::Interrupted);

        let still_done = sessions.iter().find(|s| s.workflow_name == "done").unwrap();
        assert_eq!(still_done.status, SessionStatus::Completed);

        // Verify the interrupted session was persisted to disk
        let reloaded = store.load(&running.id).unwrap();
        assert_eq!(reloaded.status, SessionStatus::Interrupted);
    }
}
