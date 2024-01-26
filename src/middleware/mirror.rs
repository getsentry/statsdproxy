use anyhow::Error;

use crate::middleware::Middleware;
use crate::types::Metric;

pub struct Mirror<M, M2> {
    next: M,
    next2: M2,
}

impl<M, M2> Mirror<M, M2> {
    pub fn new(next: M, next2: M2) -> Self {
        Mirror { next, next2 }
    }
}

impl<M, M2> Middleware for Mirror<M, M2>
where
    M: Middleware,
    M2: Middleware,
{
    fn join(&mut self) -> Result<(), Error> {
        self.next.join()?;
        self.next2.join()?;
        Ok(())
    }

    fn poll(&mut self) {
        self.next.poll();
        self.next2.poll();
    }

    fn submit(&mut self, metric: &mut Metric) {
        self.next.submit(metric);
        // XXX: if next modifies the metric, it will be noticeable in next2
        self.next2.submit(metric);
    }
}
