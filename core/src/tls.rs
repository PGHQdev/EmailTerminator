//! TLS for every connection the core opens: `rustls` with the `ring`
//! provider, and the OS trust store decides (PLAN.md 2.3).

use std::sync::Arc;

use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

pub(crate) async fn connect(host: &str, tcp: TcpStream) -> Result<TlsStream<TcpStream>, String> {
    use rustls_platform_verifier::BuilderVerifierExt;
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .and_then(|builder| builder.with_platform_verifier())
    .map_err(|err| err.to_string())?
    .with_no_client_auth();
    let name =
        rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(|err| err.to_string())?;
    tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
        .map_err(|err| err.to_string())
}
