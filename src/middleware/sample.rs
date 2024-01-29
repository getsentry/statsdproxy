use anyhow::Error;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::config::SampleConfig;
use crate::middleware::Middleware;
use crate::types::Metric;

pub struct Sample<M> {
    next: M,
    rng: SmallRng,
    config: SampleConfig,
}

impl<M> Sample<M> {
    pub fn new(config: SampleConfig, next: M) -> Self {
        let rng = SmallRng::from_entropy();
        Sample { next, config, rng }
    }
}

impl<M> Middleware for Sample<M>
where
    M: Middleware,
{
    fn join(&mut self) -> Result<(), Error> {
        self.next.join()?;
        Ok(())
    }

    fn poll(&mut self) {
        self.next.poll();
    }

    fn submit(&mut self, metric: &mut Metric) {
        if self.config.sample_rate == 0.0 {
            return;
        }

        let decision: f64 = self.rng.gen();
        if decision < self.config.sample_rate {
            self.next.submit(metric);
        }
    }
}
