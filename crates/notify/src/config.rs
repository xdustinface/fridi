use std::env;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::console::ConsoleNotifier;
use crate::slack::SlackNotifier;
use crate::telegram::TelegramNotifier;
use crate::traits::{Notifier, NotifyError};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotifyConfig {
    pub slack: Option<SlackConfig>,
    pub telegram: Option<TelegramConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub webhook_url: String,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

/// Wrapper for deserializing the top-level `[notifications]` section from a TOML file
#[derive(Deserialize)]
struct ConfigFile {
    notifications: Option<NotifyConfig>,
}

impl NotifyConfig {
    pub fn from_file(path: &Path) -> Result<Self, NotifyError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| NotifyError::ConfigError(format!("failed to read config: {e}")))?;
        let file: ConfigFile = toml::from_str(&content)
            .map_err(|e| NotifyError::ConfigError(format!("failed to parse config: {e}")))?;
        Ok(file.notifications.unwrap_or_default())
    }

    pub fn from_env() -> Self {
        let slack = env::var("CONDUCTOR_SLACK_WEBHOOK_URL").ok().map(|url| {
            SlackConfig {
                webhook_url: url,
                channel: env::var("CONDUCTOR_SLACK_CHANNEL").ok(),
            }
        });

        let telegram = env::var("CONDUCTOR_TELEGRAM_BOT_TOKEN")
            .ok()
            .and_then(|token| {
                env::var("CONDUCTOR_TELEGRAM_CHAT_ID")
                    .ok()
                    .map(|chat_id| TelegramConfig {
                        bot_token: token,
                        chat_id,
                    })
            });

        Self { slack, telegram }
    }

    pub fn build_notifiers(&self) -> Vec<Box<dyn Notifier + Send + Sync>> {
        let mut notifiers: Vec<Box<dyn Notifier + Send + Sync>> = vec![
            Box::new(ConsoleNotifier::new()),
        ];

        if let Some(slack) = &self.slack {
            notifiers.push(Box::new(SlackNotifier::new(
                slack.webhook_url.clone(),
                slack.channel.clone(),
            )));
        }

        if let Some(telegram) = &self.telegram {
            notifiers.push(Box::new(TelegramNotifier::new(
                telegram.bot_token.clone(),
                telegram.chat_id.clone(),
            )));
        }

        notifiers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        temp_env::with_vars(
            [
                ("CONDUCTOR_SLACK_WEBHOOK_URL", Some("https://hooks.slack.com/test")),
                ("CONDUCTOR_SLACK_CHANNEL", Some("#alerts")),
                ("CONDUCTOR_TELEGRAM_BOT_TOKEN", Some("123:ABC")),
                ("CONDUCTOR_TELEGRAM_CHAT_ID", Some("-100123")),
            ],
            || {
                let config = NotifyConfig::from_env();

                let slack = config.slack.unwrap();
                assert_eq!(slack.webhook_url, "https://hooks.slack.com/test");
                assert_eq!(slack.channel.as_deref(), Some("#alerts"));

                let telegram = config.telegram.unwrap();
                assert_eq!(telegram.bot_token, "123:ABC");
                assert_eq!(telegram.chat_id, "-100123");
            },
        );
    }

    #[test]
    fn test_config_build_notifiers_all() {
        let config = NotifyConfig {
            slack: Some(SlackConfig {
                webhook_url: "https://hooks.slack.com/test".to_string(),
                channel: None,
            }),
            telegram: Some(TelegramConfig {
                bot_token: "123:ABC".to_string(),
                chat_id: "-100123".to_string(),
            }),
        };

        let notifiers = config.build_notifiers();
        assert_eq!(notifiers.len(), 3);
        assert_eq!(notifiers[0].notifier_type(), "console");
        assert_eq!(notifiers[1].notifier_type(), "slack");
        assert_eq!(notifiers[2].notifier_type(), "telegram");
    }

    #[test]
    fn test_config_build_notifiers_console_only() {
        let config = NotifyConfig::default();
        let notifiers = config.build_notifiers();
        assert_eq!(notifiers.len(), 1);
        assert_eq!(notifiers[0].notifier_type(), "console");
    }
}
