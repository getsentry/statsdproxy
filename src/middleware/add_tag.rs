use crate::config::AddTagConfig;
use crate::middleware::Middleware;
use crate::types::Metric;
use anyhow::Error;

pub struct AddTag<M> {
    tags: Vec<u8>,
    next: M,
}

impl<M> AddTag<M>
where
    M: Middleware,
{
    pub fn new(config: AddTagConfig, next: M) -> Self {
        let tags = config.tags.join(",").into_bytes();
        Self { tags, next }
    }
}

impl<M> Middleware for AddTag<M>
where
    M: Middleware,
{
    fn poll(&mut self) {
        self.next.poll()
    }

    fn submit(&mut self, metric: &mut Metric) {
        match metric.tags() {
            Some(tags) => {
                let mut tag_buffer: Vec<u8> = Vec::new();
                tag_buffer.extend(tags);
                tag_buffer.extend(",".as_bytes());
                tag_buffer.extend(&self.tags);
                metric.set_tags(&tag_buffer);
            }
            None => {
                metric.set_tags(&self.tags);
            }
        }

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
        let test_cases = [
            // Without tags
            ("users.online:1|c", "users.online:1|c|#env:prod"),
            // With tags
            (
                "users.online:1|c|#tag1:a",
                "users.online:1|c|#tag1:a,env:prod",
            ),
        ];

        for test_case in test_cases {
            let config = AddTagConfig {
                tags: vec!["env:prod".to_string()],
            };
            let results = RefCell::new(vec![]);
            let next = FnStep(|metric: &mut Metric| {
                results.borrow_mut().push(metric.clone());
            });

            let mut middleware = AddTag::new(config, next);
            let mut metric = Metric::new(test_case.0.as_bytes().to_vec());
            middleware.submit(&mut metric);
            assert_eq!(results.borrow().len(), 1);
            let updated_metric = Metric::new(results.borrow_mut()[0].raw.clone());
            assert_eq!(updated_metric.raw, test_case.1.as_bytes());
        }
    }
}
