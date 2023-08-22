use crate::config::AddTagConfig;
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;
use std::collections::HashSet;

pub struct AddTag<M> {
    #[allow(dead_code)]
    tags: HashSet<Vec<u8>>,
    next: M,
}

impl<M> AddTag<M>
where
    M: Middleware,
{
    pub fn new(config: AddTagConfig, next: M) -> Self {
        let tags: HashSet<Vec<u8>> =
            HashSet::from_iter(config.tags.iter().cloned().map(|tag| tag.into_bytes()));

        Self { tags, next }
    }
}

impl<M> Middleware for AddTag<M>
where
    M: Middleware,
{
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::FnStep;
    use std::cell::RefCell;

    #[test]
    fn add_tag() {
        let config = AddTagConfig {
            tags: vec!["env:prod".to_string()],
        };
        let results = RefCell::new(vec![]);
        let next = FnStep(|metric| {
            results.borrow_mut().push(metric);
            Ok(())
        });

        let mut middleware = AddTag::new(config, next);

        let metric_without_tags = Metric::new(b"users.online:1|c".to_vec());

        middleware.submit(metric_without_tags).unwrap();

        let updated_metric = Metric::new(results.borrow_mut()[0].raw.clone());

        assert_eq!(updated_metric.tags(), Some(b"env:prod" as &[u8]));
    }
}
