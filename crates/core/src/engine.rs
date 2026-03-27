use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

use crate::dag::WorkflowDag;
use crate::schema::{OnFailure, Step, Workflow};

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("dag error: {0}")]
    Dag(#[from] crate::dag::DagError),
    #[error("step '{step}' failed: {reason}")]
    StepFailed { step: String, reason: String },
    #[error("workflow cancelled")]
    Cancelled,
    #[error("no agent registered for type '{0}'")]
    NoAgent(String),
}

/// Lifecycle state of a step
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Skipped,
}

/// Event emitted by the engine for UI consumption
#[derive(Debug, Clone)]
pub enum EngineEvent {
    WorkflowStarted {
        workflow_name: String,
    },
    StepStatusChanged {
        step_name: String,
        status: StepStatus,
    },
    WorkflowCompleted {
        workflow_name: String,
    },
    WorkflowFailed {
        workflow_name: String,
        reason: String,
    },
    NotificationRequired {
        step_name: String,
        message: String,
    },
    AgentOutput {
        step_name: String,
        data: Vec<u8>,
    },
}

/// Shared context that accumulates step outputs
#[derive(Debug, Clone, Default)]
pub struct WorkflowContext {
    step_outputs: HashMap<String, JsonValue>,
    config: HashMap<String, JsonValue>,
}

impl WorkflowContext {
    pub fn new(config: HashMap<String, JsonValue>) -> Self {
        Self {
            step_outputs: HashMap::new(),
            config,
        }
    }

    pub fn set_step_output(&mut self, step_name: &str, output: JsonValue) {
        self.step_outputs.insert(step_name.to_string(), output);
    }

    pub fn get_step_output(&self, step_name: &str) -> Option<&JsonValue> {
        self.step_outputs.get(step_name)
    }

    pub fn get_config(&self, key: &str) -> Option<&JsonValue> { self.config.get(key) }

    /// Build context map for passing to an agent
    pub fn as_agent_context(&self) -> HashMap<String, JsonValue> {
        let mut ctx = self.config.clone();
        ctx.insert(
            "steps".to_string(),
            serde_json::to_value(&self.step_outputs).unwrap_or_default(),
        );
        ctx
    }
}

/// Trait that the engine uses to spawn agents -- decoupled from concrete agent impls
pub trait AgentSpawner: Send + Sync {
    fn spawn_step(
        &self,
        step: &Step,
        context: &WorkflowContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<StepResult, String>> + Send>>;
}

/// Result from running a step
#[derive(Debug, Clone)]
pub struct StepResult {
    pub exit_code: i32,
    pub output: String,
    pub structured_output: Option<JsonValue>,
}

/// The workflow execution engine
pub struct Engine {
    event_tx: broadcast::Sender<EngineEvent>,
}

impl Engine {
    pub fn new() -> (Self, broadcast::Receiver<EngineEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);
        (Self { event_tx }, event_rx)
    }

    /// Subscribe to engine events
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> { self.event_tx.subscribe() }

    /// Get a clone of the event sender for forwarding events from external sources
    pub fn event_sender(&self) -> broadcast::Sender<EngineEvent> { self.event_tx.clone() }

    fn emit(&self, event: EngineEvent) { let _ = self.event_tx.send(event); }

    /// Execute a workflow to completion, respecting the DAG ordering
    pub async fn execute(
        &self,
        workflow: &Workflow,
        dag: &WorkflowDag,
        spawner: &dyn AgentSpawner,
    ) -> Result<WorkflowContext, EngineError> {
        info!("starting workflow '{}'", workflow.name);
        self.emit(EngineEvent::WorkflowStarted {
            workflow_name: workflow.name.clone(),
        });

        let mut context = WorkflowContext::new(workflow.config.extra.clone());
        if let Some(repo) = &workflow.config.repo {
            context
                .config
                .insert("repo".to_string(), JsonValue::String(repo.clone()));
        }

        let step_map: HashMap<&str, &Step> = workflow
            .steps
            .iter()
            .map(|s| (s.name.as_str(), s))
            .collect();
        let mut completed: HashSet<String> = HashSet::new();
        let mut statuses: HashMap<String, StepStatus> = HashMap::new();

        for step in &workflow.steps {
            statuses.insert(step.name.clone(), StepStatus::Pending);
        }

        loop {
            let ready = dag.ready_steps(&completed);
            if ready.is_empty() {
                if completed.len() == dag.step_count() {
                    break;
                }
                let all_accounted = workflow.steps.iter().all(|s| {
                    completed.contains(&s.name)
                        || matches!(
                            statuses.get(&s.name),
                            Some(StepStatus::Skipped) | Some(StepStatus::Failed(_))
                        )
                });
                if all_accounted {
                    break;
                }
                error!("deadlock: no ready steps but workflow not complete");
                return Err(EngineError::StepFailed {
                    step: "workflow".into(),
                    reason: "deadlock detected".into(),
                });
            }

            // Spawn all ready steps concurrently via JoinSet
            let mut join_set: JoinSet<(String, Step, Result<StepResult, String>)> = JoinSet::new();

            for step_name in &ready {
                let step = step_map[step_name.as_str()];

                if let Some(condition) = &step.condition {
                    if !evaluate_condition(condition, &context) {
                        info!("skipping step '{}': condition not met", step_name);
                        self.emit(EngineEvent::StepStatusChanged {
                            step_name: step_name.clone(),
                            status: StepStatus::Skipped,
                        });
                        statuses.insert(step_name.clone(), StepStatus::Skipped);
                        completed.insert(step_name.clone());
                        continue;
                    }
                }

                self.emit(EngineEvent::StepStatusChanged {
                    step_name: step_name.clone(),
                    status: StepStatus::Running,
                });
                statuses.insert(step_name.clone(), StepStatus::Running);

                let step_clone = step.clone();
                let ctx = context.clone();
                let name = step_name.clone();

                // Create the future here (borrows spawner), then move it into the task
                let fut = spawner.spawn_step(&step_clone, &ctx);
                join_set.spawn(async move {
                    let result = fut.await;
                    (name, step_clone, result)
                });
            }

            // Collect all concurrent results
            let mut first_attempt_results = Vec::new();
            while let Some(join_result) = join_set.join_next().await {
                match join_result {
                    Ok(result) => first_attempt_results.push(result),
                    Err(e) => {
                        error!("step task panicked: {}", e);
                    }
                }
            }

            // Process results and retry failures sequentially
            for (step_name, step, result) in first_attempt_results {
                let max_attempts = step
                    .retry
                    .as_ref()
                    .and_then(|r| r.max_attempts)
                    .unwrap_or(1);

                let interval = step
                    .retry
                    .as_ref()
                    .and_then(|r| r.interval.as_ref())
                    .and_then(|i| parse_duration(i))
                    .unwrap_or(Duration::from_secs(60));

                let mut attempt = 1;
                let mut last_error = String::new();
                let mut success =
                    handle_step_result(result, &step_name, &mut context, &mut last_error);
                if success {
                    self.emit(EngineEvent::StepStatusChanged {
                        step_name: step_name.clone(),
                        status: StepStatus::Completed,
                    });
                    statuses.insert(step_name.clone(), StepStatus::Completed);
                    completed.insert(step_name.clone());
                }

                // Retry loop for subsequent attempts (sequential per step)
                while !success && attempt < max_attempts {
                    attempt += 1;
                    info!(
                        "retrying step '{}' (attempt {}/{})",
                        step_name, attempt, max_attempts
                    );
                    tokio::time::sleep(interval).await;

                    success = handle_step_result(
                        spawner.spawn_step(&step, &context).await,
                        &step_name,
                        &mut context,
                        &mut last_error,
                    );
                    if success {
                        self.emit(EngineEvent::StepStatusChanged {
                            step_name: step_name.clone(),
                            status: StepStatus::Completed,
                        });
                        statuses.insert(step_name.clone(), StepStatus::Completed);
                        completed.insert(step_name.clone());
                    }
                }

                if !success {
                    let status = StepStatus::Failed(last_error.clone());
                    self.emit(EngineEvent::StepStatusChanged {
                        step_name: step_name.clone(),
                        status: status.clone(),
                    });
                    statuses.insert(step_name.clone(), status);

                    match step.on_failure.as_ref().unwrap_or(&OnFailure::Stop) {
                        OnFailure::Notify => {
                            self.emit(EngineEvent::NotificationRequired {
                                step_name: step_name.clone(),
                                message: format!("Step '{}' failed: {}", step_name, last_error),
                            });
                            completed.insert(step_name.clone());
                        }
                        OnFailure::Continue => {
                            completed.insert(step_name.clone());
                        }
                        OnFailure::Stop => {
                            self.emit(EngineEvent::WorkflowFailed {
                                workflow_name: workflow.name.clone(),
                                reason: format!("step '{}' failed: {}", step_name, last_error),
                            });
                            return Err(EngineError::StepFailed {
                                step: step_name,
                                reason: last_error,
                            });
                        }
                    }
                }
            }
        }

        info!("workflow '{}' completed", workflow.name);
        self.emit(EngineEvent::WorkflowCompleted {
            workflow_name: workflow.name.clone(),
        });

        Ok(context)
    }
}

impl Default for Engine {
    fn default() -> Self { Self::new().0 }
}

/// Process a step result, returning true if the step succeeded
fn handle_step_result(
    result: Result<StepResult, String>,
    step_name: &str,
    context: &mut WorkflowContext,
    last_error: &mut String,
) -> bool {
    match result {
        Ok(step_result) => {
            let output_value = step_result
                .structured_output
                .unwrap_or_else(|| JsonValue::String(step_result.output.clone()));
            context.set_step_output(step_name, output_value);

            if step_result.exit_code == 0 {
                info!("step '{}' completed successfully", step_name);
                true
            } else {
                *last_error = format!("exit code {}", step_result.exit_code);
                warn!(
                    "step '{}' failed with exit code {}",
                    step_name, step_result.exit_code
                );
                false
            }
        }
        Err(e) => {
            *last_error = e.clone();
            warn!("step '{}' error: {}", step_name, e);
            false
        }
    }
}

/// Simple condition evaluator.
/// Supports "true", "false", and step output references like "steps.<name>".
fn evaluate_condition(condition: &str, context: &WorkflowContext) -> bool {
    let trimmed = condition.trim();
    match trimmed {
        "true" => true,
        "false" => false,
        _ => {
            debug!("evaluating condition: {}", trimmed);
            if trimmed.starts_with("steps.") {
                let parts: Vec<&str> = trimmed.split('.').collect();
                if parts.len() >= 2 {
                    let step_name = parts[1];
                    if let Some(output) = context.get_step_output(step_name) {
                        return is_truthy(output);
                    }
                }
                false
            } else {
                true
            }
        }
    }
}

fn is_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(b) => *b,
        JsonValue::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        JsonValue::String(s) => !s.is_empty(),
        JsonValue::Array(a) => !a.is_empty(),
        JsonValue::Object(_) => true,
    }
}

/// Parse a duration string like "5m", "1h", "30s"
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: u64 = num_str.parse().ok()?;

    match unit {
        "s" => Some(Duration::from_secs(num)),
        "m" => Some(Duration::from_secs(num * 60)),
        "h" => Some(Duration::from_secs(num * 3600)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct MockSpawner {
        results: Mutex<HashMap<String, Result<StepResult, String>>>,
        call_order: Mutex<Vec<String>>,
    }

    impl MockSpawner {
        fn new() -> Self {
            Self {
                results: Mutex::new(HashMap::new()),
                call_order: Mutex::new(Vec::new()),
            }
        }

        fn set_result(&self, step_name: &str, result: Result<StepResult, String>) {
            self.results
                .lock()
                .unwrap()
                .insert(step_name.to_string(), result);
        }

        fn call_order(&self) -> Vec<String> { self.call_order.lock().unwrap().clone() }
    }

    impl AgentSpawner for MockSpawner {
        fn spawn_step(
            &self,
            step: &Step,
            _context: &WorkflowContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<StepResult, String>> + Send>>
        {
            self.call_order.lock().unwrap().push(step.name.clone());
            let result = self
                .results
                .lock()
                .unwrap()
                .get(&step.name)
                .cloned()
                .unwrap_or(Ok(StepResult {
                    exit_code: 0,
                    output: String::new(),
                    structured_output: None,
                }));
            Box::pin(async move { result })
        }
    }

    fn ok_result() -> Result<StepResult, String> {
        Ok(StepResult {
            exit_code: 0,
            output: "success".to_string(),
            structured_output: Some(JsonValue::Bool(true)),
        })
    }

    fn fail_result() -> Result<StepResult, String> {
        Ok(StepResult {
            exit_code: 1,
            output: "failed".to_string(),
            structured_output: None,
        })
    }

    #[tokio::test]
    async fn test_simple_linear_workflow() {
        let yaml = r#"
name: test
steps:
  - name: a
    agent: claude
  - name: b
    agent: claude
    depends_on: [a]
"#;
        let wf = Workflow::from_yaml(yaml).unwrap();
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let (engine, _rx) = Engine::new();
        let spawner = MockSpawner::new();
        spawner.set_result("a", ok_result());
        spawner.set_result("b", ok_result());

        let ctx = engine.execute(&wf, &dag, &spawner).await.unwrap();
        assert!(ctx.get_step_output("a").is_some());
        assert!(ctx.get_step_output("b").is_some());
        let order = spawner.call_order();
        assert_eq!(order, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_condition_skip() {
        let yaml = r#"
name: test
steps:
  - name: a
    agent: claude
  - name: b
    agent: claude
    depends_on: [a]
    condition: "false"
"#;
        let wf = Workflow::from_yaml(yaml).unwrap();
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let (engine, _rx) = Engine::new();
        let spawner = MockSpawner::new();
        spawner.set_result("a", ok_result());

        engine.execute(&wf, &dag, &spawner).await.unwrap();
        let order = spawner.call_order();
        assert_eq!(order, vec!["a"]);
    }

    #[tokio::test]
    async fn test_on_failure_stop() {
        let yaml = r#"
name: test
steps:
  - name: a
    agent: claude
    on_failure: stop
  - name: b
    agent: claude
    depends_on: [a]
"#;
        let wf = Workflow::from_yaml(yaml).unwrap();
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let (engine, _rx) = Engine::new();
        let spawner = MockSpawner::new();
        spawner.set_result("a", fail_result());

        let result = engine.execute(&wf, &dag, &spawner).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_on_failure_continue() {
        let yaml = r#"
name: test
steps:
  - name: a
    agent: claude
    on_failure: continue
  - name: b
    agent: claude
    depends_on: [a]
"#;
        let wf = Workflow::from_yaml(yaml).unwrap();
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let (engine, _rx) = Engine::new();
        let spawner = MockSpawner::new();
        spawner.set_result("a", fail_result());
        spawner.set_result("b", ok_result());

        let ctx = engine.execute(&wf, &dag, &spawner).await.unwrap();
        assert!(ctx.get_step_output("b").is_some());
    }

    #[tokio::test]
    async fn test_on_failure_notify() {
        let yaml = r#"
name: test
steps:
  - name: a
    agent: claude
    on_failure: notify
  - name: b
    agent: claude
    depends_on: [a]
"#;
        let wf = Workflow::from_yaml(yaml).unwrap();
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let (engine, mut rx) = Engine::new();
        let spawner = MockSpawner::new();
        spawner.set_result("a", fail_result());
        spawner.set_result("b", ok_result());

        engine.execute(&wf, &dag, &spawner).await.unwrap();

        let mut got_notification = false;
        while let Ok(event) = rx.try_recv() {
            if matches!(event, EngineEvent::NotificationRequired { .. }) {
                got_notification = true;
            }
        }
        assert!(got_notification);
    }

    #[tokio::test]
    async fn test_context_passing() {
        let yaml = r#"
name: test
steps:
  - name: a
    agent: claude
  - name: b
    agent: claude
    depends_on: [a]
    condition: "steps.a"
"#;
        let wf = Workflow::from_yaml(yaml).unwrap();
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let (engine, _rx) = Engine::new();
        let spawner = MockSpawner::new();
        spawner.set_result("a", ok_result());
        spawner.set_result("b", ok_result());

        let ctx = engine.execute(&wf, &dag, &spawner).await.unwrap();
        assert!(ctx.get_step_output("a").is_some());
        assert!(ctx.get_step_output("b").is_some());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5s"), Some(Duration::from_secs(5)));
        assert_eq!(parse_duration("10m"), Some(Duration::from_secs(600)));
        assert_eq!(parse_duration("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("abc"), None);
    }

    #[test]
    fn test_evaluate_condition() {
        let ctx = WorkflowContext::default();
        assert!(evaluate_condition("true", &ctx));
        assert!(!evaluate_condition("false", &ctx));
    }

    #[tokio::test]
    async fn test_engine_events_flow_through_mock_spawner() {
        let yaml = r#"
name: event-test
steps:
  - name: emit-step
    agent: claude
"#;
        let wf = Workflow::from_yaml(yaml).unwrap();
        let dag = WorkflowDag::from_workflow(&wf).unwrap();

        // Create an engine and a mock spawner that emits AgentOutput via event_tx
        let (engine, mut rx) = Engine::new();
        let event_tx = engine.event_sender();

        struct EventEmittingSpawner {
            event_tx: broadcast::Sender<EngineEvent>,
        }

        impl AgentSpawner for EventEmittingSpawner {
            fn spawn_step(
                &self,
                step: &Step,
                _context: &WorkflowContext,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<StepResult, String>> + Send>,
            > {
                let tx = self.event_tx.clone();
                let name = step.name.clone();
                Box::pin(async move {
                    let _ = tx.send(EngineEvent::AgentOutput {
                        step_name: name,
                        data: b"mock output data".to_vec(),
                    });
                    Ok(StepResult {
                        exit_code: 0,
                        output: "done".to_string(),
                        structured_output: None,
                    })
                })
            }
        }

        let spawner = EventEmittingSpawner { event_tx };

        engine.execute(&wf, &dag, &spawner).await.unwrap();

        // Drain all events and check we got the expected types
        let mut got_started = false;
        let mut got_running = false;
        let mut got_agent_output = false;
        let mut got_completed_step = false;
        let mut got_workflow_completed = false;

        while let Ok(event) = rx.try_recv() {
            match event {
                EngineEvent::WorkflowStarted { .. } => got_started = true,
                EngineEvent::StepStatusChanged { status, .. } => match status {
                    StepStatus::Running => got_running = true,
                    StepStatus::Completed => got_completed_step = true,
                    _ => {}
                },
                EngineEvent::AgentOutput { data, .. } => {
                    assert_eq!(data, b"mock output data");
                    got_agent_output = true;
                }
                EngineEvent::WorkflowCompleted { .. } => got_workflow_completed = true,
                _ => {}
            }
        }

        assert!(got_started, "should receive WorkflowStarted event");
        assert!(
            got_running,
            "should receive StepStatusChanged(Running) event"
        );
        assert!(
            got_agent_output,
            "should receive AgentOutput event from mock spawner"
        );
        assert!(
            got_completed_step,
            "should receive StepStatusChanged(Completed) event"
        );
        assert!(
            got_workflow_completed,
            "should receive WorkflowCompleted event"
        );
    }

    #[test]
    fn test_is_truthy() {
        assert!(!is_truthy(&JsonValue::Null));
        assert!(!is_truthy(&JsonValue::Bool(false)));
        assert!(is_truthy(&JsonValue::Bool(true)));
        assert!(!is_truthy(&JsonValue::Number(serde_json::Number::from(0))));
        assert!(is_truthy(&JsonValue::Number(serde_json::Number::from(1))));
        assert!(!is_truthy(&JsonValue::String(String::new())));
        assert!(is_truthy(&JsonValue::String("hello".into())));
    }

    #[tokio::test]
    async fn test_parallel_steps_run_concurrently() {
        use std::sync::Arc;

        use tokio::sync::Barrier;

        let yaml = r#"
name: parallel-test
steps:
  - name: a
    agent: claude
  - name: b
    agent: claude
  - name: c
    agent: claude
"#;
        let wf = Workflow::from_yaml(yaml).unwrap();
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let (engine, _rx) = Engine::new();

        // A barrier that requires all 3 steps to arrive before any can proceed.
        // If steps ran sequentially, the barrier would never complete.
        let barrier = Arc::new(Barrier::new(3));

        struct BarrierSpawner {
            barrier: Arc<Barrier>,
        }

        impl AgentSpawner for BarrierSpawner {
            fn spawn_step(
                &self,
                _step: &Step,
                _context: &WorkflowContext,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<StepResult, String>> + Send>,
            > {
                let b = self.barrier.clone();
                Box::pin(async move {
                    // All 3 steps must reach the barrier concurrently
                    b.wait().await;
                    Ok(StepResult {
                        exit_code: 0,
                        output: "done".to_string(),
                        structured_output: None,
                    })
                })
            }
        }

        let spawner = BarrierSpawner { barrier };

        // With a 2-second timeout: if steps are sequential the barrier deadlocks
        let result =
            tokio::time::timeout(Duration::from_secs(2), engine.execute(&wf, &dag, &spawner)).await;

        assert!(
            result.is_ok(),
            "parallel steps should complete within timeout (would deadlock if sequential)"
        );
        let ctx = result.unwrap().unwrap();
        assert!(ctx.get_step_output("a").is_some());
        assert!(ctx.get_step_output("b").is_some());
        assert!(ctx.get_step_output("c").is_some());
    }
}
