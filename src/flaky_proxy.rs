use crate::filter::Filter;
use crate::filter_outcome::FilterOutcome;
use async_trait::async_trait;
use pingora::http::{RequestHeader, ResponseHeader};
use pingora::lb::LoadBalancer;
use pingora::prelude::{HttpPeer, ProxyHttp, RoundRobin, Session};
use pingora::protocols::l4::socket::SocketAddr as PingoraSocketAddr;
use pingora::ErrorType;
use std::hash::{BuildHasher, Hash, Hasher, RandomState};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use termcolor::{Buffer, BufferWriter, WriteColor};

type RequestHash = u64;

pub struct RequestContext {
    outcome: FilterOutcome,
    log_buffer: Buffer,
}

pub struct FlakyProxy {
    sni: String,
    load_balancer: Arc<LoadBalancer<RoundRobin>>,
    hash_state: RandomState,
    filters: Vec<Box<dyn Filter + Send + Sync>>,
    log_writer: BufferWriter,
    tls: bool,
}

impl FlakyProxy {
    pub fn new(
        sni: String,
        load_balancer: Arc<LoadBalancer<RoundRobin>>,
        filters: Vec<Box<dyn Filter + Send + Sync>>,
        log_writer: BufferWriter,
        tls: bool,
    ) -> Self {
        Self {
            sni,
            load_balancer,
            hash_state: RandomState::new(),
            filters,
            log_writer,
            tls,
        }
    }

    fn hash_request(&self, session: &Session) -> RequestHash {
        let mut hasher = self.hash_state.build_hasher();

        let request_header = session.req_header();

        request_header.method.hash(&mut hasher);
        request_header.uri.hash(&mut hasher);

        let client_ip = session
            .client_addr()
            .and_then(PingoraSocketAddr::as_inet)
            .map(SocketAddr::ip);

        client_ip.hash(&mut hasher);

        hasher.finish()
    }

    fn log_request(&self, buffer: &mut Buffer, description: &str, outcome: FilterOutcome) {
        if let Err(e) = self.try_log_request(buffer, description, outcome) {
            eprintln!("Failed to log request: {}", e);
        }
    }

    fn try_log_request(
        &self,
        buffer: &mut Buffer,
        description: &str,
        outcome: FilterOutcome,
    ) -> Result<(), std::io::Error> {
        buffer.reset()?;

        write!(buffer, "{description} ")?;

        buffer.set_color(&outcome.get_colour())?;
        write!(buffer, "{outcome}")?;
        buffer.reset()?;

        writeln!(buffer)?;

        self.log_writer.print(buffer)?;

        Ok(())
    }

    fn reset_filters(&self, request_hash: RequestHash) {
        for f in &self.filters {
            f.reset(request_hash);
        }
    }
}

#[async_trait]
impl ProxyHttp for FlakyProxy {
    type CTX = RequestContext;

    fn new_ctx(&self) -> Self::CTX {
        Self::CTX {
            outcome: FilterOutcome::Passed,
            log_buffer: self.log_writer.buffer(),
        }
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

        let peer = Box::new(HttpPeer::new(backend.addr, self.tls, self.sni.clone()));

        Ok(peer)
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let request_hash = self.hash_request(session);

        for f in &self.filters {
            ctx.outcome = f.filter(request_hash);

            if ctx.outcome != FilterOutcome::Passed {
                break;
            }
        }

        self.log_request(&mut ctx.log_buffer, &session.request_summary(), ctx.outcome);

        let response_sent = match ctx.outcome {
            FilterOutcome::Passed => {
                self.reset_filters(request_hash);
                false
            }
            FilterOutcome::RequestDenied => {
                session.respond_error(503).await?;
                true
            }
            FilterOutcome::ResponseDenied => false,
        };

        Ok(response_sent)
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
        if ctx.outcome == FilterOutcome::ResponseDenied {
            Err(pingora::Error::explain(
                ErrorType::HTTPStatus(502),
                "Response blocked",
            ))
        } else {
            Ok(())
        }
    }
}
