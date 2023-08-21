use crate::config::AllowTagConfig;
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;
use std::collections::HashSet;

pub struct AllowTag {
    #[allow(dead_code)]
    tags: HashSet<Vec<u8>>,
    next: Box<dyn Middleware>,
}

impl AllowTag {
    pub fn new(config: AllowTagConfig, next: Box<dyn Middleware>) -> Self {
        let tags: HashSet<Vec<u8>> =
            HashSet::from_iter(config.tags.iter().cloned().map(|tag| tag.into_bytes()));

        Self { tags, next }
    }
}

impl Middleware for AllowTag {
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
