use anyhow::Error;
use crate::middleware::Middleware;
use crate::types::Metric;

/// Different types of operations that can be used to filter out a metric by name.
pub enum FilterType {
    /// The metric starts with the specified string.
    StartsWith(String),
    /// The metric ends with the specified string.
    EndsWith(String)
}

impl FilterType {
    /// Returns `true` if the metric name (in bytes) matches the given filter operation.
    pub fn matches(&self, value: &[u8]) -> bool {
        match self {
            Self::StartsWith(starts_with) => value.starts_with(starts_with.as_bytes()),
            Self::EndsWith(ends_with) => value.ends_with(ends_with.as_bytes())
        }
    }
}

/// A middleware that filters metric tags based on configurable filter rules.
///
/// This middleware allows you to selectively filter out tags from metrics based on predefined
/// filter rules. It's particularly useful when you want to:
/// - Apply consistent tag filtering across multiple metric calls
/// - Manage metric cardinality by filtering out certain tags
/// - Configure tag filtering at a central location rather than in individual metric calls
///
/// This middleware is particularly useful for managing metric cardinality. For example, you can
/// filter out high-cardinality tags (like user IDs) in certain environments while keeping them
/// in others, all without modifying the metric emission code.
pub struct FilterTag<M> {
    /// A list of filter rules that determine which tags should be filtered out.
    filters: Vec<FilterType>,
    /// The next middleware in the chain.
    next: M
}

impl<M> FilterTag<M> where M:Middleware {
    pub fn new(filters: Vec<FilterType>, next: M) -> FilterTag<M> {
        Self {
            filters, next
        }
    }
}

impl<M> Middleware for FilterTag<M> where M:Middleware {
    fn join(&mut self) -> Result<(), Error> {
        self.next.join()
    }
    fn poll(&mut self) {
        self.next.poll()
    }

    fn submit(&mut self, metric: &mut Metric) {
        let has_filtered_tags = metric
            .tags_iter()
            .any(|t| self.filters.iter().any(|filters| filters.matches(t.name())));

        if has_filtered_tags {
            let mut new_metric = metric.clone();
            new_metric.set_tags_from_iter(
                metric
                    .tags_iter()
                    .filter(|t| !self.filters.iter().any(|filters| filters.matches(t.name()))),
            );
            self.next.submit(&mut new_metric);
        } else {
            self.next.submit(metric);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use crate::middleware::filter_tag::{FilterTag, FilterType};
    use crate::middleware::Middleware;
    use crate::testutils::FnStep;
    use crate::types::Metric;

    #[test]
    fn test_filter_starts_with() {
        let results = RefCell::new(Vec::new());
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut filter = FilterTag::new(vec![FilterType::StartsWith("hc_".to_owned())], next);
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
        let results = RefCell::new(Vec::new());
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut filter = FilterTag::new(vec![FilterType::EndsWith("_hc".to_owned())], next);
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
        let results = RefCell::new(Vec::new());
        let next = FnStep(|metric: &mut Metric| {
            results.borrow_mut().push(metric.clone());
        });
        let mut filter = FilterTag::new(vec![
            FilterType::StartsWith("hc_".to_owned()),
            FilterType::EndsWith("_with_ending".to_owned())
        ], next);
        filter.submit(&mut Metric::new(
            b"foo.bar:1|c|#abc.tag:test,hc_project:1000,metric_with_ending:12".to_vec(),
        ));

        assert_eq!(
            results.borrow()[0],
            Metric::new(b"foo.bar:1|c|#abc.tag:test".to_vec())
        );
    }
}