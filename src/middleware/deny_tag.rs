use crate::config::DenyTagConfig;
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;
use std::collections::HashSet;


pub struct DenyTag<M> {
    #[allow(dead_code)]
    tags: HashSet<Vec<u8>>,
    next: M,
}

impl<M> DenyTag<M>
where
    M: Middleware
{
    pub fn new(config: DenyTagConfig, next: M) -> Self {
        let tags: HashSet<Vec<u8>> =
            HashSet::from_iter(config.tags.iter().cloned().map(|tag| tag.into_bytes()));

        Self { tags, next }
    }
}

impl<M> Middleware for DenyTag<M>
where
    M: Middleware
{
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
            rewriten_metric.set_tags_from_iter(tags_to_keep.iter());
            self.next.submit(rewriten_metric)
        } else {
            self.next.submit(metric)
        }
    }

    fn join(&mut self) -> Result<(), Error> {
        self.next.join()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::testutils::FnStep;

    #[test]
    fn basic() {
        let config = DenyTagConfig {
            tags: vec!["nope".to_string()],
        };

        let results = RefCell::new(vec![]);
        let next = FnStep(|metric| {
            results.borrow_mut().push(metric);
            Ok(())
        });
        let mut tag_denier = DenyTag::new(config, next);

        tag_denier
            .submit(Metric::new(b"servers.online:1|c|#country:china,nope:foo".to_vec()))
            .unwrap();
        assert_eq!(results.borrow()[0], Metric::new(b"servers.online:1|c|#country:china".to_vec()));

        tag_denier
            .submit(Metric::new(b"servers.online:1|c|#country:china,nope:foo,extra_stuff,,".to_vec()))
            .unwrap();
        assert_eq!(results.borrow()[1], Metric::new(b"servers.online:1|c|#country:china,extra_stuff,,".to_vec()));
    }
}