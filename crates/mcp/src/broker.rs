use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

use tokio::sync::{broadcast, mpsc};
use tracing::debug;

use crate::types::{AgentInfo, AgentMessage, AgentResult, SpawnRequest, StatusUpdate};

/// Manages mailboxes and routes messages between agents.
///
/// All public methods are safe to call from multiple threads. Internal state
/// is protected by a `Mutex` so callers never need external synchronization.
pub struct MessageBroker {
    inner: Mutex<BrokerState>,
    status_tx: broadcast::Sender<StatusUpdate>,
    result_tx: broadcast::Sender<AgentResult>,
    spawn_tx: mpsc::Sender<SpawnRequest>,
}

struct BrokerState {
    mailboxes: HashMap<String, Vec<AgentMessage>>,
    agents: HashMap<String, AgentInfo>,
}

impl MessageBroker {
    /// Create a new broker together with a spawn-request receiver.
    ///
    /// The returned `mpsc::Receiver<SpawnRequest>` should be consumed by the
    /// orchestrator to handle dynamic agent spawning.
    pub fn new() -> (Self, mpsc::Receiver<SpawnRequest>) {
        let (status_tx, _) = broadcast::channel(256);
        let (result_tx, _) = broadcast::channel(256);
        let (spawn_tx, spawn_rx) = mpsc::channel(64);
        let broker = Self {
            inner: Mutex::new(BrokerState {
                mailboxes: HashMap::new(),
                agents: HashMap::new(),
            }),
            status_tx,
            result_tx,
            spawn_tx,
        };
        (broker, spawn_rx)
    }

    /// Register a new agent with the broker, creating an empty mailbox.
    pub fn register_agent(&self, id: String, role: String, parent: Option<String>) {
        let info = AgentInfo {
            id: id.clone(),
            role,
            status: "running".into(),
            parent,
            spawned_at: SystemTime::now(),
        };
        let mut state = self.inner.lock().expect("broker lock poisoned");
        state.mailboxes.entry(id.clone()).or_default();
        state.agents.insert(id, info);
    }

    /// Route a message to the recipient's mailbox.
    ///
    /// Returns `false` if the recipient is not registered.
    pub fn send_message(&self, from: &str, to: &str, content: serde_json::Value) -> bool {
        let msg = AgentMessage {
            from: from.to_string(),
            to: to.to_string(),
            content,
            timestamp: SystemTime::now(),
        };
        let mut state = self.inner.lock().expect("broker lock poisoned");
        if let Some(mailbox) = state.mailboxes.get_mut(to) {
            debug!(from, to, "message routed");
            mailbox.push(msg);
            true
        } else {
            false
        }
    }

    /// Drain and return all pending messages for an agent.
    pub fn read_messages(&self, agent_id: &str) -> Vec<AgentMessage> {
        let mut state = self.inner.lock().expect("broker lock poisoned");
        state
            .mailboxes
            .get_mut(agent_id)
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// Update an agent's status and broadcast the change to subscribers.
    pub fn update_status(&self, agent_id: &str, status: String, detail: Option<String>) -> bool {
        let update = {
            let mut state = self.inner.lock().expect("broker lock poisoned");
            if let Some(info) = state.agents.get_mut(agent_id) {
                info.status = status.clone();
                true
            } else {
                false
            }
        };
        if update {
            let _ = self.status_tx.send(StatusUpdate {
                agent_id: agent_id.to_string(),
                status,
                detail,
                timestamp: SystemTime::now(),
            });
        }
        update
    }

    /// Record a result from an agent and mark it as completed.
    pub fn report_result(&self, agent_id: &str, result: serde_json::Value) -> bool {
        let ok = {
            let mut state = self.inner.lock().expect("broker lock poisoned");
            if let Some(info) = state.agents.get_mut(agent_id) {
                info.status = "completed".into();
                true
            } else {
                false
            }
        };
        if ok {
            let _ = self.result_tx.send(AgentResult {
                agent_id: agent_id.to_string(),
                result,
                timestamp: SystemTime::now(),
            });
            let _ = self.status_tx.send(StatusUpdate {
                agent_id: agent_id.to_string(),
                status: "completed".into(),
                detail: None,
                timestamp: SystemTime::now(),
            });
        }
        ok
    }

    /// Forward a spawn request to the orchestrator.
    pub async fn request_spawn(
        &self,
        agent_id: String,
        role: String,
        input: serde_json::Value,
        requested_by: String,
    ) -> Result<(), mpsc::error::SendError<SpawnRequest>> {
        self.spawn_tx
            .send(SpawnRequest {
                agent_id,
                role,
                input,
                requested_by,
            })
            .await
    }

    /// Return info about all registered agents.
    pub fn list_agents(&self) -> Vec<AgentInfo> {
        let state = self.inner.lock().expect("broker lock poisoned");
        state.agents.values().cloned().collect()
    }

    /// Subscribe to status update broadcasts.
    pub fn subscribe_status(&self) -> broadcast::Receiver<StatusUpdate> {
        self.status_tx.subscribe()
    }

    /// Subscribe to agent result broadcasts.
    pub fn subscribe_results(&self) -> broadcast::Receiver<AgentResult> {
        self.result_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_register_and_list_agents() {
        let (broker, _rx) = MessageBroker::new();
        broker.register_agent("a1".into(), "coordinator".into(), None);
        broker.register_agent("a2".into(), "developer".into(), Some("a1".into()));

        let agents = broker.list_agents();
        assert_eq!(agents.len(), 2);
        let ids: Vec<&str> = agents.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"a1"));
        assert!(ids.contains(&"a2"));
    }

    #[test]
    fn test_send_and_read_messages() {
        let (broker, _rx) = MessageBroker::new();
        broker.register_agent("sender".into(), "coordinator".into(), None);
        broker.register_agent("receiver".into(), "developer".into(), None);

        let sent = broker.send_message("sender", "receiver", json!({"task": "build"}));
        assert!(sent);

        let messages = broker.read_messages("receiver");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].from, "sender");
        assert_eq!(messages[0].content, json!({"task": "build"}));

        // Reading again should return empty (messages were drained)
        let messages = broker.read_messages("receiver");
        assert!(messages.is_empty());
    }

    #[test]
    fn test_mailbox_isolation() {
        let (broker, _rx) = MessageBroker::new();
        broker.register_agent("a".into(), "dev".into(), None);
        broker.register_agent("b".into(), "dev".into(), None);

        broker.send_message("x", "a", json!("for-a"));
        broker.send_message("x", "b", json!("for-b"));

        let a_msgs = broker.read_messages("a");
        assert_eq!(a_msgs.len(), 1);
        assert_eq!(a_msgs[0].content, json!("for-a"));

        let b_msgs = broker.read_messages("b");
        assert_eq!(b_msgs.len(), 1);
        assert_eq!(b_msgs[0].content, json!("for-b"));
    }

    #[test]
    fn test_send_to_unknown_recipient() {
        let (broker, _rx) = MessageBroker::new();
        let sent = broker.send_message("a", "nonexistent", json!("hello"));
        assert!(!sent);
    }

    #[test]
    fn test_status_updates() {
        let (broker, _rx) = MessageBroker::new();
        broker.register_agent("a1".into(), "dev".into(), None);
        let mut status_rx = broker.subscribe_status();

        let ok = broker.update_status("a1", "working".into(), Some("step 2".into()));
        assert!(ok);

        let update = status_rx.try_recv().unwrap();
        assert_eq!(update.agent_id, "a1");
        assert_eq!(update.status, "working");
        assert_eq!(update.detail.as_deref(), Some("step 2"));

        // Agent info should reflect the update
        let agents = broker.list_agents();
        let a1 = agents.iter().find(|a| a.id == "a1").unwrap();
        assert_eq!(a1.status, "working");
    }

    #[test]
    fn test_status_update_unknown_agent() {
        let (broker, _rx) = MessageBroker::new();
        let ok = broker.update_status("ghost", "running".into(), None);
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_spawn_request() {
        let (broker, mut spawn_rx) = MessageBroker::new();
        broker.register_agent("coord".into(), "coordinator".into(), None);

        broker
            .request_spawn(
                "new-agent-1".into(),
                "developer".into(),
                json!({"task": "code"}),
                "coord".into(),
            )
            .await
            .unwrap();

        let req = spawn_rx.recv().await.unwrap();
        assert_eq!(req.agent_id, "new-agent-1");
        assert_eq!(req.role, "developer");
        assert_eq!(req.input, json!({"task": "code"}));
        assert_eq!(req.requested_by, "coord");
    }

    #[test]
    fn test_report_result() {
        let (broker, _rx) = MessageBroker::new();
        broker.register_agent("a1".into(), "dev".into(), None);
        let mut status_rx = broker.subscribe_status();
        let mut result_rx = broker.subscribe_results();

        let ok = broker.report_result("a1", json!({"output": "done"}));
        assert!(ok);

        let result = result_rx.try_recv().unwrap();
        assert_eq!(result.agent_id, "a1");
        assert_eq!(result.result, json!({"output": "done"}));

        let status = status_rx.try_recv().unwrap();
        assert_eq!(status.status, "completed");

        let agents = broker.list_agents();
        let a1 = agents.iter().find(|a| a.id == "a1").unwrap();
        assert_eq!(a1.status, "completed");
    }

    #[test]
    fn test_report_result_unknown_agent() {
        let (broker, _rx) = MessageBroker::new();
        let ok = broker.report_result("ghost", json!("result"));
        assert!(!ok);
    }
}
