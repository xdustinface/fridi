use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("failed to read workflow file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse workflow YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("validation error: {0}")]
    Validation(String),
}

/// Top-level workflow definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub config: WorkflowConfig,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub notifications: NotificationConfig,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub repo: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Cron { schedule: String },
    Manual,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub slack: Option<SlackConfig>,
    pub telegram: Option<TelegramConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlackConfig {
    pub webhook_url: String,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub agent: Option<String>,
    pub skill: Option<String>,
    pub args: Option<String>,
    pub prompt: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub condition: Option<String>,
    pub for_each: Option<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    pub on_failure: Option<OnFailure>,
    pub retry: Option<RetryConfig>,
    #[serde(rename = "type")]
    pub step_type: Option<StepType>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Agent,
    Notification,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnFailure {
    Notify,
    Stop,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    pub until: Option<String>,
    pub max_attempts: Option<u32>,
    pub interval: Option<String>,
}

impl Workflow {
    pub fn from_file(path: &Path) -> Result<Self, SchemaError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml(&content)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, SchemaError> {
        let workflow: Workflow = serde_yaml::from_str(yaml)?;
        workflow.validate()?;
        Ok(workflow)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        if self.name.is_empty() {
            return Err(SchemaError::Validation(
                "workflow name cannot be empty".into(),
            ));
        }
        if self.steps.is_empty() {
            return Err(SchemaError::Validation(
                "workflow must have at least one step".into(),
            ));
        }

        let mut seen = HashSet::new();
        for step in &self.steps {
            if !seen.insert(&step.name) {
                return Err(SchemaError::Validation(format!(
                    "duplicate step name: {}",
                    step.name
                )));
            }
        }

        let step_names: HashSet<&str> = self.steps.iter().map(|s| s.name.as_str()).collect();
        for step in &self.steps {
            for dep in &step.depends_on {
                if !step_names.contains(dep.as_str()) {
                    return Err(SchemaError::Validation(format!(
                        "step '{}' depends on unknown step '{}'",
                        step.name, dep
                    )));
                }
            }
            if step.depends_on.contains(&step.name) {
                return Err(SchemaError::Validation(format!(
                    "step '{}' cannot depend on itself",
                    step.name
                )));
            }
        }

        Ok(())
    }
}

/// Interpolate `${ENV_VAR}` patterns with environment variable values.
pub fn interpolate_env(input: &str) -> String {
    let mut result = input.to_string();
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let value = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                value,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }
    result
}

/// Replace `${FRIDI_REPO}` with the provided repo value.
/// Other `${VAR}` patterns are resolved from the environment.
pub fn interpolate_with_repo(input: &str, repo: &str) -> String {
    let replaced = input.replace("${FRIDI_REPO}", repo);
    interpolate_env(&replaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_workflow() {
        let yaml = r#"
name: test
steps:
  - name: step1
    agent: claude
    skill: recap
"#;
        let workflow = Workflow::from_yaml(yaml).unwrap();
        assert_eq!(workflow.name, "test");
        assert_eq!(workflow.steps.len(), 1);
    }

    #[test]
    fn test_parse_full_workflow() {
        let yaml = r##"
name: full-test
description: A full workflow
config:
  repo: "owner/repo"
triggers:
  - type: cron
    schedule: "*/5 * * * *"
  - type: manual
notifications:
  slack:
    webhook_url: "https://hooks.slack.com/test"
    channel: "#test"
steps:
  - name: step1
    agent: claude
    skill: recap
    outputs: [result]
  - name: step2
    agent: claude
    skill: fixup
    depends_on: [step1]
    condition: "steps.step1.outputs.result > 0"
    retry:
      max_attempts: 3
      interval: "1m"
    on_failure: notify
"##;
        let workflow = Workflow::from_yaml(yaml).unwrap();
        assert_eq!(workflow.name, "full-test");
        assert_eq!(workflow.triggers.len(), 2);
        assert!(workflow.notifications.slack.is_some());
        assert_eq!(workflow.steps.len(), 2);
        assert_eq!(workflow.steps[1].depends_on, vec!["step1"]);
    }

    #[test]
    fn test_empty_name_rejected() {
        let yaml = "name: \"\"\nsteps:\n  - name: step1\n    agent: claude\n";
        assert!(Workflow::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_no_steps_rejected() {
        let yaml = "name: test\nsteps: []\n";
        assert!(Workflow::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_duplicate_step_names_rejected() {
        let yaml =
            "name: test\nsteps:\n  - name: s\n    agent: claude\n  - name: s\n    agent: claude\n";
        assert!(Workflow::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_unknown_dependency_rejected() {
        let yaml = "name: test\nsteps:\n  - name: s\n    agent: claude\n    depends_on: [nope]\n";
        assert!(Workflow::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_self_dependency_rejected() {
        let yaml = "name: test\nsteps:\n  - name: s\n    agent: claude\n    depends_on: [s]\n";
        assert!(Workflow::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_env_interpolation() {
        std::env::set_var("TEST_FRIDI_VAR", "hello");
        assert_eq!(interpolate_env("${TEST_FRIDI_VAR}"), "hello");
        assert_eq!(interpolate_env("no_vars"), "no_vars");
        std::env::remove_var("TEST_FRIDI_VAR");
    }

    #[test]
    fn test_load_pr_babysitter_yaml() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("workflows/pr-babysitter.yaml");
        let workflow = Workflow::from_file(&path).unwrap();
        assert_eq!(workflow.name, "pr-babysitter");
        assert_eq!(
            workflow.description.as_deref(),
            Some("Monitor PRs for issues, review them, fix problems, watch CI until green")
        );

        // Triggers: cron + manual
        assert_eq!(workflow.triggers.len(), 2);
        assert!(
            matches!(&workflow.triggers[0], Trigger::Cron { schedule } if schedule == "*/30 * * * *")
        );
        assert!(matches!(&workflow.triggers[1], Trigger::Manual));

        // Notifications configured
        assert!(workflow.notifications.slack.is_some());
        assert!(workflow.notifications.telegram.is_some());

        // Five steps with correct names and ordering
        assert_eq!(workflow.steps.len(), 5);
        let names: Vec<&str> = workflow.steps.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["check-prs", "review", "fix", "watch-ci", "notify-complete"]
        );

        // check-prs has no dependencies
        assert!(workflow.steps[0].depends_on.is_empty());
        assert!(workflow.steps[0].prompt.is_some());

        // review depends on check-prs and has a condition
        assert_eq!(workflow.steps[1].depends_on, vec!["check-prs"]);
        assert!(workflow.steps[1].condition.is_some());
        assert!(workflow.steps[1].for_each.is_some());
        assert_eq!(workflow.steps[1].on_failure, Some(OnFailure::Notify));

        // watch-ci has retry config
        let retry = workflow.steps[3].retry.as_ref().unwrap();
        assert_eq!(retry.max_attempts, Some(10));
        assert_eq!(retry.interval.as_deref(), Some("5m"));

        // notify-complete is a notification step
        assert_eq!(workflow.steps[4].step_type, Some(StepType::Notification));
        assert!(workflow.steps[4].message.is_some());
    }
}
