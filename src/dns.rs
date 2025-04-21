use async_trait::async_trait;
use hickory_resolver::config::ResolverConfig;
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::Resolver;
use pingora::lb::discovery::ServiceDiscovery;
use pingora::lb::{Backend, Extensions};
use pingora::protocols::l4::socket::SocketAddr::Inet;
use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;

pub struct Dns {
    domain: String,
    resolver: Resolver<TokioConnectionProvider>,
}

impl Dns {
    pub fn new(domain: String) -> Self {
        Self {
            domain,
            resolver: Resolver::builder_with_config(
                ResolverConfig::default(),
                TokioConnectionProvider::default(),
            )
            .build(),
        }
    }
}

#[async_trait]
impl ServiceDiscovery for Dns {
    async fn discover(&self) -> pingora::Result<(BTreeSet<Backend>, HashMap<u64, bool>)> {
        let hosts = self.resolver.lookup_ip(&self.domain).await.unwrap();

        let backends = BTreeSet::from_iter(hosts.iter().map(|ip| Backend {
            // TODO conditional TLS
            addr: Inet(SocketAddr::new(ip, 443)),
            weight: 1,
            ext: Extensions::new(),
        }));

        Ok((backends, HashMap::new()))
    }
}
