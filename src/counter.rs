use crate::filter::Filter;
use dashmap::DashMap;

pub struct CountingFilter {
    map: DashMap<u64, u64>,
    max: u64,
}

impl CountingFilter {
    pub fn new(max: u64) -> Self {
        Self {
            map: DashMap::new(),
            max,
        }
    }
}

impl Filter for CountingFilter {
    fn filter(&self, request_hash: u64) -> bool {
        let mut count = self.map.entry(request_hash).or_insert(0);

        let is_allowed = *count >= self.max;

        if !is_allowed {
            *count += 1;
        }

        is_allowed
    }

    fn reset(&self, request_hash: u64) {
        self.map
            .entry(request_hash)
            .and_modify(|v| *v = 0)
            .or_insert(0);
    }
}
