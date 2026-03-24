use std::collections::HashMap;

use dioxus::prelude::*;
use fridi_core::engine::{EngineEvent, StepStatus};
use fridi_core::session::SessionStatus;
use tokio::sync::broadcast;

/// Maximum bytes retained per step in the agent output buffer.
/// Older output is discarded when this limit is exceeded.
const MAX_OUTPUT_BYTES_PER_STEP: usize = 512 * 1024;

/// Live workflow state updated by engine events
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LiveWorkflowState {
    pub(crate) step_statuses: HashMap<String, StepStatus>,
    pub(crate) workflow_status: Option<SessionStatus>,
    pub(crate) notifications: Vec<String>,
    pub(crate) agent_outputs: HashMap<String, Vec<u8>>,
}

/// Dioxus hook that subscribes to engine events and produces live workflow state.
///
/// The receiver signal should contain an `Option<broadcast::Receiver<EngineEvent>>`.
/// On first run the receiver is taken (via `Option::take`) so it can only be consumed once.
pub(crate) fn use_engine_events(
    mut rx: Signal<Option<broadcast::Receiver<EngineEvent>>>,
) -> Signal<LiveWorkflowState> {
    let mut state = use_signal(LiveWorkflowState::default);

    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        let Some(mut receiver) = rx.write().take() else {
            return;
        };
        while let Ok(event) = receiver.recv().await {
            match event {
                EngineEvent::StepStatusChanged { step_name, status } => {
                    state.write().step_statuses.insert(step_name, status);
                }
                EngineEvent::WorkflowStarted { .. } => {
                    let mut s = state.write();
                    s.workflow_status = Some(SessionStatus::Running);
                    s.step_statuses.clear();
                }
                EngineEvent::WorkflowCompleted { .. } => {
                    state.write().workflow_status = Some(SessionStatus::Completed);
                }
                EngineEvent::WorkflowFailed { reason, .. } => {
                    let mut s = state.write();
                    s.workflow_status = Some(SessionStatus::Failed);
                    s.notifications.push(format!("Failed: {reason}"));
                }
                EngineEvent::NotificationRequired { step_name, message } => {
                    state
                        .write()
                        .notifications
                        .push(format!("[{step_name}] {message}"));
                }
                EngineEvent::AgentOutput { step_name, data } => {
                    let mut s = state.write();
                    let buf = s.agent_outputs.entry(step_name).or_default();
                    buf.extend_from_slice(&data);
                    if buf.len() > MAX_OUTPUT_BYTES_PER_STEP {
                        let drain = buf.len() - MAX_OUTPUT_BYTES_PER_STEP;
                        buf.drain(..drain);
                    }
                }
            }
        }
    });

    state
}
