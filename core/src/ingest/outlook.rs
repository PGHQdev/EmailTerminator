//! Outlook.com sign-in (PLAN.md 1.6). The app signs in as a public OAuth
//! client: the authorization code flow with PKCE (RFC 7636), the browser
//! returning to a loopback port (RFC 8252 7.3). It asks for Graph's
//! read-only `Mail.Read`. Only the refresh token is kept; each sync trades it
//! for an access token that [`super::graph`] sends.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use zeroize::Zeroizing;

/// The app's registration in Microsoft Entra: public, so it carries no secret.
pub const CLIENT_ID: &str = "a3ccc687-2d05-4830-8d27-d2c3e2c07996";
/// `common` takes personal and work accounts alike.
const AUTHORITY: &str = "https://login.microsoftonline.com/common/oauth2/v2.0";
/// Read-only mail, a refresh token, and an ID token that names the mailbox.
const SCOPE: &str = "https://graph.microsoft.com/Mail.Read offline_access openid email";
/// How long the browser has to come back.
const WAIT: Duration = Duration::from_secs(10 * 60);
/// A browser's request line and headers fit in this.
const MAX_REQUEST: usize = 16 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    /// Microsoft's own words: a denied consent, a blocked tenant, an expired
    /// refresh token.
    #[error("{0}")]
    Refused(String),
    #[error("the browser did not come back within 10 minutes")]
    TimedOut,
    #[error("cancelled")]
    Cancelled,
    #[error("cannot open a local port for the sign-in: {0}")]
    Loopback(String),
    #[error("cannot reach Microsoft: {0}")]
    Network(String),
    #[error("Microsoft's answer was not understood: {0}")]
    BadAnswer(String),
}

pub struct Tokens {
    pub access: Zeroizing<String>,
    /// Microsoft may send a new one with each refresh; the old one then stops.
    pub refresh: Option<Zeroizing<String>>,
    /// The mailbox address, from the ID token.
    pub username: Option<String>,
}

/// One sign-in in progress: the loopback port the browser returns to, and
/// the PKCE verifier and `state` that tie its answer to this request.
pub struct SignIn {
    listeners: Vec<TcpListener>,
    redirect: String,
    verifier: Zeroizing<String>,
    state: String,
    url: String,
    cancel: Arc<Notify>,
}

/// Stops a [`SignIn::finish`] that is waiting for the browser.
#[derive(Clone)]
pub struct Cancel(Arc<Notify>);

impl Cancel {
    pub fn cancel(&self) {
        self.0.notify_one();
    }
}

impl SignIn {
    pub async fn start() -> Result<Self, OAuthError> {
        let loopback = |e: std::io::Error| OAuthError::Loopback(e.to_string());
        let v4 = TcpListener::bind("127.0.0.1:0").await.map_err(loopback)?;
        let port = v4.local_addr().map_err(loopback)?.port();
        // The registered redirect is `localhost`, which a browser may resolve
        // to ::1 first. The IPv6 port is a courtesy: IPv4 alone still works.
        let mut listeners = vec![v4];
        if let Ok(v6) = TcpListener::bind(("::1", port)).await {
            listeners.push(v6);
        }
        let redirect = format!("http://localhost:{port}");
        let verifier = Zeroizing::new(random_token()?);
        let state = random_token()?;
        let url = format!(
            "{AUTHORITY}/authorize?client_id={CLIENT_ID}&response_type=code&response_mode=query\
             &redirect_uri={}&scope={}&state={state}&code_challenge={}\
             &code_challenge_method=S256&prompt=select_account",
            encode(&redirect),
            encode(SCOPE),
            challenge(&verifier),
        );
        Ok(Self {
            listeners,
            redirect,
            verifier,
            state,
            url,
            cancel: Arc::new(Notify::new()),
        })
    }

    pub fn canceller(&self) -> Cancel {
        Cancel(self.cancel.clone())
    }

    /// The page to open in the browser.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Waits for the browser to come back, then trades its code for tokens.
    pub async fn finish(self) -> Result<Tokens, OAuthError> {
        let code = tokio::select! {
            code = tokio::time::timeout(WAIT, self.code()) => {
                code.map_err(|_| OAuthError::TimedOut)??
            }
            () = self.cancel.notified() => return Err(OAuthError::Cancelled),
        };
        token(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &self.redirect),
            ("code_verifier", &self.verifier),
        ])
        .await
    }

    /// The code from the first request that carries this sign-in's `state`.
    /// Anything else on the port (a favicon, another page) is answered and
    /// ignored.
    async fn code(&self) -> Result<Zeroizing<String>, OAuthError> {
        loop {
            let accepts = self.listeners.iter().map(|l| Box::pin(l.accept()));
            let (accepted, _, _) = futures::future::select_all(accepts).await;
            let Ok((mut socket, _)) = accepted else {
                continue;
            };
            let Some(query) = read_query(&mut socket).await else {
                respond(&mut socket, "404 Not Found", "").await;
                continue;
            };
            let param = |name: &str| {
                query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
                    .map(decode)
            };
            if param("state").as_deref() != Some(self.state.as_str()) {
                respond(&mut socket, "400 Bad Request", "").await;
                continue;
            }
            if let Some(code) = param("code") {
                respond(&mut socket, "200 OK", DONE).await;
                return Ok(Zeroizing::new(code));
            }
            respond(&mut socket, "200 OK", STOPPED).await;
            let error = param("error").unwrap_or_else(|| "no code".into());
            return Err(OAuthError::Refused(match param("error_description") {
                Some(description) => first_line(&description),
                None => error,
            }));
        }
    }
}

/// Trades a refresh token for a new access token.
pub async fn refresh(refresh_token: &str) -> Result<Tokens, OAuthError> {
    token(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ])
    .await
}

async fn token(params: &[(&str, &str)]) -> Result<Tokens, OAuthError> {
    let body = Zeroizing::new(
        [("client_id", CLIENT_ID), ("scope", SCOPE)]
            .iter()
            .chain(params)
            .map(|(k, v)| format!("{k}={}", encode(v)))
            .collect::<Vec<_>>()
            .join("&"),
    );
    let network = |e: &dyn std::fmt::Display| OAuthError::Network(e.to_string());
    let answer = crate::tls::http_client()
        .map_err(|e| network(&e))?
        .post(format!("{AUTHORITY}/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| network(&e))?;
    let status = answer.status().as_u16();
    let body = answer.bytes().await.map_err(|e| network(&e))?;
    tokens(status, &body)
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

fn tokens(status: u16, body: &[u8]) -> Result<Tokens, OAuthError> {
    let answer: TokenAnswer = serde_json::from_slice(body)
        .map_err(|_| OAuthError::BadAnswer(format!("HTTP {status}")))?;
    if let Some(error) = answer.error {
        return Err(OAuthError::Refused(
            answer.error_description.map_or(error, |d| first_line(&d)),
        ));
    }
    let access = answer
        .access_token
        .filter(|_| (200..300).contains(&status))
        .ok_or_else(|| OAuthError::BadAnswer(format!("HTTP {status} without an access token")))?;
    Ok(Tokens {
        access: Zeroizing::new(access),
        refresh: answer.refresh_token.map(Zeroizing::new),
        username: answer.id_token.as_deref().and_then(username),
    })
}

/// The address an ID token names. It came straight from the token endpoint
/// over TLS, so its signature is not checked (OpenID Connect Core 3.1.3.7).
fn username(id_token: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Claims {
        email: Option<String>,
        preferred_username: Option<String>,
    }
    let payload = id_token.split('.').nth(1)?;
    let json = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: Claims = serde_json::from_slice(&json).ok()?;
    claims
        .email
        .into_iter()
        .chain(claims.preferred_username)
        .find(|a| a.contains('@'))
}

fn random_token() -> Result<String, OAuthError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| OAuthError::Loopback(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Percent-encodes all but RFC 3986's unreserved characters.
pub(crate) fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Reverses form encoding: `%XX` and `+` for a space.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).trim().to_owned()
}

/// The query of a `GET /?...` request, or `None` for any other request.
async fn read_query(socket: &mut TcpStream) -> Option<String> {
    let mut request = Vec::new();
    let mut buf = [0u8; 2048];
    let read = async {
        while !request.windows(4).any(|w| w == b"\r\n\r\n") && request.len() < MAX_REQUEST {
            match socket.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => request.extend_from_slice(&buf[..n]),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), read)
        .await
        .ok()?;
    let line = request.split(|&b| b == b'\r').next()?;
    let target = std::str::from_utf8(line)
        .ok()?
        .strip_prefix("GET ")?
        .split(' ')
        .next()?;
    target.strip_prefix("/?").map(str::to_owned)
}

async fn respond(socket: &mut TcpStream, status: &str, page: &str) {
    let answer = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    let _ = socket.write_all(answer.as_bytes()).await;
    let _ = socket.shutdown().await;
}

const DONE: &str = "<!doctype html><title>EmailTerminator</title>\
    <p style=\"font:16px system-ui;margin:3em\">Signed in. You can close this tab \
    and go back to EmailTerminator.</p>";
const STOPPED: &str = "<!doctype html><title>EmailTerminator</title>\
    <p style=\"font:16px system-ui;margin:3em\">The sign-in stopped. Go back to \
    EmailTerminator to see why.</p>";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_is_the_verifiers_sha256() {
        // The expected value comes from `openssl dgst -sha256 -binary | base64`.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mJ92K9XVZ9OZ9sKf8EJG0ELOT2Ph98"),
            "ul7U9yvfgdiXHZNQGANj06ZbMCoeQBF_wyhuIkJ1_dc"
        );
    }

    #[test]
    fn form_values_round_trip() {
        let scope = encode(SCOPE);
        assert!(scope.starts_with("https%3A%2F%2Fgraph.microsoft.com%2FMail.Read"));
        assert!(!scope.contains(' '));
        assert_eq!(decode(&scope), SCOPE);
        assert_eq!(decode("M.C5_a%21b+c%"), "M.C5_a!b c%");
    }

    #[test]
    fn a_token_answer_gives_the_mailbox_address() {
        let claims = URL_SAFE_NO_PAD.encode(r#"{"preferred_username":"me@outlook.test"}"#);
        let body =
            format!(r#"{{"access_token":"at","refresh_token":"rt","id_token":"h.{claims}.s"}}"#);
        let t = tokens(200, body.as_bytes()).unwrap();
        assert_eq!(t.access.as_str(), "at");
        assert_eq!(t.refresh.as_deref().map(String::as_str), Some("rt"));
        assert_eq!(t.username.as_deref(), Some("me@outlook.test"));
    }

    #[test]
    fn a_refusal_carries_microsofts_first_line() {
        let body = br#"{"error":"invalid_grant","error_description":"AADSTS70000: The grant expired.\r\nTrace ID: 1"}"#;
        assert!(matches!(
            tokens(400, body),
            Err(OAuthError::Refused(words)) if words == "AADSTS70000: The grant expired."
        ));
        assert!(matches!(
            tokens(502, b"<html>"),
            Err(OAuthError::BadAnswer(_))
        ));
    }

    async fn visit(port: u16, path: &str) -> String {
        let mut socket = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut answer = String::new();
        socket.read_to_string(&mut answer).await.unwrap();
        answer
    }

    #[tokio::test]
    async fn the_browser_comes_back_with_the_code() {
        let sign_in = SignIn::start().await.unwrap();
        assert!(sign_in.url().contains("code_challenge_method=S256"));
        let port: u16 = sign_in
            .redirect
            .rsplit(':')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let state = sign_in.state.clone();
        let browser = tokio::spawn(async move {
            assert!(
                visit(port, "/favicon.ico")
                    .await
                    .starts_with("HTTP/1.1 404")
            );
            assert!(
                visit(port, "/?code=x&state=forged")
                    .await
                    .starts_with("HTTP/1.1 400")
            );
            visit(port, &format!("/?code=M.C5%21a&state={state}")).await
        });
        let code = sign_in.code().await.unwrap();
        assert_eq!(code.as_str(), "M.C5!a");
        assert!(browser.await.unwrap().contains("Signed in."));
    }

    #[tokio::test]
    async fn a_denied_consent_is_refused_in_microsofts_words() {
        let sign_in = SignIn::start().await.unwrap();
        let port: u16 = sign_in
            .redirect
            .rsplit(':')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let path = format!(
            "/?error=access_denied&error_description=The+user+declined.&state={}",
            sign_in.state
        );
        tokio::spawn(async move { visit(port, &path).await });
        assert!(matches!(
            sign_in.code().await,
            Err(OAuthError::Refused(words)) if words == "The user declined."
        ));
    }

    #[tokio::test]
    async fn a_waiting_sign_in_can_be_cancelled() {
        let sign_in = SignIn::start().await.unwrap();
        sign_in.canceller().cancel();
        assert!(matches!(sign_in.finish().await, Err(OAuthError::Cancelled)));
    }
}
