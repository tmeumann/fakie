use crate::filter::Filter;
use crate::filter_outcome::FilterOutcome;
use pingora::{Custom, Error, Result};
use rand::distr::Bernoulli;
use rand::prelude::Distribution;
use rand::{rng, RngExt};

pub struct ChaosFilter {
    drop_distribution: Bernoulli,
}

impl ChaosFilter {
    pub fn new(drop_rate: f64) -> Result<Self> {
        let drop_distribution =
            Bernoulli::new(drop_rate).map_err(|_| Error::new(Custom("Invalid chaos drop rate")))?;
        Ok(Self { drop_distribution })
    }
}

impl Filter for ChaosFilter {
    fn filter(&self, _request_hash: u64) -> FilterOutcome {
        if !self.drop_distribution.sample(&mut rng()) {
            // it's 50:50 whether the chaos filter drops the request or the response
            if rng().random() {
                FilterOutcome::RequestDenied
            } else {
                FilterOutcome::ResponseDenied
            }
        } else {
            FilterOutcome::Passed
        }
    }

    fn reset(&self, _request_hash: u64) {}
}
