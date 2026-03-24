use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::session::{AgentEntry, Session, SessionStore, SessionStoreError};

/// Request to spawn a new agent, received from the coordinator via MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRequest {
    pub role: String,
    pub input: serde_json::Value,
    pub parent: Option<String>,
}

/// Minimal agent role definition loaded from YAML.
/// Mirrors the structure in `fridi-agent::definition::AgentDefinition`
/// but lives in core to avoid a circular dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRoleConfig {
    pub name: String,
    pub description: String,
    pub prompt: String,
    #[serde(default)]
    pub permissions: Option<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub spawnable_roles: Vec<String>,
    #[serde(default)]
    pub default_args: Vec<String>,
}

impl AgentRoleConfig {
    /// Create a default config for a role that has no YAML definition.
    /// The agent gets a plain Claude session with no special instructions.
    pub(crate) fn default_for(role: &str) -> Self {
        Self {
            name: role.to_string(),
            description: String::new(),
            prompt: String::new(),
            permissions: None,
            allowed_tools: Vec::new(),
            spawnable_roles: Vec::new(),
            default_args: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("session store error: {0}")]
    Store(#[from] SessionStoreError),
    #[error("agent role config error: {0}")]
    RoleConfig(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Load agent role configs from a directory of YAML files
pub fn load_role_configs(dir: &Path) -> Result<Vec<AgentRoleConfig>, OrchestratorError> {
    let mut configs = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == "yaml" || ext == "yml" {
                let content = std::fs::read_to_string(&path)?;
                let config: AgentRoleConfig = serde_yaml::from_str(&content).map_err(|e| {
                    OrchestratorError::RoleConfig(format!("{}: {}", path.display(), e))
                })?;
                configs.push(config);
            }
        }
    }
    configs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(configs)
}

/// The orchestrator ties together MCP communication, agent spawning,
/// and session persistence.
pub struct Orchestrator {
    session: Session,
    store: SessionStore,
    role_configs: Vec<AgentRoleConfig>,
    agent_counts: HashMap<String, usize>,
    spawn_tx: Option<mpsc::Sender<SpawnRequest>>,
    spawn_rx: mpsc::Receiver<SpawnRequest>,
    repo: String,
    session_dir: PathBuf,
}

impl Orchestrator {
    /// Create a new orchestrator for the given session.
    ///
    /// Loads agent role configs from `agents_dir` and sets up the spawn channel.
    pub fn new(
        session: Session,
        store: SessionStore,
        role_configs: Vec<AgentRoleConfig>,
        repo: &str,
        session_dir: PathBuf,
    ) -> Self {
        let (spawn_tx, spawn_rx) = mpsc::channel(64);
        let agent_counts = Self::build_agent_counts(&session);
        Self {
            session,
            store,
            role_configs,
            agent_counts,
            spawn_tx: Some(spawn_tx),
            spawn_rx,
            repo: repo.to_string(),
            session_dir,
        }
    }

    /// Create an orchestrator by loading role configs from a directory
    pub fn from_agents_dir(
        session: Session,
        store: SessionStore,
        agents_dir: &Path,
        repo: &str,
        session_dir: PathBuf,
    ) -> Result<Self, OrchestratorError> {
        let role_configs = load_role_configs(agents_dir)?;
        Ok(Self::new(session, store, role_configs, repo, session_dir))
    }

    /// Scan existing agents in a session and compute the highest assigned
    /// number per role so that newly spawned agents never collide with
    /// persisted ones.
    fn build_agent_counts(session: &Session) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for entry in session.agents.values() {
            if let Some(n) = entry
                .id
                .rsplit_once('-')
                .and_then(|(_, suffix)| suffix.parse::<usize>().ok())
            {
                let current = counts.entry(entry.role.clone()).or_insert(0);
                if n > *current {
                    *current = n;
                }
            }
        }
        counts
    }

    /// Spawn an agent with the given role, returning its assigned ID.
    ///
    /// The agent ID follows the `{role}-{n}` format where n increments
    /// for each agent of the same role.
    pub fn spawn_agent(
        &mut self,
        role: &str,
        _input: serde_json::Value,
        parent: Option<&str>,
    ) -> Result<String, OrchestratorError> {
        if !self.role_configs.iter().any(|c| c.name == role) {
            if role.is_empty()
                || role.len() > 128
                || !role
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(OrchestratorError::RoleConfig(format!(
                    "invalid role name: {role:?}"
                )));
            }
            warn!(role = %role, "no agent config found, using default");
            self.role_configs.push(AgentRoleConfig::default_for(role));
        }

        let count = self.agent_counts.entry(role.to_string()).or_insert(0);
        *count += 1;
        let agent_id = format!("{}-{}", role, count);

        let entry = AgentEntry {
            id: agent_id.clone(),
            role: role.to_string(),
            claude_session_id: None,
            status: "spawning".to_string(),
            parent: parent.map(String::from),
            spawned_at: Utc::now(),
        };

        self.session.add_agent(entry);
        self.store.save(&self.session)?;

        info!(
            agent_id = %agent_id,
            role = %role,
            parent = ?parent,
            "spawned agent"
        );

        Ok(agent_id)
    }

    /// Get a sender that can submit spawn requests to this orchestrator.
    /// MCP tools use this to request agent spawning.
    ///
    /// Returns `None` if the spawn loop has already started (the internal
    /// sender is dropped when `run_spawn_loop` begins so the channel closes
    /// once all external senders are dropped).
    pub fn spawn_sender(&self) -> Option<mpsc::Sender<SpawnRequest>> {
        self.spawn_tx.as_ref().cloned()
    }

    /// Process incoming spawn requests until the channel closes.
    ///
    /// Drops the internal sender so the loop terminates once all external
    /// senders obtained via `spawn_sender` are dropped.
    pub async fn run_spawn_loop(&mut self) {
        self.spawn_tx.take();
        info!("spawn loop started");
        while let Some(request) = self.spawn_rx.recv().await {
            info!(role = %request.role, "received spawn request");
            match self.spawn_agent(&request.role, request.input, request.parent.as_deref()) {
                Ok(agent_id) => {
                    info!(agent_id = %agent_id, "spawn request fulfilled");
                }
                Err(e) => {
                    error!(error = %e, "failed to fulfill spawn request");
                }
            }
        }
        info!("spawn loop ended");
    }

    pub fn session(&self) -> &Session { &self.session }

    pub fn session_mut(&mut self) -> &mut Session { &mut self.session }

    pub fn role_configs(&self) -> &[AgentRoleConfig] { &self.role_configs }

    pub fn repo(&self) -> &str { &self.repo }

    pub fn session_dir(&self) -> &Path { &self.session_dir }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::session::SessionId;

    fn test_role_configs() -> Vec<AgentRoleConfig> {
        vec![
            AgentRoleConfig {
                name: "coordinator".into(),
                description: "Coordinates work".into(),
                prompt: "You coordinate".into(),
                permissions: Some("bypassPermissions".into()),
                allowed_tools: vec!["Bash".into()],
                spawnable_roles: vec!["developer".into()],
                default_args: vec![],
            },
            AgentRoleConfig {
                name: "developer".into(),
                description: "Writes code".into(),
                prompt: "You develop".into(),
                permissions: None,
                allowed_tools: vec!["Bash".into(), "Read".into()],
                spawnable_roles: vec![],
                default_args: vec![],
            },
            AgentRoleConfig {
                name: "qa".into(),
                description: "Reviews code".into(),
                prompt: "You review".into(),
                permissions: None,
                allowed_tools: vec!["Read".into()],
                spawnable_roles: vec![],
                default_args: vec![],
            },
        ]
    }

    fn test_session() -> Session {
        Session::new(
            SessionId::new("test-wf"),
            "test-wf".into(),
            "test.yaml".into(),
            Some("owner/repo".into()),
        )
    }

    #[test]
    fn test_orchestrator_new() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let session = test_session();
        let session_dir = tmp.path().join(session.id.as_str());

        let orch = Orchestrator::new(
            session,
            store,
            test_role_configs(),
            "owner/repo",
            session_dir,
        );

        assert_eq!(orch.role_configs().len(), 3);
        assert_eq!(orch.repo(), "owner/repo");
        assert!(orch.session().agents.is_empty());
    }

    #[test]
    fn test_spawn_agent() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let session = test_session();
        let session_dir = tmp.path().join(session.id.as_str());

        let mut orch = Orchestrator::new(
            session,
            store.clone(),
            test_role_configs(),
            "owner/repo",
            session_dir,
        );

        let agent_id = orch
            .spawn_agent(
                "developer",
                serde_json::json!({"task": "implement feature"}),
                None,
            )
            .unwrap();

        assert_eq!(agent_id, "developer-1");
        assert_eq!(orch.session().agents.len(), 1);

        let entry = &orch.session().agents["developer-1"];
        assert_eq!(entry.role, "developer");
        assert_eq!(entry.status, "spawning");
        assert!(entry.parent.is_none());
        assert!(entry.claude_session_id.is_none());

        // Verify persisted to store
        let loaded = store.load(&orch.session().id).unwrap();
        assert_eq!(loaded.agents.len(), 1);
        assert!(loaded.agents.contains_key("developer-1"));
    }

    #[test]
    fn test_agent_id_incrementing() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let session = test_session();
        let session_dir = tmp.path().join(session.id.as_str());

        let mut orch = Orchestrator::new(
            session,
            store,
            test_role_configs(),
            "owner/repo",
            session_dir,
        );

        let id1 = orch
            .spawn_agent("developer", serde_json::json!({}), None)
            .unwrap();
        let id2 = orch
            .spawn_agent("developer", serde_json::json!({}), None)
            .unwrap();
        let id3 = orch
            .spawn_agent("developer", serde_json::json!({}), None)
            .unwrap();

        assert_eq!(id1, "developer-1");
        assert_eq!(id2, "developer-2");
        assert_eq!(id3, "developer-3");
        assert_eq!(orch.session().agents.len(), 3);
    }

    #[test]
    fn test_unknown_role_gets_default_config() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let session = test_session();
        let session_dir = tmp.path().join(session.id.as_str());

        let mut orch = Orchestrator::new(
            session,
            store,
            test_role_configs(),
            "owner/repo",
            session_dir,
        );

        // Unknown roles should succeed with a default config
        let agent_id = orch
            .spawn_agent("claude", serde_json::json!({}), None)
            .unwrap();
        assert_eq!(agent_id, "claude-1");

        // The default config should have been added to role_configs
        let config = orch
            .role_configs()
            .iter()
            .find(|c| c.name == "claude")
            .unwrap();
        assert!(config.prompt.is_empty());
        assert!(config.description.is_empty());
        assert!(config.permissions.is_none());
        assert!(config.allowed_tools.is_empty());

        // Subsequent spawns of the same unknown role should reuse the config
        let agent_id2 = orch
            .spawn_agent("claude", serde_json::json!({}), None)
            .unwrap();
        assert_eq!(agent_id2, "claude-2");

        // A different unknown role also gets a default
        let agent_id3 = orch
            .spawn_agent("custom-agent", serde_json::json!({}), None)
            .unwrap();
        assert_eq!(agent_id3, "custom-agent-1");
    }

    #[test]
    fn test_session_agent_tracking() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let session = test_session();
        let session_dir = tmp.path().join(session.id.as_str());

        let mut orch = Orchestrator::new(
            session,
            store,
            test_role_configs(),
            "owner/repo",
            session_dir,
        );

        let coord_id = orch
            .spawn_agent("coordinator", serde_json::json!({}), None)
            .unwrap();
        let dev_id = orch
            .spawn_agent(
                "developer",
                serde_json::json!({"task": "code"}),
                Some(&coord_id),
            )
            .unwrap();
        let qa_id = orch
            .spawn_agent("qa", serde_json::json!({"task": "review"}), Some(&coord_id))
            .unwrap();

        assert_eq!(coord_id, "coordinator-1");
        assert_eq!(dev_id, "developer-1");
        assert_eq!(qa_id, "qa-1");

        let agents = &orch.session().agents;
        assert_eq!(agents.len(), 3);

        assert!(agents["coordinator-1"].parent.is_none());
        assert_eq!(
            agents["developer-1"].parent.as_deref(),
            Some("coordinator-1")
        );
        assert_eq!(agents["qa-1"].parent.as_deref(), Some("coordinator-1"));
    }

    #[test]
    fn test_load_role_configs_from_dir() {
        let tmp = TempDir::new().unwrap();

        std::fs::write(
            tmp.path().join("developer.yaml"),
            "name: developer\ndescription: Writes code\nprompt: You develop\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("coordinator.yml"),
            "name: coordinator\ndescription: Coordinates\nprompt: You coordinate\n",
        )
        .unwrap();
        // Non-YAML file should be skipped
        std::fs::write(tmp.path().join("readme.txt"), "not yaml").unwrap();

        let configs = load_role_configs(tmp.path()).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].name, "coordinator");
        assert_eq!(configs[1].name, "developer");
    }

    #[test]
    fn test_from_agents_dir() {
        let agents_tmp = TempDir::new().unwrap();
        let store_tmp = TempDir::new().unwrap();

        std::fs::write(
            agents_tmp.path().join("dev.yaml"),
            "name: developer\ndescription: D\nprompt: P\n",
        )
        .unwrap();

        let store = SessionStore::new(store_tmp.path());
        let session = test_session();
        let session_dir = store_tmp.path().join(session.id.as_str());

        let orch = Orchestrator::from_agents_dir(
            session,
            store,
            agents_tmp.path(),
            "owner/repo",
            session_dir,
        )
        .unwrap();

        assert_eq!(orch.role_configs().len(), 1);
        assert_eq!(orch.role_configs()[0].name, "developer");
    }

    #[tokio::test]
    async fn test_spawn_loop() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let session = test_session();
        let session_dir = tmp.path().join(session.id.as_str());

        let mut orch = Orchestrator::new(
            session,
            store,
            test_role_configs(),
            "owner/repo",
            session_dir,
        );

        let tx = orch.spawn_sender().unwrap();

        tx.send(SpawnRequest {
            role: "developer".into(),
            input: serde_json::json!({"task": "build"}),
            parent: Some("coordinator-1".into()),
        })
        .await
        .unwrap();

        tx.send(SpawnRequest {
            role: "qa".into(),
            input: serde_json::json!({"task": "review"}),
            parent: Some("coordinator-1".into()),
        })
        .await
        .unwrap();

        // Drop sender so the loop will end after processing
        drop(tx);

        orch.run_spawn_loop().await;

        assert_eq!(orch.session().agents.len(), 2);
        assert!(orch.session().agents.contains_key("developer-1"));
        assert!(orch.session().agents.contains_key("qa-1"));
    }

    #[test]
    fn test_agent_counts_initialized_from_persisted_session() {
        let tmp = TempDir::new().unwrap();
        let store = SessionStore::new(tmp.path());
        let mut session = test_session();

        // Simulate a persisted session that already has agents
        session.add_agent(AgentEntry {
            id: "developer-1".into(),
            role: "developer".into(),
            claude_session_id: None,
            status: "done".into(),
            parent: None,
            spawned_at: Utc::now(),
        });
        session.add_agent(AgentEntry {
            id: "developer-3".into(),
            role: "developer".into(),
            claude_session_id: None,
            status: "running".into(),
            parent: None,
            spawned_at: Utc::now(),
        });
        session.add_agent(AgentEntry {
            id: "qa-2".into(),
            role: "qa".into(),
            claude_session_id: None,
            status: "done".into(),
            parent: None,
            spawned_at: Utc::now(),
        });

        store.save(&session).unwrap();

        let session_dir = tmp.path().join(session.id.as_str());
        let mut orch = Orchestrator::new(
            session,
            store,
            test_role_configs(),
            "owner/repo",
            session_dir,
        );

        // New developer should be 4 (max existing is 3)
        let dev_id = orch
            .spawn_agent("developer", serde_json::json!({}), None)
            .unwrap();
        assert_eq!(dev_id, "developer-4");

        // New qa should be 3 (max existing is 2)
        let qa_id = orch.spawn_agent("qa", serde_json::json!({}), None).unwrap();
        assert_eq!(qa_id, "qa-3");

        // New coordinator should be 1 (none existed before)
        let coord_id = orch
            .spawn_agent("coordinator", serde_json::json!({}), None)
            .unwrap();
        assert_eq!(coord_id, "coordinator-1");
    }
}
