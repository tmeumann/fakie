pub trait Filter {
    fn filter(&self, request_hash: u64) -> bool;
    fn reset(&self, request_hash: u64);
}
