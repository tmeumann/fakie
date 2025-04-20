use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;
use async_trait::async_trait;
use dashmap::DashMap;
use hickory_resolver::config::ResolverConfig;
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::{Resolver};
use pingora::http::ResponseHeader;
use pingora::lb::{Backend, Backends};
use pingora::lb::discovery::ServiceDiscovery;
use pingora::prelude::*;
use pingora::protocols::l4::socket::SocketAddr::Inet;

type RequestHash = u64;

struct Dns {
    domain: String,
    resolver: Resolver<TokioConnectionProvider>,
}

impl Dns {
    fn new(domain: String) -> Self {
        Self {
            domain,
            resolver: Resolver::builder_with_config(
                ResolverConfig::default(),
                TokioConnectionProvider::default()
            ).build()
        }
    }
}

#[async_trait]
impl ServiceDiscovery for Dns {
    async fn discover(&self) -> Result<(BTreeSet<Backend>, HashMap<u64, bool>)> {
        let hosts = self.resolver.lookup_ip(self.domain.clone()).await.unwrap();

        let backends = BTreeSet::from_iter(hosts.iter().map(|ip| Backend {
            // TODO conditional TLS
            addr: Inet(SocketAddr::new(ip, 443)),
            weight: 1,
            ext: Default::default(),
        }).inspect(|backend| println!("Found backend: {:?}", backend)));

        Ok((backends, Default::default()))
    }
}

struct FlakyProxy {
    sni: String,
    load_balancer: Arc<LoadBalancer<RoundRobin>>,
    requests: DashMap<RequestHash, u64>
}

impl FlakyProxy {
    fn new(sni: String, load_balancer: Arc<LoadBalancer<RoundRobin>>) -> Self {
        Self {
            sni,
            load_balancer,
            requests: DashMap::new()
        }
    }
}

#[async_trait]
impl ProxyHttp for FlakyProxy {
    type CTX = ();

    fn new_ctx(&self) -> Self::CTX {}

    async fn upstream_peer(&self, _session: &mut Session, _ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        let backend = self.load_balancer.select(b"", 256).unwrap();
        // TODO conditional TLS
        Ok(Box::new(HttpPeer::new(backend.addr, true, self.sni.clone())))
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        upstream_request.insert_header("Host", format!("{}", self.sni))?;
        Ok(())
    }

    fn upstream_response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) {
        println!("RESP: {upstream_response:#?}");
    }
}

fn main() {
    let mut my_server = Server::new(None).unwrap();
    my_server.bootstrap();

    let domain = "cataas.com";

    let backends = Backends::new(Box::new(Dns::new(domain.to_string())));

    let load_balancer = LoadBalancer::from_backends(backends);

    let background = background_service("dns", load_balancer);

    let proxy = FlakyProxy::new(domain.to_string(), background.task());

    let mut proxy_service = http_proxy_service(&my_server.configuration, proxy);

    proxy_service.add_tcp("127.0.0.1:3030");

    my_server.add_service(background);

    my_server.add_service(proxy_service);

    my_server.run_forever();
}
