use crate::config::DenyTagConfig;
use crate::middleware::Middleware;
use crate::types::Metric;
use anyhow::Error;
use std::collections::HashSet;

pub struct DenyTag<M> {
    tags: HashSet<Vec<u8>>,
    next: M,
}

impl<M> DenyTag<M>
where
    M: Middleware,
{
    pub fn new(config: DenyTagConfig, next: M) -> Self {
        let tags: HashSet<Vec<u8>> =
            HashSet::from_iter(config.tags.iter().cloned().map(|tag| tag.into_bytes()));

        Self { tags, next }
    }
}

impl<M> Middleware for DenyTag<M>
where
    M: Middleware,
{
    fn poll(&mut self) {
        self.next.poll()
    }

    fn submit(&mut self, metric: &mut Metric) {
        let mut tags_to_keep = Vec::new();
        let mut rewrite_tags = false;

        for tag in metric.tags_iter() {
            if self.tags.contains(tag.name()) {
                log::debug!("deny_tag: Dropping tag {:?}", tag.name());
                rewrite_tags = true;
            } else {
                tags_to_keep.push(tag);
            }
        }

        if rewrite_tags {
            let mut rewriten_metric = metric.clone();
            rewriten_metric.set_tags_from_iter(tags_to_keep.into_iter());
            self.next.submit(&mut rewriten_metric)
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
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut tag_denier = DenyTag::new(config, next);

        tag_denier.submit(&mut Metric::new(
            b"servers.online:1|c|#country:china,nope:foo".to_vec(),
        ));
        assert_eq!(
            results.borrow()[0],
            Metric::new(b"servers.online:1|c|#country:china".to_vec())
        );

        tag_denier.submit(&mut Metric::new(
            b"servers.online:1|c|#country:china,nope:foo,extra_stuff,,".to_vec(),
        ));
        assert_eq!(
            results.borrow()[1],
            Metric::new(b"servers.online:1|c|#country:china,extra_stuff,,".to_vec())
        );
    }
}
