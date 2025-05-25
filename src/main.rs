#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]
#![warn(unsafe_code)]

use crate::counter::CountingFilter;
use clap::Parser;
use dns::Dns;
use pingora::lb::Backends;
use pingora::listeners::ServerAddress;
use pingora::prelude::*;
use proxy::FlakyProxy;
use std::io::IsTerminal;
use termcolor::{BufferWriter, ColorChoice};

mod counter;
mod dns;
mod filter;
mod proxy;

/// An HTTP proxy that intentionally drops requests and/or responses
#[derive(Parser)]
struct Args {
    /// If supplied, TLS will be used
    #[arg(short, long)]
    tls: bool,
    /// Address to listen on
    #[arg(short, long, default_value = "127.0.0.1:8000")]
    listen: String,
    /// Number of requests to drop before allowing one through
    #[arg(short, long, default_value_t = 1)]
    queries: u64,
    /// Number of responses to drop before allowing one through
    #[arg(short, long, default_value_t = 1)]
    responses: u64,
    /// Target to forward requests to
    target: String,
}

fn main() -> Result<()> {
    let Args {
        tls: _, // TODO
        listen,
        queries: _,   // TODO
        responses: _, // TODO
        target,
    } = Args::parse();

    let mut server = Server::new(None)?;
    server.bootstrap();

    let backends = Backends::new(Box::new(Dns::new(target.clone())));

    let load_balancer = LoadBalancer::from_backends(backends);

    let background = background_service("dns", load_balancer);

    let colour_choice = if std::io::stdin().is_terminal() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };

    let log_writer = BufferWriter::stdout(colour_choice);

    let proxy = FlakyProxy::new(
        target,
        background.task(),
        vec![Box::new(CountingFilter::new(1))],
        vec![Box::new(CountingFilter::new(1))],
        log_writer,
    );

    let mut proxy_service = http_proxy_service(&server.configuration, proxy);

    proxy_service.add_address(ServerAddress::Tcp(listen.clone(), None));

    server.add_service(background);
    server.add_service(proxy_service);

    println!("🛹🛹🛹 flaking out on {listen} 🛹🛹🛹");

    server.run_forever();
}
