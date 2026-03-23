use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::traits::NotifyError;

pub(crate) struct RateLimiter {
    min_interval: Duration,
    last_sent: Mutex<HashMap<String, Instant>>,
}

impl RateLimiter {
    pub(crate) fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last_sent: Mutex::new(HashMap::new()),
        }
    }

    /// Check whether a notification with the given key is allowed.
    /// Records the current time on success so subsequent calls within
    /// `min_interval` will be rejected.
    pub(crate) async fn check(&self, key: &str) -> Result<(), NotifyError> {
        let mut map = self.last_sent.lock().await;
        let now = Instant::now();

        if let Some(last) = map.get(key) {
            let elapsed = now.duration_since(*last);
            if elapsed < self.min_interval {
                let remaining = self.min_interval - elapsed;
                return Err(NotifyError::RateLimited(remaining.as_secs()));
            }
        }

        map.insert(key.to_string(), now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_first_call() {
        let limiter = RateLimiter::new(Duration::from_secs(60));
        assert!(limiter.check("key1").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_rapid_calls() {
        let limiter = RateLimiter::new(Duration::from_secs(60));
        limiter.check("key1").await.unwrap();

        let result = limiter.check("key1").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NotifyError::RateLimited(_)));
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_after_interval() {
        let limiter = RateLimiter::new(Duration::from_millis(10));
        limiter.check("key1").await.unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;

        assert!(limiter.check("key1").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_independent_keys() {
        let limiter = RateLimiter::new(Duration::from_secs(60));
        limiter.check("key1").await.unwrap();

        assert!(limiter.check("key2").await.is_ok());
    }
}
