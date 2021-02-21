use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::{create_dir, File};
use std::io::prelude::*;
use std::path::Path;
use std::sync::Arc;

use async_stream::stream;
use futures::stream::{Stream, StreamExt};
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use parking_lot::RwLock;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::rustls::{self, sign};
use tokio_rustls::{server::TlsStream, TlsAcceptor};
use webpki::{DNSName, DNSNameRef};

use crate::config::Config;

pub(crate) async fn tls_server(
    config: &Config,
    socket: std::net::TcpListener,
) -> color_eyre::Result<()> {
    let proxy =
        make_service_fn(|_| async move { Ok::<_, eyre::Error>(service_fn(super::handle_request)) });

    let incoming_tls_stream = tls_stream(config, socket)?.filter(|s| {
        eprintln!("Filtering");
        futures::future::ready(match s {
            Ok(_) => {
                eprintln!("Done filtering");
                true
            }
            Err(e) => {
                eprintln!("Error in TLS stream:\n{:#}", e);
                false
            }
        })
    });

    let server =
        Server::builder(hyper::server::accept::from_stream(incoming_tls_stream)).serve(proxy);

    server.await?;
    Ok(())
}

fn tls_stream(
    config: &Config,
    socket: std::net::TcpListener,
) -> color_eyre::Result<impl Stream<Item = color_eyre::Result<TlsStream<TcpStream>>>> {
    let config_dir = config.general.config_dir.clone();

    let tls_cfg = {
        let cert_resolver = CertificateResolver::new(root_cert(&config_dir).unwrap());

        let mut cfg = tokio_rustls::rustls::ServerConfig::new(rustls::NoClientAuth::new());
        cfg.cert_resolver = Arc::new(cert_resolver);
        // Configure ALPN to accept HTTP/2, HTTP/1.1 in that order.
        cfg.set_protocols(&[b"h2".to_vec(), b"http/1.1".to_vec()]);
        Arc::new(cfg)
    };

    let tls_acceptor = TlsAcceptor::from(tls_cfg);

    eprintln!("Starting TLS server on {}", socket.local_addr().unwrap());

    let listener = TcpListener::from_std(socket)?;

    let stream = stream! {
        loop {
            eprintln!("Starting loop");
            match listener.accept().await {
                Ok((sock, _addr)) => {
                    eprintln!("Accepted");
                    yield tls_acceptor.accept(sock).await.map_err(Into::into);
                },
                Err(e) => yield Err(e.into()),
            }
            eprintln!("Loop complete");
        }
    };

    Ok(stream)
}

/// Get root certificate, autogenerating it if it doesn't exist
fn root_cert(config_dir: &Path) -> color_eyre::Result<rcgen::Certificate> {
    let certs_dir = config_dir.join("certs");
    let cert_path = certs_dir.join("root-cert.der");
    let key_path = certs_dir.join("root-key.der");

    if cert_path.exists() && key_path.exists() {
        let mut root_key = Vec::new();
        File::open(key_path)?.read_to_end(&mut root_key)?;

        let mut cert_data = Vec::new();
        File::open(cert_path)?.read_to_end(&mut cert_data)?;

        let key = rcgen::KeyPair::try_from(root_key.as_slice())?;
        let cert_params = rcgen::CertificateParams::from_ca_cert_der(&cert_data, key)?;
        Ok(rcgen::Certificate::from_params(cert_params)?)
    } else {
        create_dir(certs_dir).ok();
        let mut cert_params = rcgen::CertificateParams::new(vec!["Oxidux".to_string()]);
        cert_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "Oxidux CA");
        cert_params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "Oxidux CA");
        cert_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let generated_cert = rcgen::Certificate::from_params(cert_params)?;

        File::create(cert_path)?.write_all(&generated_cert.serialize_der()?)?;
        File::create(key_path)?.write_all(&generated_cert.serialize_private_key_der())?;

        Ok(generated_cert)
    }
}

struct CertificateResolver {
    root_cert: rcgen::Certificate,
    cert_cache: RwLock<HashMap<DNSName, sign::CertifiedKey>>,
}

impl CertificateResolver {
    fn new(root_cert: rcgen::Certificate) -> Self {
        Self {
            root_cert,
            cert_cache: RwLock::new(HashMap::default()),
        }
    }

    /// Generate a new key for the provided domain and sign it with our CA key
    fn generate_certified_key(&self, domain: DNSNameRef) -> sign::CertifiedKey {
        let domain: &str = domain.into();
        let mut cert_params = rcgen::CertificateParams::new(vec![domain.to_string()]);
        cert_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, domain);
        cert_params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "Oxidux CA");
        cert_params.serial_number = Some(rand::random());
        let generated_cert = rcgen::Certificate::from_params(cert_params).unwrap();
        let cert = rustls::Certificate(
            generated_cert
                .serialize_der_with_signer(&self.root_cert)
                .unwrap(),
        );

        let pkey = rustls::PrivateKey(generated_cert.serialize_private_key_der());

        let signing_key = rustls::sign::any_supported_type(&pkey).unwrap();

        sign::CertifiedKey::new(vec![cert], Arc::new(signing_key))
    }
}

impl rustls::ResolvesServerCert for CertificateResolver {
    fn resolve(&self, client_hello: rustls::ClientHello) -> Option<sign::CertifiedKey> {
        let domain = client_hello.server_name()?.to_owned();

        if let Some(key) = self.cert_cache.read().get(&domain) {
            return Some(key.clone());
        }

        let key = self.generate_certified_key(domain.as_ref());

        self.cert_cache.write().insert(domain, key.clone());

        Some(key)
    }
}
