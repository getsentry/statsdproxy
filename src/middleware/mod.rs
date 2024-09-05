use anyhow::Error;

use crate::types::Metric;

pub mod add_tag;
pub mod aggregate;
pub mod allow_tag;
pub mod cardinality_limit;
pub mod deny_tag;
pub mod mirror;
pub mod sample;
pub mod tag_cardinality_limit;
pub mod upstream;

#[cfg(feature = "cli")]
pub mod server;

impl Middleware for Box<dyn Middleware> {
    fn join(&mut self) -> Result<(), Error> {
        self.as_mut().join()
    }
    fn poll(&mut self) {
        self.as_mut().poll()
    }
    fn submit(&mut self, metric: &mut Metric) {
        self.as_mut().submit(metric)
    }
}

pub trait Middleware {
    fn join(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn poll(&mut self) {}
    fn submit(&mut self, metric: &mut Metric);
}
