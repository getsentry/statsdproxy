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
        let mut tags_to_keep = Vec::new();
        let mut rewrite_tags = false;

        for tag in metric.tags_iter() {
            if self.tags.contains(tag.name) {
                rewrite_tags = true;
            } else {
                tags_to_keep.push(tag);
            }
        }

        if rewrite_tags {
            let mut rewriten_metric = metric.clone();
            let tag_bytes = tags_to_keep.iter().map(|t| t.to_bytes());
            
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
