use async_trait::async_trait;
use dashmap::DashMap;
use pingora::http::{RequestHeader, ResponseHeader};
use pingora::lb::LoadBalancer;
use pingora::prelude::{HttpPeer, ProxyHttp, RoundRobin, Session};
use pingora::protocols::l4::socket::SocketAddr as PingoraSocketAddr;
use pingora::ErrorType;
use std::hash::{BuildHasher, Hash, Hasher, RandomState};
use std::net::SocketAddr;
use std::sync::Arc;

type RequestHash = u64;

pub struct FlakyProxy {
    sni: String,
    load_balancer: Arc<LoadBalancer<RoundRobin>>,
    hash_state: RandomState,
    request_counters: DashMap<RequestHash, u64>,
    response_counters: DashMap<RequestHash, u64>,
}

impl FlakyProxy {
    pub fn new(sni: String, load_balancer: Arc<LoadBalancer<RoundRobin>>) -> Self {
        Self {
            sni,
            load_balancer,
            hash_state: RandomState::new(),
            request_counters: DashMap::new(),
            response_counters: DashMap::new(),
        }
    }

    fn hash_request(&self, session: &Session) -> RequestHash {
        let mut hasher = self.hash_state.build_hasher();

        let request_header = session.req_header();

        request_header.method.hash(&mut hasher);
        request_header.uri.hash(&mut hasher);
        request_header.version.hash(&mut hasher);

        session
            .client_addr()
            .and_then(PingoraSocketAddr::as_inet)
            .map(SocketAddr::ip)
            .hash(&mut hasher);

        hasher.finish()
    }
}

#[async_trait]
impl ProxyHttp for FlakyProxy {
    type CTX = Option<RequestHash>;

    fn new_ctx(&self) -> Self::CTX {
        None
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let request_hash = self.hash_request(session);

        *ctx = Some(request_hash);

        let previous_attempts = *self
            .request_counters
            .entry(request_hash)
            .and_modify(|v| *v = v.saturating_add(1))
            .or_insert(0);

        if previous_attempts >= 1 {
            return Ok(false);
        }

        session.respond_error(503).await?;

        Ok(true)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        let backend = self
            .load_balancer
            .select(b"", 256)
            .ok_or_else(|| pingora::Error::explain(ErrorType::ConnectError, "DNS failure"))?;

        // TODO conditional TLS
        let peer = Box::new(HttpPeer::new(backend.addr, true, self.sni.clone()));

        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        upstream_request.insert_header("Host", &self.sni)?;
        Ok(())
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        _upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        let request_hash = ctx.ok_or_else(|| {
            pingora::Error::explain(ErrorType::InternalError, "Missing request hash")
        })?;

        let previous_attempts = *self
            .response_counters
            .entry(request_hash)
            .and_modify(|v| *v = v.saturating_add(1))
            .or_insert(0);

        if previous_attempts >= 1 {
            Ok(())
        } else {
            Err(pingora::Error::explain(
                ErrorType::HTTPStatus(503),
                "Response blocked",
            ))
        }
    }
}
