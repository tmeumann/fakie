use crate::filter_outcome::FilterOutcome;

pub trait Filter {
    fn filter(&self, request_hash: u64) -> FilterOutcome;
    fn reset(&self, request_hash: u64);
}
