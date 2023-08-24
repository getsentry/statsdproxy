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
        let mut rewritten_metric = metric.clone();

        rewritten_metric.set_tags_from_iter(metric.tags_iter().filter(|tag| {
            let tag_name = tag.name();

            if let Some(tag_value) = tag.value() {
                for quota in self.quotas.iter() {
                    // Drop the tag if it does not fit in quota
                    if (quota.tag == "*" || quota.tag.as_bytes() == tag_name)
                        && (quota.values_seen.len() >= quota.limit as usize
                            && !quota.values_seen.contains(tag_value))
                    {
                        return false;
                    }
                }
            }

            // Tag fits in quota, or has no value -- keep it
            true
        }));

        self.next.submit(rewritten_metric.clone())?;

        // Increment quotas
        for tag in rewritten_metric.tags_iter() {
            for quota in self.quotas.iter_mut() {
                if quota.tag == "*" || quota.tag.as_bytes() == tag.name().to_vec() {
                    if let Some(tag_value) = tag.value() {
                        quota.values_seen.insert(tag_value.to_vec());
                    }
                }
            }
        }

        Ok(())
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
    fn tag_cardinality_limit() {
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
        assert_eq!(
            results.borrow()[0],
            Metric::new(b"users.online:1|c|#env:prod".to_vec())
        );
        limiter
            .submit(Metric::new(b"users.online:1|c|#env:dev".to_vec()))
            .unwrap();
        // env was stripped from metric
        assert_eq!(
            results.borrow()[1],
            Metric::new(b"users.online:1|c".to_vec())
        );

        limiter
            .submit(Metric::new(b"users.online:1|c|#env".to_vec()))
            .unwrap();
        // Tag without value is not limited
        assert_eq!(
            results.borrow()[2],
            Metric::new(b"users.online:1|c|#env".to_vec())
        );
    }
}
