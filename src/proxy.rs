use crate::filter::Filter;
use async_trait::async_trait;
use pingora::http::{RequestHeader, ResponseHeader};
use pingora::lb::LoadBalancer;
use pingora::prelude::{HttpPeer, ProxyHttp, RoundRobin, Session};
use pingora::protocols::l4::socket::SocketAddr as PingoraSocketAddr;
use pingora::ErrorType;
use std::fmt::Display;
use std::hash::{BuildHasher, Hash, Hasher, RandomState};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use termcolor::{Buffer, BufferWriter, Color, ColorSpec, WriteColor};

type RequestHash = u64;

pub enum ProxyOutcome {
    RequestDropped,
    ResponseDropped,
    Forwarded,
}

impl ProxyOutcome {
    pub fn get_colour(&self) -> ColorSpec {
        let mut colour_spec = ColorSpec::new();

        let foreground = match self {
            ProxyOutcome::RequestDropped => Color::Red,
            ProxyOutcome::ResponseDropped => Color::Yellow,
            ProxyOutcome::Forwarded => Color::Green,
        };

        colour_spec.set_fg(Some(foreground));

        colour_spec
    }
}

impl Display for ProxyOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyOutcome::RequestDropped => write!(f, "REQUEST DROPPED"),
            ProxyOutcome::ResponseDropped => write!(f, "RESPONSE DROPPED"),
            ProxyOutcome::Forwarded => write!(f, "FORWARDED"),
        }
    }
}

pub struct RequestContext {
    hash: Option<RequestHash>,
    log_buffer: Buffer,
}

struct FilterTracker {
    enabled: bool,
    filter: Box<dyn Filter + Send + Sync>,
}

pub struct FlakyProxy {
    sni: String,
    load_balancer: Arc<LoadBalancer<RoundRobin>>,
    hash_state: RandomState,
    request_filters: Vec<Box<dyn Filter + Send + Sync>>,
    response_filters: Vec<Box<dyn Filter + Send + Sync>>,
    log_writer: BufferWriter,
}

impl FlakyProxy {
    pub fn new(
        sni: String,
        load_balancer: Arc<LoadBalancer<RoundRobin>>,
        request_filters: Vec<Box<dyn Filter + Send + Sync>>,
        response_filters: Vec<Box<dyn Filter + Send + Sync>>,
        log_writer: BufferWriter,
    ) -> Self {
        Self {
            sni,
            load_balancer,
            hash_state: RandomState::new(),
            request_filters,
            response_filters,
            log_writer,
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

    fn log_request(&self, buffer: &mut Buffer, description: &str, outcome: ProxyOutcome) {
        if let Err(e) = self.try_log_request(buffer, description, outcome) {
            eprintln!("Failed to log request: {}", e);
        }
    }

    fn try_log_request(
        &self,
        buffer: &mut Buffer,
        description: &str,
        outcome: ProxyOutcome,
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
}

#[async_trait]
impl ProxyHttp for FlakyProxy {
    type CTX = RequestContext;

    fn new_ctx(&self) -> Self::CTX {
        Self::CTX {
            hash: None,
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

        // TODO conditional TLS
        let peer = Box::new(HttpPeer::new(backend.addr, true, self.sni.clone()));

        Ok(peer)
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let request_hash = self.hash_request(session);

        ctx.hash = Some(request_hash);

        for f in &self.request_filters {
            if !f.filter(request_hash) {
                self.log_request(
                    &mut ctx.log_buffer,
                    &session.request_summary(),
                    ProxyOutcome::RequestDropped,
                );

                session.respond_error(503).await?;

                return Ok(true);
            }
        }

        Ok(false)
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
        session: &mut Session,
        _upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        let request_hash = ctx.hash.ok_or_else(|| {
            pingora::Error::explain(ErrorType::InternalError, "Missing request hash")
        })?;

        for f in &self.response_filters {
            if !f.filter(request_hash) {
                self.log_request(
                    &mut ctx.log_buffer,
                    &session.request_summary(),
                    ProxyOutcome::ResponseDropped,
                );

                return Err(pingora::Error::explain(
                    ErrorType::HTTPStatus(502),
                    "Response blocked",
                ));
            }
        }

        self.log_request(
            &mut ctx.log_buffer,
            &session.request_summary(),
            ProxyOutcome::Forwarded,
        );

        for f in &self.request_filters {
            f.reset(request_hash);
        }

        for f in &self.response_filters {
            f.reset(request_hash);
        }

        Ok(())
    }
}
