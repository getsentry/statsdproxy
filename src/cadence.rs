use std::{io, sync::Mutex};

use cadence::MetricSink;

use crate::{middleware::Middleware, types::Metric};

pub struct StatsdProxyMetricSink<M> {
    next: Mutex<M>,
}

impl<M> StatsdProxyMetricSink<M>
where
    M: Middleware,
{
    pub fn new(next: M) -> StatsdProxyMetricSink<M> {
        StatsdProxyMetricSink {
            next: Mutex::new(next),
        }
    }
}

impl<M> MetricSink for StatsdProxyMetricSink<M>
where
    M: Middleware,
{
    // FIXME: There's a bit of an impedance mismatch between Cadence's metric sinks and our middleware interface,
    // so this is not entirely correct:
    //
    // 1) The return value from `emit` on success is supposed to be the number of bytes writen to the sink,
    // or zero if the write was buffered. `Middleware` doesn't have a way to propagate this information up
    // from one middleware to the next.
    //
    // 2) `flush` is supposed to force a flush of all buffered metrics, but there's no way to ask the
    // next middleware to do this.

    fn emit(&self, raw_metric: &str) -> io::Result<usize> {
        let cooked_metric = Metric::new(raw_metric.as_bytes().to_vec());
        let mut next = self.next.lock().unwrap();
        next.poll();
        next.submit(&mut cooked_metric);

        Ok(raw_metric.len())
    }

    fn flush(&self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::RwLock;

    use super::*;
    use crate::testutils::FnStep;
    use cadence::prelude::*;
    use cadence::StatsdClient;

    #[test]
    fn basic() {
        let results = Arc::new(RwLock::new(vec![]));
        let results2 = results.clone();
        let next = FnStep(move |metric| {
            results.write().unwrap().push(metric);
        });

        let sink = StatsdProxyMetricSink::new(next);
        let client = StatsdClient::from_sink("test.metrics", sink);

        client.incr("test.counter").unwrap();
        client.gauge("test.gauge", 42).unwrap();

        assert_eq!(results2.read().unwrap().len(), 2);
    }
}
