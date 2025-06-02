use crate::filter::Filter;
use crate::filter_outcome::FilterOutcome;
use dashmap::DashMap;

pub struct CountingFilter {
    map: DashMap<u64, u64>,
    max: u64,
    failure_mode: FilterOutcome,
}

impl CountingFilter {
    pub fn new(max: u64, failure_mode: FilterOutcome) -> Self {
        Self {
            map: DashMap::new(),
            max,
            failure_mode,
        }
    }
}

impl Filter for CountingFilter {
    fn filter(&self, request_hash: u64) -> FilterOutcome {
        let mut count = self.map.entry(request_hash).or_insert(0);

        if *count >= self.max {
            FilterOutcome::Passed
        } else {
            *count += 1;
            self.failure_mode
        }
    }

    fn reset(&self, request_hash: u64) {
        self.map
            .entry(request_hash)
            .and_modify(|v| *v = 0)
            .or_insert(0);
    }
}
