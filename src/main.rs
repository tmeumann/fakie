#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]
#![warn(unsafe_code)]

use dns::Dns;
use pingora::lb::Backends;
use pingora::prelude::*;
use proxy::FlakyProxy;

mod dns;
mod proxy;

fn main() {
    let mut server = Server::new(None).unwrap();
    server.bootstrap();

    // TODO
    let domain = "cataas.com";

    let backends = Backends::new(Box::new(Dns::new(domain.to_string())));

    let load_balancer = LoadBalancer::from_backends(backends);

    let background = background_service("dns", load_balancer);

    let proxy = FlakyProxy::new(domain.to_string(), background.task());

    let mut proxy_service = http_proxy_service(&server.configuration, proxy);

    // TODO
    proxy_service.add_tcp("127.0.0.1:3030");

    server.add_service(background);
    server.add_service(proxy_service);

    server.run_forever();
}
