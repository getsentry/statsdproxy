use crate::config::CardinalityLimitConfig;
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;

pub struct CardinalityLimit {
    next: Box<dyn Middleware>,
}

impl CardinalityLimit {
    pub fn new(_config: CardinalityLimitConfig, next: Box<dyn Middleware>) -> Self {
        Self { next }
    }
}

impl Middleware for CardinalityLimit {
    fn poll(&mut self) -> Result<(), Error> {
        self.next.poll()
    }

    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        self.next.submit(metric)
    }

    fn join(&mut self) -> Result<(), Error> {
        self.next.join()
    }
}
