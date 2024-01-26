use anyhow::Error;
use sentry::metrics::Metric as SentryMetric;

use crate::middleware::Middleware;
use crate::types::Metric;

pub struct Sentry {}

impl Sentry {
    pub fn new() -> Self {
        Sentry {}
    }
}

impl Middleware for Sentry {
    fn join(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn poll(&mut self) {}

    fn submit(&mut self, metric: &mut Metric) {
        let raw = match std::str::from_utf8(&metric.raw) {
            Ok(x) => x,
            Err(e) => {
                log::debug!("metric is not utf8: {:?}", e);
                return;
            }
        };

        let metric = match SentryMetric::parse_statsd(raw) {
            Ok(x) => x,
            Err(e) => {
                log::debug!("sentry cannot parse metric: {:?}", e);
                return;
            }
        };

        metric.send();
    }
}
