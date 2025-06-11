use std::collections::HashSet;
use crate::config::DenyTagConfig;
use crate::middleware::Middleware;
use crate::types::Metric;
use anyhow::Error;

/// A middleware that denies metric tags based on configurable filter rules.
///
/// This middleware allows you to explicitly deny tags from metrics based on predefined
/// filter rules. It's particularly useful when you want to:
/// - Consistently deny specific tags across multiple metric calls
/// - Control metric cardinality by denying high-cardinality tags
/// - Centralize tag denial rules rather than handling them in individual metric calls
///
/// A common use case is managing metric cardinality. For example, you can
/// deny high-cardinality tags (like user IDs) in certain environments while allowing them
/// in others, all without modifying the metric emission code.
pub struct DenyTag<M> {
    filters: HashSet<DenyType>,
    next: M,
}

impl<M> DenyTag<M>
where
    M: Middleware,
{
    pub fn new(config: DenyTagConfig, next: M) -> Self {
        let filters = config.starts_with.into_iter()
            .map(DenyType::StartsWith)
            .chain(config.ends_with.into_iter()
                .map(DenyType::EndsWith))
            .chain(config.tags.into_iter().map(DenyType::Equals))
            .collect();

        Self { filters, next }
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
            if self.filters.iter().any(|f| f.matches(tag.name())) {
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

/// Different types of operations that can be used to strip out a metric tag by name.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum DenyType {
    /// The metric tag starts with the specified string.
    StartsWith(String),
    /// The metric tag ends with the specified string.
    EndsWith(String),
    /// The metric tag matches the word exactly.
    Equals(String),
}

impl DenyType {
    /// Returns `true` if the metric name (in bytes) matches the given filter operation.
    pub fn matches(&self, value: &[u8]) -> bool {
        match self {
            Self::StartsWith(starts_with) => value.starts_with(starts_with.as_bytes()),
            Self::EndsWith(ends_with) => value.ends_with(ends_with.as_bytes()),
            Self::Equals(equals) => equals.as_bytes() == value,
        }
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
            starts_with: vec![],
            ends_with: vec![]
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

    #[test]
    fn test_filter_starts_with() {
        let config = DenyTagConfig {
            tags: vec![],
            starts_with: vec!["hc_".to_owned()],
            ends_with: vec![]
        };
        let results = RefCell::new(Vec::new());
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut filter = DenyTag::new(config, next);
        filter.submit(&mut Metric::new(
            b"foo.bar:1|c|#abc.tag:test,hc_project:1000".to_vec(),
        ));

        assert_eq!(
            results.borrow()[0],
            Metric::new(b"foo.bar:1|c|#abc.tag:test".to_vec())
        );
    }

    #[test]
    fn test_filter_ends_with() {
        let config = DenyTagConfig {
            tags: vec![],
            starts_with: vec![],
            ends_with: vec!["_hc".to_owned()]
        };
        let results = RefCell::new(Vec::new());
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut filter = DenyTag::new(config, next);
        filter.submit(&mut Metric::new(
            b"foo.bar:1|c|#abc.tag:test,project_hc:1000".to_vec(),
        ));

        assert_eq!(
            results.borrow()[0],
            Metric::new(b"foo.bar:1|c|#abc.tag:test".to_vec())
        );
    }

    #[test]
    fn test_multiple_filters() {
        let config = DenyTagConfig {
            tags: vec![],
            starts_with: vec!["hc_".to_owned()],
            ends_with: vec!["_with_ending".to_owned()]
        };
        let results = RefCell::new(Vec::new());
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut filter = DenyTag::new(config, next);
        filter.submit(&mut Metric::new(
            b"foo.bar:1|c|#abc.tag:test,hc_project:1000,metric_with_ending:12".to_vec(),
        ));

        assert_eq!(
            results.borrow()[0],
            Metric::new(b"foo.bar:1|c|#abc.tag:test".to_vec())
        );
    }

    #[test]
    fn test_deduplication() {
        let config = DenyTagConfig {
            tags: vec!["test1".to_owned(), "test1".to_owned()],
            starts_with: vec!["start1".to_owned(), "start1".to_owned()],
            ends_with: vec!["end1".to_owned(), "end1".to_owned()]
        };
        let results = RefCell::new(Vec::new());
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let filter = DenyTag::new(config, next);
        let expected = HashSet::from_iter(vec![
            DenyType::Equals("test1".to_owned()),
            DenyType::StartsWith("start1".to_owned()),
            DenyType::EndsWith("end1".to_owned())].iter().cloned());
        assert_eq!(filter.filters, expected);
    }
}
