use crate::dns_service_discovery::DnsServiceDiscovery;
use crate::flaky_proxy::FlakyProxy;
use crate::params::Params;
use pingora::lb::{Backends, LoadBalancer};
use pingora::listeners::ServerAddress;
use pingora::prelude::{Server, background_service, http_proxy_service};
use std::io::IsTerminal;
use termcolor::{BufferWriter, ColorChoice};

pub fn run_server(params: Params) -> anyhow::Result<()> {
    let Params {
        listen_addr,
        upstream_host,
        upstream_port,
        upstream_is_tls: is_tls,
        filters,
    } = params;

    let mut server = Server::new(None)?;

    let dns = background_service(
        "dns",
        LoadBalancer::from_backends(Backends::new(Box::new(DnsServiceDiscovery::new(
            upstream_host.clone(),
            upstream_port,
        )?))),
    );

    let colour_choice = if std::io::stdin().is_terminal() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };

    let log_writer = BufferWriter::stdout(colour_choice);

    let flaky_proxy = FlakyProxy::new(upstream_host, dns.task(), filters, log_writer, is_tls);

    let mut proxy_service = http_proxy_service(&server.configuration, flaky_proxy);

    proxy_service.add_address(ServerAddress::Tcp(listen_addr.clone(), None));

    server.add_service(dns);
    server.add_service(proxy_service);

    println!("🛹🛹🛹 flaking out on {listen_addr} 🛹🛹🛹");

    server.run_forever();
}
