use crate::chaos_filter::ChaosFilter;
use crate::counting_filter::CountingFilter;
use crate::filter::Filter;
use crate::filter_outcome::FilterOutcome;
use clap::{Args, Command, FromArgMatches, Parser};
use pingora::Custom;
use url::Url;

/// fakie – the flaky web proxy
///
/// Filters are reset after a successful request/response is achieved.
#[derive(Parser)]
struct FakieArgs {
    /// Drop this many send attempts for any given request
    #[arg(short, long)]
    sends: Option<u64>,
    /// Drop this many server responses for any given request
    #[arg(short, long)]
    responses: Option<u64>,
    /// Chaos mode - drop the supplied percentage of traffic
    #[arg(short, long)]
    chaos: Option<f64>,
    /// Listen address
    #[arg(short, long, default_value = "127.0.0.1:8000")]
    listen: String,
    /// The target server to forward to (eg. https://cataas.com or 127.0.0.1:8080)
    target_server: String,
}

type BoxedFilter = Box<dyn Filter + Send + Sync>;

pub struct Params {
    pub listen_addr: String,
    pub upstream_host: String,
    pub upstream_port: u16,
    pub upstream_is_tls: bool,
    pub filters: Vec<BoxedFilter>,
}

impl Params {
    pub fn parse_cli_args() -> pingora::Result<Self> {
        let cli = FakieArgs::augment_args(Command::new("fakie"));

        let argument_matches = cli.get_matches();

        let FakieArgs {
            listen: listen_addr,
            sends: requests,
            responses,
            chaos,
            target_server: target,
        } = FakieArgs::from_arg_matches(&argument_matches)
            .map_err(|_| pingora::Error::new(Custom("Invalid arguments")))?;

        let requests_index = argument_matches.index_of("requests");
        let responses_index = argument_matches.index_of("responses");
        let chaos_index = argument_matches.index_of("chaos");

        let mut indexed_filters = [
            (requests_index, Self::request_count_filter(requests)),
            (responses_index, Self::response_count_filter(responses)),
            (chaos_index, Self::chaos_filter(chaos)?),
        ];

        indexed_filters.sort_by_key(|(i, _)| *i);

        let filters = indexed_filters.into_iter().flat_map(|(_, f)| f).collect();

        let target_url = if let Ok(url) = Url::parse(&target) {
            url
        } else {
            Url::parse(&format!("http://{target}"))
                .map_err(|_| pingora::Error::new(Custom("Invalid target")))?
        };

        let upstream_is_tls = target_url.scheme() != "http";

        let upstream_host = target_url
            .host_str()
            .ok_or_else(|| pingora::Error::new(Custom("Target must have a host")))?
            .into();

        let upstream_port = target_url
            .port()
            .unwrap_or(if upstream_is_tls { 443 } else { 80 });

        Ok(Self {
            listen_addr,
            upstream_host,
            upstream_port,
            upstream_is_tls,
            filters,
        })
    }

    fn request_count_filter(n: Option<u64>) -> Option<BoxedFilter> {
        n.map(|n| Box::new(CountingFilter::new(n, FilterOutcome::RequestDenied)) as BoxedFilter)
    }

    fn response_count_filter(n: Option<u64>) -> Option<BoxedFilter> {
        n.map(|n| Box::new(CountingFilter::new(n, FilterOutcome::ResponseDenied)) as BoxedFilter)
    }

    fn chaos_filter(percentage: Option<f64>) -> pingora::Result<Option<BoxedFilter>> {
        if let Some(p) = percentage {
            let filter = ChaosFilter::new(p / 100.0)
                .map_err(|_| pingora::Error::new(Custom("Invalid chaos percentage")))?;
            Ok(Some(Box::new(filter)))
        } else {
            Ok(None)
        }
    }
}
