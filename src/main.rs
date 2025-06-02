#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]
#![warn(unsafe_code)]

use crate::params::Params;
use crate::server::run_server;

mod chaos_filter;
mod counting_filter;
mod dns_service_discovery;
mod filter;
mod filter_outcome;
mod flaky_proxy;
mod params;
mod server;

fn main() -> pingora::Result<()> {
    let proxy_params = Params::parse_cli_args()?;
    run_server(proxy_params)
}
