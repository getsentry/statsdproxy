use crate::config::DenyTagConfig;
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;
use std::collections::HashSet;

pub struct DenyTag {
    #[allow(dead_code)]
    tags: HashSet<Vec<u8>>,
    next: Box<dyn Middleware>,
}

impl DenyTag {
    pub fn new(config: DenyTagConfig, next: Box<dyn Middleware>) -> Self {
        let tags: HashSet<Vec<u8>> =
            HashSet::from_iter(config.tags.iter().cloned().map(|tag| tag.into_bytes()));
        Self { next, tags }
    }
}

impl Middleware for DenyTag {
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
