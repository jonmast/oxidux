use futures::{future, Future};
use std::{
    net::{SocketAddr, TcpListener, UdpSocket},
    pin::Pin,
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use tokio::runtime::Runtime;
use trust_dns_client::{
    op::{LowerQuery, ResponseCode},
    rr::{
        dnssec::{DnsSecResult, Signer, SupportedAlgorithms},
        rdata::key::KEY,
        LowerName, Name, RData, Record, RecordType,
    },
};
use trust_dns_server::{
    authority::{
        AuthLookup, Authority, Catalog, LookupError, LookupRecords, LookupResult, MessageRequest,
        UpdateResult, ZoneType,
    },
    ServerFuture,
};

use crate::proxy::launchd;

// This can probably be anything, it's built for DDOS prevention
const TCP_TIMEOUT: u64 = 5;

/// Start a local DNS server to point our development TLD to localhost
///
/// This is intended for use only with the MacOS resolver system, it can't be used as a regular DNS
/// server to do real lookups.
pub fn start_dns_server(port: u16, domain: &str, runtime: &Runtime) -> color_eyre::Result<()> {
    let dns_address = format!("127.0.0.1:{}", port);
    let mut catalog = Catalog::new();

    let name = Name::from_str(domain).unwrap();
    let authority = LocalhostAuthority {
        name: name.clone().into(),
    };
    catalog.upsert(name.into(), Box::new(authority));

    let mut server = ServerFuture::new(catalog);
    let address: SocketAddr = dns_address.parse().unwrap();

    let udp_socket =
        launchd::get_udp_socket("DnsUdpSocket").or_else(|_| UdpSocket::bind(&address))?;
    let tcp_listener =
        launchd::get_tcp_socket("DnsTcpSocket").or_else(|_| TcpListener::bind(&address))?;

    eprintln!(
        "Starting DNS server on UDP {} / TCP {}",
        udp_socket.local_addr().unwrap(),
        tcp_listener.local_addr().unwrap()
    );

    server.register_socket_std(udp_socket, runtime);
    server.register_listener_std(tcp_listener, Duration::from_secs(TCP_TIMEOUT), runtime)?;

    Ok(())
}

struct LocalhostAuthority {
    name: LowerName,
}

impl Authority for LocalhostAuthority {
    type Lookup = AuthLookup;
    type LookupFuture = future::Ready<Result<Self::Lookup, LookupError>>;

    fn zone_type(&self) -> ZoneType {
        // Using forward, not sure if this is "correct" but other types require NS handling
        ZoneType::Forward
    }

    fn is_axfr_allowed(&self) -> bool {
        false
    }

    fn update(&mut self, _update: &MessageRequest) -> UpdateResult<bool> {
        Err(ResponseCode::NotImp)
    }

    fn origin(&self) -> &LowerName {
        &self.name
    }

    fn lookup(
        &self,
        name: &LowerName,
        query_type: RecordType,
        is_secure: bool,
        _supported_algorithms: SupportedAlgorithms,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Lookup, LookupError>> + Send>> {
        let result: LookupResult<LookupRecords> = match query_type {
            RecordType::A => {
                let record =
                    Record::from_rdata(name.into(), 0, RData::A("127.0.0.1".parse().unwrap()));
                Ok(LookupRecords::new(
                    is_secure,
                    Default::default(),
                    Arc::new(record.into()),
                ))
            }
            RecordType::AAAA => {
                let record =
                    Record::from_rdata(name.into(), 0, RData::AAAA("::1".parse().unwrap()));
                Ok(LookupRecords::new(
                    is_secure,
                    Default::default(),
                    Arc::new(record.into()),
                ))
            }
            _ => Err(LookupError::ResponseCode(ResponseCode::NXDomain)),
        };

        Box::pin(future::ready(
            result.map(|answers| AuthLookup::answers(answers, None)),
        ))
    }

    fn search(
        &self,
        query: &LowerQuery,
        is_secure: bool,
        supported_algorithms: SupportedAlgorithms,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Lookup, LookupError>> + Send>> {
        // Always delegate to lookup for this, seems to be meant for handling AFXR which we're not
        // bothering with.
        Box::pin(self.lookup(
            query.name(),
            query.query_type(),
            is_secure,
            supported_algorithms,
        ))
    }

    // Following methods are for DNSSEC, which we're not supporting
    fn get_nsec_records(
        &self,
        _name: &LowerName,
        _is_secure: bool,
        _supported_algorithms: SupportedAlgorithms,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Lookup, LookupError>> + Send>> {
        Box::pin(future::ok(AuthLookup::default()))
    }

    fn add_update_auth_key(&mut self, _name: Name, _key: KEY) -> DnsSecResult<()> {
        Err("DNSSEC not supported".into())
    }

    /// This will fail, the dnssec feature must be enabled
    fn add_zone_signing_key(&mut self, _signer: Signer) -> DnsSecResult<()> {
        Err("DNSSEC not supported".into())
    }

    fn secure_zone(&mut self) -> DnsSecResult<()> {
        Err("DNSSEC not supported".into())
    }
}
