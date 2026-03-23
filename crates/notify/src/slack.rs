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
