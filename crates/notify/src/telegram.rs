use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use tracing::{debug, error};

use crate::traits::{NotificationContext, Notifier, NotifyError};

pub struct TelegramNotifier {
    bot_token: String,
    chat_id: String,
    client: Client,
    base_url: String,
}

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    chat_id: &'a str,
    text: &'a str,
    parse_mode: &'a str,
}

impl TelegramNotifier {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            client: Client::new(),
            base_url: "https://api.telegram.org".to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    fn format_message(&self, ctx: &NotificationContext) -> String {
        let status_emoji = match ctx.status.as_str() {
            "completed" => "\u{2705}",
            "failed" => "\u{274c}",
            "running" => "\u{23f3}",
            _ => "\u{2139}\u{fe0f}",
        };

        let mut msg = format!(
            "{status_emoji} *{workflow}* \u{2014} {step}\nStatus: {status}",
            workflow = escape_markdown(&ctx.workflow_name),
            step = escape_markdown(&ctx.step_name),
            status = escape_markdown(&ctx.status),
        );

        if let Some(message) = &ctx.message {
            msg.push_str(&format!("\n\n{}", escape_markdown(message)));
        }

        msg
    }
}

#[async_trait]
impl Notifier for TelegramNotifier {
    async fn send(&self, ctx: &NotificationContext) -> Result<(), NotifyError> {
        let text = self.format_message(ctx);
        let url = format!("{}/bot{}/sendMessage", self.base_url, self.bot_token);

        let request = SendMessageRequest {
            chat_id: &self.chat_id,
            text: &text,
            parse_mode: "MarkdownV2",
        };

        debug!("sending Telegram notification to chat {}", self.chat_id);

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Telegram API error: {status} \u{2014} {body}");
            return Err(NotifyError::SendError(format!(
                "Telegram API returned {status}: {body}"
            )));
        }

        Ok(())
    }

    fn notifier_type(&self) -> &str { "telegram" }
}

/// Escape special characters for Telegram MarkdownV2
fn escape_markdown(text: &str) -> String {
    let special_chars = [
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
    ];
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        if special_chars.contains(&ch) {
            result.push('\\');
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn make_context() -> NotificationContext {
        NotificationContext {
            workflow_name: "deploy".to_string(),
            step_name: "notify".to_string(),
            status: "completed".to_string(),
            message: Some("Deployed v1.0".to_string()),
            data: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_telegram_send_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path_regex(r"/bot.+/sendMessage"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true,"result":{}}"#))
            .expect(1)
            .mount(&server)
            .await;

        let notifier = TelegramNotifier::new("123:ABC".to_string(), "-100".to_string())
            .with_base_url(server.uri());

        let ctx = make_context();
        notifier.send(&ctx).await.unwrap();
    }

    #[tokio::test]
    async fn test_telegram_send_http_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path_regex(r"/bot.+/sendMessage"))
            .respond_with(ResponseTemplate::new(403).set_body_string("bot was blocked"))
            .expect(1)
            .mount(&server)
            .await;

        let notifier = TelegramNotifier::new("123:ABC".to_string(), "-100".to_string())
            .with_base_url(server.uri());

        let ctx = make_context();
        let result = notifier.send(&ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("403"), "expected 403 in error: {err}");
    }
}
