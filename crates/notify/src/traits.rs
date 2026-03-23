use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("failed to send notification: {0}")]
    SendError(String),
    #[error("configuration error: {0}")]
    ConfigError(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("rate limited, retry after {0}s")]
    RateLimited(u64),
}

/// Context passed to notification templates
#[derive(Debug, Clone)]
pub struct NotificationContext {
    pub workflow_name: String,
    pub step_name: String,
    pub status: String,
    pub message: Option<String>,
    /// Arbitrary key-value data from the step
    pub data: HashMap<String, JsonValue>,
}

#[async_trait]
pub trait Notifier: Send + Sync {
    /// Send a notification with the given context
    async fn send(&self, ctx: &NotificationContext) -> Result<(), NotifyError>;

    /// The notifier type name (e.g., "telegram", "slack", "console")
    fn notifier_type(&self) -> &str;
}

/// Simple message template rendering.
/// Replaces `{{field}}` with values from context.
pub fn render_template(template: &str, ctx: &NotificationContext) -> String {
    let mut result = template.to_string();
    result = result.replace("{{workflow}}", &ctx.workflow_name);
    result = result.replace("{{step}}", &ctx.step_name);
    result = result.replace("{{status}}", &ctx.status);
    if let Some(msg) = &ctx.message {
        result = result.replace("{{message}}", msg);
    }
    // Replace {{data.key}} patterns
    for (key, value) in &ctx.data {
        let pattern = format!("{{{{data.{key}}}}}");
        let replacement = match value {
            JsonValue::String(s) => s.clone(),
            other => other.to_string(),
        };
        result = result.replace(&pattern, &replacement);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context() -> NotificationContext {
        let mut data = HashMap::new();
        data.insert("pr_count".to_string(), serde_json::json!(3));
        data.insert("repo".to_string(), serde_json::json!("owner/repo"));

        NotificationContext {
            workflow_name: "pr-babysitter".to_string(),
            step_name: "check-prs".to_string(),
            status: "completed".to_string(),
            message: Some("Found 3 PRs needing attention".to_string()),
            data,
        }
    }

    #[test]
    fn test_render_template_basic() {
        let ctx = make_context();
        let result = render_template("Workflow {{workflow}} step {{step}} is {{status}}", &ctx);
        assert_eq!(result, "Workflow pr-babysitter step check-prs is completed");
    }

    #[test]
    fn test_render_template_with_data() {
        let ctx = make_context();
        let result = render_template("Found {{data.pr_count}} PRs in {{data.repo}}", &ctx);
        assert_eq!(result, "Found 3 PRs in owner/repo");
    }

    #[test]
    fn test_render_template_with_message() {
        let ctx = make_context();
        let result = render_template("{{message}}", &ctx);
        assert_eq!(result, "Found 3 PRs needing attention");
    }

    #[test]
    fn test_render_template_no_match() {
        let ctx = make_context();
        let result = render_template("no templates here", &ctx);
        assert_eq!(result, "no templates here");
    }
}
