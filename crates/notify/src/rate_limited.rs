use std::time::Duration;

use async_trait::async_trait;

use crate::rate_limiter::RateLimiter;
use crate::traits::{NotificationContext, Notifier, NotifyError};

pub struct RateLimitedNotifier<N: Notifier> {
    inner: N,
    rate_limiter: RateLimiter,
}

impl<N: Notifier> RateLimitedNotifier<N> {
    pub fn new(inner: N, min_interval: Duration) -> Self {
        Self {
            inner,
            rate_limiter: RateLimiter::new(min_interval),
        }
    }
}

#[async_trait]
impl<N: Notifier> Notifier for RateLimitedNotifier<N> {
    async fn send(&self, ctx: &NotificationContext) -> Result<(), NotifyError> {
        let key = format!(
            "{}:{}:{}",
            self.inner.notifier_type(),
            ctx.workflow_name,
            ctx.step_name
        );
        self.rate_limiter.check(&key).await?;
        self.inner.send(ctx).await
    }

    fn notifier_type(&self) -> &str {
        self.inner.notifier_type()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::console::ConsoleNotifier;

    fn make_context() -> NotificationContext {
        NotificationContext {
            workflow_name: "test-wf".to_string(),
            step_name: "step1".to_string(),
            status: "completed".to_string(),
            message: None,
            data: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_rate_limited_notifier_passes_through() {
        let notifier = RateLimitedNotifier::new(ConsoleNotifier::new(), Duration::from_secs(60));
        let ctx = make_context();
        assert!(notifier.send(&ctx).await.is_ok());
        assert_eq!(notifier.notifier_type(), "console");
    }

    #[tokio::test]
    async fn test_rate_limited_notifier_blocks_spam() {
        let notifier = RateLimitedNotifier::new(ConsoleNotifier::new(), Duration::from_secs(60));
        let ctx = make_context();
        notifier.send(&ctx).await.unwrap();

        let result = notifier.send(&ctx).await;
        assert!(matches!(result, Err(NotifyError::RateLimited(_))));
    }
}
