use crate::{
    middleware::{Middleware, Overloaded},
    types::Metric,
};

pub struct FnStep<F>(pub F);

impl<F> Middleware for FnStep<F>
where
    F: FnMut(Metric) -> Result<(), Overloaded>,
{
    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        (self.0)(metric)
    }
}
