#[cfg(test)]
use std::sync::Mutex;

use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use std::{fmt, str};

use crate::{config::AggregateMetricsConfig, middleware::Middleware, types::Metric};

#[derive(Hash, Eq, PartialEq)]
struct BucketKey {
    // contains the raw metric bytes with the value stripped out
    // for example, `users.online:1|c|#country:china` would be stored as:
    //
    //   metric_bytes: users.online:|c|#country:china
    //   insert_value_at: 13
    metric_bytes: Vec<u8>,
    insert_value_at: usize,
}

impl fmt::Debug for BucketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BucketKey")
            .field("metric_bytes", &str::from_utf8(&self.metric_bytes))
            .field("insert_value_at", &self.insert_value_at)
            .finish()
    }
}

#[derive(Debug)]
enum BucketValue {
    Counter(f64),
    Gauge(f64),
}

impl BucketValue {
    fn merge(&mut self, other: &BucketValue) {
        match (self, other) {
            (BucketValue::Gauge(a), BucketValue::Gauge(b)) => *a = *b,
            (BucketValue::Counter(a), BucketValue::Counter(b)) => *a += *b,
            // this codepath should never happen because two different bucket values end up in
            // different hashmap keys
            _ => panic!("attempted to merge two unrelated bucket values together"),
        }
    }
}

pub struct AggregateMetrics<M> {
    config: AggregateMetricsConfig,
    metrics_map: HashMap<BucketKey, BucketValue>,
    last_flushed_at: u64,
    next: M,
}

impl<M> AggregateMetrics<M>
where
    M: Middleware,
{
    pub fn new(config: AggregateMetricsConfig, next: M) -> Self {
        AggregateMetrics {
            config,
            metrics_map: HashMap::new(),
            next,
            last_flushed_at: 0,
        }
    }

    fn insert_metric(&mut self, metric: &Metric) -> Result<(), &'static str> {
        let raw_value = metric
            .value()
            .and_then(|x| str::from_utf8(x).ok())
            .ok_or("failed to parse metric value as utf8")?;
        let value = match metric.ty().ok_or("failed to parse metric type")? {
            b"c" if self.config.aggregate_counters => BucketValue::Counter(
                raw_value
                    .parse()
                    .map_err(|_| "failed to parse counter value")?,
            ),
            b"g" if self.config.aggregate_gauges => BucketValue::Gauge(
                raw_value
                    .parse()
                    .map_err(|_| "failed to parse gauge value")?,
            ),
            _ => return Err("unsupported metric type"),
        };

        let value_start = raw_value.as_ptr() as usize - metric.raw.as_ptr() as usize;
        let value_end = value_start + raw_value.len();
        let mut metric_bucket_bytes = metric.raw[..value_start].to_vec();
        metric_bucket_bytes.extend(&metric.raw[value_end..]);

        let key = BucketKey {
            metric_bytes: metric_bucket_bytes,
            insert_value_at: value_start,
        };

        self.metrics_map
            .entry(key)
            .and_modify(|other_value| other_value.merge(&value))
            .or_insert(value);

        Ok(())
    }

    fn flush_metrics(&mut self) {
        self.next.poll();

        let mut values_iter = self.metrics_map.drain();

        for (key, value) in &mut values_iter {
            let value_bytes = match value {
                BucketValue::Gauge(x) => x.to_string().into_bytes(),
                BucketValue::Counter(x) => x.to_string().into_bytes(),
            };

            let mut metric_bytes = key.metric_bytes[..key.insert_value_at].to_vec();
            metric_bytes.extend(value_bytes);
            metric_bytes.extend(&key.metric_bytes[key.insert_value_at..]);

            self.next.submit(&mut Metric::new(metric_bytes));
        }
    }
}

#[cfg(test)]
static CURRENT_TIME: Mutex<Option<u64>> = Mutex::new(None);

impl<M> Middleware for AggregateMetrics<M>
where
    M: Middleware,
{
    fn poll(&mut self) {
        #[cfg(test)]
        let overwrite_now = *CURRENT_TIME.lock().unwrap();
        #[cfg(not(test))]
        let overwrite_now = None;

        #[allow(clippy::unnecessary_literal_unwrap)]
        let now = overwrite_now.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        let rounded_bucket =
            i64::try_from((now / self.config.flush_interval) * self.config.flush_interval)
                .expect("overflow when calculating with flush_interval");
        let rounded_bucket = u64::try_from(rounded_bucket + self.config.flush_offset)
            .expect("overflow when calculating with flush_interval");

        if self.last_flushed_at + self.config.flush_interval <= rounded_bucket {
            self.flush_metrics();
            self.last_flushed_at = rounded_bucket;
        }

        self.next.poll()
    }

    fn submit(&mut self, metric: &mut Metric) {
        match self.insert_metric(metric) {
            Ok(()) => {}
            Err(_) => {
                // for now discard the parsing error, we might want to add info logging here
                self.next.submit(metric);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    use crate::testutils::FnStep;

    #[test]
    fn basic() {
        let config = AggregateMetricsConfig {
            aggregate_counters: true,
            aggregate_gauges: true,
            flush_interval: 10,
            flush_offset: 0,
            max_map_size: None,
        };
        let results = RefCell::new(vec![]);
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut aggregator = AggregateMetrics::new(config, next);

        *CURRENT_TIME.lock().unwrap() = Some(0);

        aggregator.poll();

        aggregator.submit(&mut Metric::new(
            b"users.online:1|c|@0.5|#country:china".to_vec(),
        ));

        *CURRENT_TIME.lock().unwrap() = Some(1);

        aggregator.poll();

        aggregator.submit(&mut Metric::new(
            b"users.online:1|c|@0.5|#country:china".to_vec(),
        ));

        assert_eq!(results.borrow_mut().len(), 0);

        *CURRENT_TIME.lock().unwrap() = Some(11);

        aggregator.poll();

        assert_eq!(
            results.borrow_mut().as_slice(),
            &[Metric::new(
                b"users.online:2|c|@0.5|#country:china".to_vec()
            )]
        );
    }

    #[test]
    fn gauges() {
        let config = AggregateMetricsConfig {
            aggregate_counters: true,
            aggregate_gauges: true,
            flush_interval: 10,
            flush_offset: 0,
            max_map_size: None,
        };
        let results = RefCell::new(vec![]);
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut aggregator = AggregateMetrics::new(config, next);

        *CURRENT_TIME.lock().unwrap() = Some(0);

        aggregator.poll();

        aggregator.submit(&mut Metric::new(
            b"users.online:3|g|@0.5|#country:china".to_vec(),
        ));

        *CURRENT_TIME.lock().unwrap() = Some(1);

        aggregator.poll();

        aggregator.submit(&mut Metric::new(
            b"users.online:2|g|@0.5|#country:china".to_vec(),
        ));

        assert_eq!(results.borrow_mut().len(), 0);

        *CURRENT_TIME.lock().unwrap() = Some(11);

        aggregator.poll();

        assert_eq!(
            results.borrow_mut().as_slice(),
            &[Metric::new(
                b"users.online:2|g|@0.5|#country:china".to_vec()
            )]
        );
    }
}
