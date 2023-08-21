use crate::config::{CardinalityLimitConfig, LimitConfig};
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;
use crc32fast::Hasher;
use std::collections::HashMap;
use std::convert::From;
use std::time::{SystemTime, UNIX_EPOCH};

// Vaguely modelled after https://github.com/getsentry/sentry-redis-tools/blob/main/sentry_redis_tools/cardinality_limiter.py
// but without sliding window functionality (or redis)

struct Quota {
    window: u64,
    limit: u64,
    // granularity: u64,
}

impl From<LimitConfig> for Quota {
    fn from(config: LimitConfig) -> Self {
        // let granularity = match config.window {
        //     0..=300 => 1,
        //     301..=1440 => 60,
        //     _ => 3600,
        // };

        Quota {
            window: config.window as u64,
            limit: config.limit,
        }
    }
}

pub struct CardinalityLimit {
    quotas: Vec<Quota>,
    quota_usage: HashMap<String, u64>,
    next: Box<dyn Middleware>,
}

impl CardinalityLimit {
    pub fn new(config: CardinalityLimitConfig, next: Box<dyn Middleware>) -> Self {
        let quotas = config.limits.into_iter().map(Quota::from).collect();
        Self {
            quotas,
            next,
            quota_usage: HashMap::new(),
        }
    }

    fn hash_metric(&self, metric: &Metric) -> u32 {
        let mut hasher = Hasher::new();
        if let Some(name) = metric.name() {
            hasher.update(name);
        }
        if let Some(tags) = metric.tags() {
            hasher.update(tags);
        }
        hasher.finalize()
    }
}

impl Middleware for CardinalityLimit {
    fn poll(&mut self) -> Result<(), Error> {
        self.next.poll()
    }

    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        // TODO: fix this
        let metric_hash = self.hash_metric(&metric);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        for quota in &self.quotas {
            let window_start = now / quota.window as u64;

            let key = format!(
                "{}:{}:{}:{}",
                metric_hash, quota.window, quota.limit, window_start
            );
            self.quota_usage.insert(key, 666);
        }

        self.next.submit(metric)
    }

    fn join(&mut self) -> Result<(), Error> {
        self.next.join()
    }
}
