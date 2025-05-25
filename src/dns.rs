use async_trait::async_trait;
use hickory_resolver::config::ResolverConfig;
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::Resolver;
use pingora::lb::discovery::ServiceDiscovery;
use pingora::lb::{Backend, Extensions};
use pingora::protocols::l4::socket::SocketAddr::Inet;
use std::collections::{BTreeSet, HashMap};
use std::iter;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

pub struct DnsCapableDiscovery {
    host: String,
    port: u16,
    resolver: Resolver<TokioConnectionProvider>,
}

impl DnsCapableDiscovery {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            resolver: Resolver::builder_with_config(
                ResolverConfig::default(),
                TokioConnectionProvider::default(),
            )
            .build(),
        }
    }

    fn build_tree(&self, ip_addrs: impl Iterator<Item = IpAddr>) -> BTreeSet<Backend> {
        BTreeSet::from_iter(ip_addrs.map(|addr| Backend {
            addr: Inet(SocketAddr::new(addr, self.port)),
            weight: 1,
            ext: Extensions::new(),
        }))
    }
}

#[async_trait]
impl ServiceDiscovery for DnsCapableDiscovery {
    async fn discover(&self) -> pingora::Result<(BTreeSet<Backend>, HashMap<u64, bool>)> {
        let backends = if let Ok(ip_addr) = IpAddr::from_str(&self.host) {
            self.build_tree(iter::once(ip_addr))
        } else {
            let resolved_ips = self
                .resolver
                .lookup_ip(&self.host)
                .await
                .unwrap_or_else(|e| {
                    // Pingora swallows errors here, so we fail spectacularly instead
                    eprintln!("DNS resolution error: {}", e);
                    std::process::exit(-1)
                });

            self.build_tree(resolved_ips.iter())
        };

        Ok((backends, HashMap::new()))
    }
}
