use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error};

use crate::traits::{NotificationContext, Notifier, NotifyError};

pub struct SlackNotifier {
    webhook_url: String,
    channel: Option<String>,
    client: Client,
}

impl SlackNotifier {
    pub fn new(webhook_url: String, channel: Option<String>) -> Self {
        Self {
            webhook_url,
            channel,
            client: Client::new(),
        }
    }

    fn format_message(&self, ctx: &NotificationContext) -> serde_json::Value {
        let status_emoji = match ctx.status.as_str() {
            "completed" => ":white_check_mark:",
            "failed" => ":x:",
            "running" => ":hourglass_flowing_sand:",
            _ => ":information_source:",
        };

        let mut text = format!(
            "{status_emoji} *{workflow}* \u{2014} {step}\nStatus: {status}",
            workflow = ctx.workflow_name,
            step = ctx.step_name,
            status = ctx.status,
        );

        if let Some(message) = &ctx.message {
            text.push_str(&format!("\n\n{message}"));
        }

        let mut payload = json!({ "text": text });

        if let Some(channel) = &self.channel {
            payload["channel"] = json!(channel);
        }

        payload
    }
}

#[async_trait]
impl Notifier for SlackNotifier {
    async fn send(&self, ctx: &NotificationContext) -> Result<(), NotifyError> {
        let payload = self.format_message(ctx);

        debug!("sending Slack notification");

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Slack webhook error: {status} \u{2014} {body}");
            return Err(NotifyError::SendError(format!(
                "Slack webhook returned {status}: {body}"
            )));
        }

        Ok(())
    }

    fn notifier_type(&self) -> &str {
        "slack"
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn make_context() -> NotificationContext {
        NotificationContext {
            workflow_name: "deploy".to_string(),
            step_name: "build".to_string(),
            status: "completed".to_string(),
            message: Some("Build succeeded".to_string()),
            data: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_slack_send_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .expect(1)
            .mount(&server)
            .await;

        let notifier = SlackNotifier::new(
            format!("{}/webhook", server.uri()),
            Some("#test".to_string()),
        );
        let ctx = make_context();
        notifier.send(&ctx).await.unwrap();
    }

    #[tokio::test]
    async fn test_slack_send_http_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .expect(1)
            .mount(&server)
            .await;

        let notifier = SlackNotifier::new(format!("{}/webhook", server.uri()), None);
        let ctx = make_context();
        let result = notifier.send(&ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"), "expected 500 in error: {err}");
    }
}
