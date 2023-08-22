use crate::config::{CardinalityLimitConfig, LimitConfig};
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;
use crc32fast::Hasher;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::From;
use std::time::{SystemTime, UNIX_EPOCH};

// Vaguely modelled after https://github.com/getsentry/sentry-redis-tools/blob/main/sentry_redis_tools/cardinality_limiter.py
// but without redis

struct Quota {
    /// The time window for which the limit applies. "We accept only 3 distinct metrics per hour"
    /// means the limit is 3, and our window is 3600.
    window: u64,
    /// The number of distinct hashes we want to accept per the timewindow `window`.
    limit: usize,
    /// The timewindow `window` is always relative to the current timestamp, i.e. it "slides", so
    /// that for example an hourly window does not reset the entire limit every hour. This would
    /// create unpleasant "step" effects where at the beginning of each hour, there is a burst of
    /// traffic being accepted (assuming your unfiltered traffic is generally above limits).
    ///
    /// `granularity` is a divisor of `window` that can be used to make those steps smaller. If
    /// granularity equals window equals e.g. 1 hour, there is a burst of traffic at the beginning
    /// of every hour. If granularity equals 1 minute instead, the bursts are much smaller and
    /// happen every minute instead.
    ///
    /// `granularity / window` influences memory usage linearly.
    granularity: u64,

    // a granule is a segment of the window. for example, for a 24-hour window with 1h granularity,
    // we store 24 granules:
    //
    // | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24
    //
    //
    // as we add metrics, they are added to _all_ granules. every hour, the oldest granule gets
    // deleted by `remove_old_keys`, and the second-oldest granule takes its place.
    //
    // this means that the oldest granule contains all hashes observed within the sliding window.
    //
    // a hash has to "fit" into the oldest granule to be accepted. each granule contains a set of
    // hashes, and if the hash is already in the set, it is accepted "for free". if the set's size
    // is `limit`, the hash is rejected.
    //
    // currently all of this is stored as a map of sets. the map key is an absolute timestamp
    // rounded down to the next multiple of `granularity`. the set contains hashes.
    //
    // the outer map could be a ring buffer, then we can reuse the inner BTreeSet and save
    // allocations. even cooler would be to reduce pointer chasing... somehow.
    usage: BTreeMap<u64, BTreeSet<u32>>,
}

impl Quota {
    fn remove_old_keys(&mut self, now: u64) {
        let window_start = now - self.window;

        while let Some(entry) = self.usage.first_entry() {
            if *entry.key() >= window_start {
                break;
            }

            entry.remove_entry();
        }
    }
    fn does_metric_fit(&self, now: u64, hash: u32) -> bool {
        let window_start = now - self.window;
        match self.usage.get(&window_start) {
            Some(oldest_granule) => {
                oldest_granule.len() < self.limit || oldest_granule.contains(&hash)
            }
            None => true,
        }
    }

    fn insert_metric(&mut self, now: u64, hash: u32) {
        let mut current_granule = now - self.window;

        while current_granule < now {
            self.usage.entry(current_granule).or_default().insert(hash);
            current_granule += self.granularity;
        }
    }
}

impl From<LimitConfig> for Quota {
    fn from(config: LimitConfig) -> Self {
        let granularity = match config.window {
            // 5 minutes -> second granularity
            0..=300 => 1,

            // 30 minutes -> minute granularity
            301..=1800 => 60,

            // anything else -> hourly granularity
            _ => 3600,
        };

        Quota {
            window: config.window.into(),
            limit: config
                .limit
                .try_into()
                .expect("quota limit does not fit into native integer (usize)"),
            granularity,
            usage: BTreeMap::new(),
        }
    }
}

pub struct CardinalityLimit<M> {
    quotas: Vec<Quota>,
    next: M,
}

impl<M> CardinalityLimit<M>
where
    M: Middleware,
{
    pub fn new(config: CardinalityLimitConfig, next: M) -> Self {
        let quotas = config.limits.into_iter().map(Quota::from).collect();
        Self { quotas, next }
    }

    fn hash_metric(&self, metric: &Metric) -> u32 {
        let mut hasher = Hasher::new();
        if let Some(name) = metric.name() {
            println!("hashing name: {name:?}");
            hasher.update(name);
        }
        if let Some(tags) = metric.tags() {
            println!("hashing tags: {tags:?}");
            hasher.update(tags);
        }
        hasher.finalize()
    }
}

impl<M> Middleware for CardinalityLimit<M>
where
    M: Middleware,
{
    fn poll(&mut self) -> Result<(), Error> {
        self.next.poll()
    }

    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        let metric_hash = self.hash_metric(&metric);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for quota in &mut self.quotas {
            quota.remove_old_keys(now);

            if !quota.does_metric_fit(now, metric_hash) {
                return Ok(());
            }
        }

        self.next.submit(metric)?;

        // If upstream submission of the metric fails with Overloaded, we don't want to count it
        // against the limit.
        for quota in &mut self.quotas {
            quota.insert_metric(now, metric_hash);
        }

        Ok(())
    }

    fn join(&mut self) -> Result<(), Error> {
        self.next.join()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::testutils::FnStep;

    #[test]
    fn basic() {
        let config = CardinalityLimitConfig {
            limits: vec![LimitConfig {
                limit: 2,
                window: 3600,
            }],
        };

        let results = RefCell::new(vec![]);
        let next = FnStep(|metric| {
            results.borrow_mut().push(metric);
            Ok(())
        });
        let mut limiter = CardinalityLimit::new(config, next);

        limiter
            .submit(Metric::new(b"users.online:1|c|#country:china".to_vec()))
            .unwrap();
        assert_eq!(results.borrow_mut().len(), 1);

        limiter
            .submit(Metric::new(b"servers.online:1|c|#country:china".to_vec()))
            .unwrap();
        assert_eq!(results.borrow_mut().len(), 2);

        // we have already ingested two distinct timeseries, this one should be dropped.
        limiter
            .submit(Metric::new(b"servers.online:1|c|#country:japan".to_vec()))
            .unwrap();
        assert_eq!(results.borrow_mut().len(), 2);

        // A metric with the same hash as an old one within `window` should pass through.
        limiter
            .submit(Metric::new(b"users.online:1|c|#country:china".to_vec()))
            .unwrap();
        assert_eq!(results.borrow_mut().len(), 3);
    }
}
