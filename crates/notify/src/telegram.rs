use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use tracing::{debug, error};

use crate::traits::{NotificationContext, Notifier, NotifyError};

pub struct TelegramNotifier {
    bot_token: String,
    chat_id: String,
    client: Client,
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
        }
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
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);

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

    fn notifier_type(&self) -> &str {
        "telegram"
    }
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
