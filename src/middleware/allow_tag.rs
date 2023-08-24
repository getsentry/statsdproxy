use crate::config::AllowTagConfig;
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;
use std::collections::HashSet;

pub struct AllowTag<M> {
    tags: HashSet<Vec<u8>>,
    next: M,
}

impl<M> AllowTag<M>
where
    M: Middleware,
{
    pub fn new(config: AllowTagConfig, next: M) -> Self {
        let tags: HashSet<Vec<u8>> =
            HashSet::from_iter(config.tags.iter().cloned().map(|tag| tag.into_bytes()));

        Self { tags, next }
    }
}

impl<M> Middleware for AllowTag<M>
where
    M: Middleware,
{
    fn poll(&mut self) -> Result<(), Overloaded> {
        self.next.poll()
    }

    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        let mut tags_to_keep = Vec::new();
        let mut rewrite_tags = false;
        for tag in metric.tags_iter() {
            if self.tags.contains(tag.name()) {
                tags_to_keep.push(tag);
            } else {
                rewrite_tags = true;
            }
        }

        if rewrite_tags {
            let mut rewriten_metric = metric.clone();
            rewriten_metric.set_tags_from_iter(tags_to_keep.into_iter());
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
        let config = AllowTagConfig {
            tags: vec!["country".to_string(), "arch".to_string()],
        };

        let results = RefCell::new(vec![]);
        let next = FnStep(|metric| {
            results.borrow_mut().push(metric);
            Ok(())
        });
        let mut tag_allower = AllowTag::new(config, next);

        tag_allower
            .submit(Metric::new(
                b"servers.online:1|c|#country:china,arch:arm64".to_vec(),
            ))
            .unwrap();
        assert_eq!(
            results.borrow()[0],
            Metric::new(b"servers.online:1|c|#country:china,arch:arm64".to_vec())
        );

        tag_allower
            .submit(Metric::new(b"servers.online:1|c|#machine_type:large,country:china,zone:a,arch:arm64,region:east".to_vec()))
            .unwrap();
        assert_eq!(
            results.borrow()[1],
            Metric::new(b"servers.online:1|c|#country:china,arch:arm64".to_vec())
        );
    }
}
