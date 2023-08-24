use crate::{middleware::Middleware, types::Metric};

pub struct FnStep<F>(pub F);

impl<F> Middleware for FnStep<F>
where
    F: FnMut(&mut Metric),
{
    fn submit(&mut self, metric: &mut Metric) {
        (self.0)(metric)
    }
}
