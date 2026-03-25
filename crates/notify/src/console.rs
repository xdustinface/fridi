use async_trait::async_trait;
use tracing::info;

use crate::traits::{NotificationContext, Notifier, NotifyError};

/// A notifier that prints to the console via tracing, useful for development and testing
pub struct ConsoleNotifier;

impl ConsoleNotifier {
    pub fn new() -> Self { Self }
}

impl Default for ConsoleNotifier {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Notifier for ConsoleNotifier {
    async fn send(&self, ctx: &NotificationContext) -> Result<(), NotifyError> {
        let status_icon = match ctx.status.as_str() {
            "completed" => "[OK]",
            "failed" => "[FAIL]",
            "running" => "[..]",
            _ => "[INFO]",
        };

        info!(
            "{status_icon} {workflow} \u{2014} {step}: {status}",
            workflow = ctx.workflow_name,
            step = ctx.step_name,
            status = ctx.status,
        );

        if let Some(message) = &ctx.message {
            info!("  Message: {message}");
        }

        Ok(())
    }

    fn notifier_type(&self) -> &str { "console" }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[tokio::test]
    async fn test_console_notifier() {
        let notifier = ConsoleNotifier::new();
        let ctx = NotificationContext {
            workflow_name: "test".to_string(),
            step_name: "step1".to_string(),
            status: "completed".to_string(),
            message: Some("All good".to_string()),
            data: HashMap::new(),
        };
        notifier.send(&ctx).await.unwrap();
        assert_eq!(notifier.notifier_type(), "console");
    }
}
