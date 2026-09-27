//! TLS for every connection the core opens: `rustls` with the `ring`
//! provider, and the OS trust store decides (PLAN.md 2.3).

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

pub(crate) fn config() -> Result<rustls::ClientConfig, String> {
    use rustls_platform_verifier::BuilderVerifierExt;
    Ok(rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .and_then(|builder| builder.with_platform_verifier())
    .map_err(|err| err.to_string())?
    .with_no_client_auth())
}

pub(crate) async fn connect(host: &str, tcp: TcpStream) -> Result<TlsStream<TcpStream>, String> {
    let name =
        rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(|err| err.to_string())?;
    tokio_rustls::TlsConnector::from(Arc::new(config()?))
        .connect(name, tcp)
        .await
        .map_err(|err| err.to_string())
}

/// An HTTPS client over the same TLS setup, for Microsoft's sign-in and
/// Graph. It keeps connections open across requests, sends no cookies, and
/// follows no redirects.
pub(crate) fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .tls_backend_preconfigured(config()?)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("EmailTerminator")
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_https_client_builds_with_the_ring_provider() {
        super::http_client().unwrap();
    }

    /// Needs the network: `cargo test -p et-core -- --ignored graph_answers`.
    #[tokio::test]
    #[ignore]
    async fn graph_answers_over_tls() {
        let answer = super::http_client()
            .unwrap()
            .get("https://graph.microsoft.com/v1.0/me")
            .send()
            .await
            .unwrap();
        assert_eq!(answer.status().as_u16(), 401);
    }
}
