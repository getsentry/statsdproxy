use crate::config::{TagCardinalityLimitConfig, TagLimitConfig};
use crate::middleware::{Middleware, Overloaded};
use crate::types::Metric;
use anyhow::Error;
use std::collections::HashSet;

#[derive(Clone, Debug)]
struct Quota {
    // Currently this supports wildcard (*) or exact match on tag key
    tag: String,
    limit: u64,
    values_seen: HashSet<Vec<u8>>,
}

impl From<TagLimitConfig> for Quota {
    fn from(config: TagLimitConfig) -> Self {
        Quota {
            tag: config.tag,
            limit: config.limit,
            values_seen: HashSet::new(),
        }
    }
}

pub struct TagCardinalityLimit<M> {
    next: M,
    quotas: Vec<Quota>,
}

impl<M> TagCardinalityLimit<M>
where
    M: Middleware,
{
    #[allow(dead_code)]
    pub fn new(config: TagCardinalityLimitConfig, next: M) -> Self {
        Self {
            next,
            quotas: config.limits.into_iter().map(Quota::from).collect(),
        }
    }
}

impl<M> Middleware for TagCardinalityLimit<M>
where
    M: Middleware,
{
    fn poll(&mut self) -> Result<(), Overloaded> {
        self.next.poll()
    }

    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        for tag in metric.tags_iter() {
            let tag_key = tag.raw.split(|&b| b == b':').next().unwrap();
            // Check all quotas
            for quota in self.quotas.iter() {
                // Check if quota is relevant
                if (quota.tag == "*" || quota.tag.as_bytes() == tag_key)
                    && quota.values_seen.len() >= quota.limit as usize
                    && !quota.values_seen.contains(tag.raw)
                {
                    println!("dropping");
                    // Drop
                }
            }
        }

        let quota = self.quotas[0].clone();
        println!(
            "quotas: {} {} {:?}",
            quota.tag, quota.limit, quota.values_seen
        );

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
    fn cardinality_limit() {
        let config = TagCardinalityLimitConfig {
            limits: vec![TagLimitConfig {
                tag: "env".to_string(),
                limit: 1,
            }],
        };
        let results = RefCell::new(vec![]);
        let next = FnStep(|metric| {
            results.borrow_mut().push(metric);
            Ok(())
        });

        let mut limiter = TagCardinalityLimit::new(config, next);
        limiter
            .submit(Metric::new(b"users.online:1|c|#env:prod".to_vec()))
            .unwrap();
        assert_eq!(results.borrow_mut().len(), 1);
        limiter
            .submit(Metric::new(b"users.online:1|c|#env:dev".to_vec()))
            .unwrap();
        assert_eq!(results.borrow_mut().len(), 1);
    }
}
