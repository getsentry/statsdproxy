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

        Self { tags, next }
    }
}

impl Middleware for DenyTag {
    fn poll(&mut self) -> Result<(), Error> {
        self.next.poll()
    }

    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        let mut tags_to_keep = Vec::new();
        let mut rewrite_tags = false;

        for tag in metric.tags_iter() {
            if tag.name().is_some_and(|t| self.tags.contains(t)) {
                rewrite_tags = true;
            } else {
                tags_to_keep.push(tag);
            }
        }

        if rewrite_tags {
            let mut rewriten_metric = metric.clone();
            let tag_bytes = tags_to_keep.iter().map(|t| t.raw);
            
            let mut tag_buffer = Vec::new();
            for t in tag_bytes {
                tag_buffer.extend(t);
                
                tag_buffer.push(b',');
            }
            rewriten_metric.set_tags(&tag_buffer[0..tag_buffer.len() - 1]); // omit trailing ',' from loop above

            self.next.submit(rewriten_metric)
        } else {
            self.next.submit(metric)
        }
    }

    fn join(&mut self) -> Result<(), Error> {
        self.next.join()
    }
}
