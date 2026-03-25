use std::collections::HashMap;
use std::time::Duration;

use dioxus::prelude::*;
use fridi_core::engine::{EngineEvent, StepStatus};
use fridi_core::session::{SessionId, SessionStatus};
use tokio::sync::broadcast;

/// Maximum bytes retained per step in the agent output buffer.
/// Older output is discarded when this limit is exceeded.
const MAX_OUTPUT_BYTES_PER_STEP: usize = 512 * 1024;

/// Live state for a single session, updated by engine events.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SessionLiveState {
    pub(crate) step_statuses: HashMap<String, StepStatus>,
    pub(crate) workflow_status: Option<SessionStatus>,
    pub(crate) notifications: Vec<String>,
    pub(crate) agent_outputs: HashMap<String, Vec<u8>>,
}

/// Per-session engine receivers, keyed by session id.
pub(crate) type EngineReceivers = Signal<HashMap<SessionId, broadcast::Receiver<EngineEvent>>>;

/// Per-session live workflow state, keyed by session id.
pub(crate) type LiveStates = Signal<HashMap<SessionId, SessionLiveState>>;

/// Dioxus hook that manages per-session engine event receivers.
///
/// Watches the receivers map for new entries and spawns a processing task
/// for each new session. Each task drains its receiver and updates the
/// corresponding entry in the returned live-states map.
pub(crate) fn use_engine_events(mut receivers: EngineReceivers) -> LiveStates {
    let mut states: LiveStates = use_signal(HashMap::new);

    // Track which sessions already have a spawned listener
    let mut tracked: Signal<Vec<SessionId>> = use_signal(Vec::new);

    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        loop {
            // Check for new receivers that don't have a listener yet
            let new_sessions: Vec<(SessionId, broadcast::Receiver<EngineEvent>)> = {
                let tracked_read = tracked.read();
                let mut rx_map = receivers.write();
                let mut new = Vec::new();
                let keys: Vec<SessionId> = rx_map.keys().cloned().collect();
                for key in keys {
                    if !tracked_read.contains(&key) {
                        if let Some(rx) = rx_map.remove(&key) {
                            new.push((key, rx));
                        }
                    }
                }
                new
            };

            for (session_id, receiver) in new_sessions {
                tracked.write().push(session_id.clone());
                // Initialize empty state for this session
                states.write().entry(session_id.clone()).or_default();

                spawn_session_listener(session_id, receiver, states);
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    states
}

/// Spawn a Dioxus async task that drains a single session's receiver
/// and writes events into the per-session live state map.
fn spawn_session_listener(
    session_id: SessionId,
    mut receiver: broadcast::Receiver<EngineEvent>,
    mut states: LiveStates,
) {
    spawn(async move {
        loop {
            let event = match receiver.recv().await {
                Ok(event) => event,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        "engine event receiver for session {} lagged by {n} messages",
                        session_id
                    );
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            };

            let mut all_states = states.write();
            let state = all_states.entry(session_id.clone()).or_default();

            match event {
                EngineEvent::StepStatusChanged { step_name, status } => {
                    state.step_statuses.insert(step_name, status);
                }
                EngineEvent::WorkflowStarted { .. } => {
                    state.workflow_status = Some(SessionStatus::Running);
                    state.step_statuses.clear();
                }
                EngineEvent::WorkflowCompleted { .. } => {
                    state.workflow_status = Some(SessionStatus::Completed);
                }
                EngineEvent::WorkflowFailed { reason, .. } => {
                    state.workflow_status = Some(SessionStatus::Failed);
                    state.notifications.push(format!("Failed: {reason}"));
                }
                EngineEvent::NotificationRequired { step_name, message } => {
                    state.notifications.push(format!("[{step_name}] {message}"));
                }
                EngineEvent::AgentOutput { step_name, data } => {
                    let buf = state.agent_outputs.entry(step_name).or_default();
                    buf.extend_from_slice(&data);
                    if buf.len() > MAX_OUTPUT_BYTES_PER_STEP {
                        let drain = buf.len() - MAX_OUTPUT_BYTES_PER_STEP;
                        buf.drain(..drain);
                    }
                }
            }
        }
    });
}
