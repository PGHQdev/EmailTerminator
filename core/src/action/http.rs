//! The two HTTPS requests the core sends: RFC 8058's one-click POST, whose
//! answer is a status code, and the Outlook token request, whose answer is a
//! JSON body. Written over the TLS stack the IMAP client already uses, so the
//! core adds no HTTP library.
//!
//! RFC 8058 §3.2: the POST carries no cookies and no credentials. Redirects
//! are not followed; only a 2xx answer counts as done.

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

const TIMEOUT: Duration = Duration::from_secs(20);
/// A status line is short; a server that sends more than this before one is
/// not answering HTTP.
const MAX_HEAD: usize = 8 * 1024;
/// A token answer is a few kilobytes; more than this is not one.
const MAX_ANSWER: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HttpError {
    #[error("not an HTTPS address")]
    BadUrl,
    #[error("cannot reach {host}: {message}")]
    Unreachable { host: String, message: String },
    #[error("{host} did not answer within 20 seconds")]
    Timeout { host: String },
    #[error("{host} did not answer in HTTP")]
    NotHttp { host: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    /// As `connect` takes it: an IPv6 literal without brackets.
    pub host: String,
    pub port: u16,
    /// The `Host` header: the authority as written, minus a default port.
    pub authority: String,
    /// Path and query; `/` when the URL has neither.
    pub path: String,
}

/// Reads `https://host[:port][/path][?query][#fragment]`. A URL carrying
/// credentials, whitespace or a control character is refused.
pub(crate) fn parse(url: &str) -> Result<Target, HttpError> {
    let (scheme, rest) = url.split_once("://").ok_or(HttpError::BadUrl)?;
    if !scheme.eq_ignore_ascii_case("https")
        || url.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(HttpError::BadUrl);
    }
    let rest = rest.split('#').next().unwrap_or("");
    let split = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(split);
    if authority.is_empty() || authority.contains('@') {
        return Err(HttpError::BadUrl);
    }
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (host, after) = v6.split_once(']').ok_or(HttpError::BadUrl)?;
        (host, after.strip_prefix(':'))
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port = match port {
        Some(p) => p.parse().map_err(|_| HttpError::BadUrl)?,
        None => 443,
    };
    if host.is_empty() {
        return Err(HttpError::BadUrl);
    }
    let authority = if port == 443 {
        authority.trim_end_matches(":443").to_owned()
    } else {
        authority.to_owned()
    };
    let path = match path {
        "" => "/".to_owned(),
        p if p.starts_with('?') => format!("/{p}"),
        p => p.to_owned(),
    };
    Ok(Target {
        host: host.to_ascii_lowercase(),
        port,
        authority,
        path,
    })
}

/// POSTs `body` as a form to `url` and returns the status code.
pub async fn post_form(url: &str, body: &str) -> Result<u16, HttpError> {
    let target = parse(url)?;
    let host = target.host.clone();
    let exchange = async {
        let tls = open(&target).await?;
        exchange(tls, &target, body).await
    };
    tokio::time::timeout(TIMEOUT, exchange)
        .await
        .map_err(|_| HttpError::Timeout { host })?
}

/// The status code and body of an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
}

/// POSTs `body` as a form to `url` and reads the whole answer.
pub async fn post_form_read(url: &str, body: &str) -> Result<Answer, HttpError> {
    let target = parse(url)?;
    let host = target.host.clone();
    let exchange = async {
        let tls = open(&target).await?;
        exchange_read(tls, &target, body).await
    };
    tokio::time::timeout(TIMEOUT, exchange)
        .await
        .map_err(|_| HttpError::Timeout { host })?
}

async fn open(target: &Target) -> Result<TlsStream<TcpStream>, HttpError> {
    let unreachable = |message: String| HttpError::Unreachable {
        host: target.host.clone(),
        message,
    };
    let tcp = TcpStream::connect((target.host.as_str(), target.port))
        .await
        .map_err(|e| unreachable(e.to_string()))?;
    crate::tls::connect(&target.host, tcp)
        .await
        .map_err(unreachable)
}

async fn send<S: AsyncWrite + Unpin>(
    stream: &mut S,
    target: &Target,
    body: &str,
) -> Result<(), HttpError> {
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: EmailTerminator\r\n\
         Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        target.path,
        target.authority,
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| HttpError::Unreachable {
            host: target.host.clone(),
            message: e.to_string(),
        })?;
    stream.flush().await.ok();
    Ok(())
}

pub(crate) async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    target: &Target,
    body: &str,
) -> Result<u16, HttpError> {
    send(&mut stream, target, body).await?;
    let host = target.host.clone();
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while !head.windows(2).any(|w| w == b"\r\n") {
        let n = stream.read(&mut buf).await.unwrap_or(0);
        if n == 0 || head.len() > MAX_HEAD {
            return Err(HttpError::NotHttp { host });
        }
        head.extend_from_slice(&buf[..n]);
    }
    status(&head).ok_or(HttpError::NotHttp { host })
}

/// Sends the request and reads until the server closes, as `Connection:
/// close` asks.
pub(crate) async fn exchange_read<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    target: &Target,
    body: &str,
) -> Result<Answer, HttpError> {
    send(&mut stream, target, body).await?;
    let not_http = || HttpError::NotHttp {
        host: target.host.clone(),
    };
    let mut raw = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        // A TLS peer that closes without close_notify still ended the answer.
        let n = stream.read(&mut buf).await.unwrap_or(0);
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..n]);
        if raw.len() > MAX_HEAD + MAX_ANSWER {
            return Err(not_http());
        }
    }
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(not_http)?;
    let (head, rest) = (&raw[..split], &raw[split + 4..]);
    let status = status(head).ok_or_else(not_http)?;
    let head = String::from_utf8_lossy(head).to_ascii_lowercase();
    let header = |name: &str| {
        head.split("\r\n")
            .find_map(|line| line.strip_prefix(name)?.trim_start().strip_prefix(':'))
            .map(str::trim)
    };
    let body = if header("transfer-encoding").is_some_and(|v| v.contains("chunked")) {
        dechunk(rest).ok_or_else(not_http)?
    } else if let Some(length) = header("content-length") {
        let length: usize = length.parse().map_err(|_| not_http())?;
        rest.get(..length).ok_or_else(not_http)?.to_vec()
    } else {
        rest.to_vec()
    };
    Ok(Answer { status, body })
}

/// Joins a chunked body (RFC 9112 7.1). Trailers are ignored.
fn dechunk(mut rest: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let end = rest.windows(2).position(|w| w == b"\r\n")?;
        let line = std::str::from_utf8(&rest[..end]).ok()?;
        let size = usize::from_str_radix(line.split(';').next()?.trim(), 16).ok()?;
        rest = &rest[end + 2..];
        if size == 0 {
            return Some(body);
        }
        body.extend_from_slice(rest.get(..size)?);
        rest = rest.get(size + 2..)?;
    }
}

/// The code in a status line such as `HTTP/1.1 204 No Content`.
fn status(head: &[u8]) -> Option<u16> {
    let line = head.split(|&b| b == b'\r').next()?;
    let line = std::str::from_utf8(line).ok()?;
    let mut words = line.split(' ');
    words.next()?.starts_with("HTTP/1.").then_some(())?;
    let code = words.next()?;
    (code.len() == 3).then_some(())?;
    code.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_split_into_host_port_and_path() {
        let t = parse("https://List.Example.test/u/8a1f?x=1#top").unwrap();
        assert_eq!(
            (
                t.host.as_str(),
                t.port,
                t.authority.as_str(),
                t.path.as_str()
            ),
            ("list.example.test", 443, "List.Example.test", "/u/8a1f?x=1")
        );
        let t = parse("HTTPS://a.test:8443?u=1").unwrap();
        assert_eq!(
            (t.port, t.authority.as_str(), t.path.as_str()),
            (8443, "a.test:8443", "/?u=1")
        );
        let t = parse("https://a.test:443").unwrap();
        assert_eq!((t.authority.as_str(), t.path.as_str()), ("a.test", "/"));
        let t = parse("https://[2001:db8::1]:444/u").unwrap();
        assert_eq!((t.host.as_str(), t.port), ("2001:db8::1", 444));
    }

    #[test]
    fn unusable_urls_are_refused() {
        for url in [
            "http://a.test/u",
            "mailto:u@a.test",
            "https://user:pw@a.test/u",
            "https:///u",
            "https://a.test:x/u",
            "https://a.test/u v",
            "https://a.test/\r\nX: 1",
        ] {
            assert_eq!(parse(url), Err(HttpError::BadUrl), "{url}");
        }
    }

    #[test]
    fn status_lines_parse() {
        assert_eq!(status(b"HTTP/1.1 204 No Content\r\n"), Some(204));
        assert_eq!(status(b"HTTP/1.0 404\r\n"), Some(404));
        assert_eq!(status(b"SSH-2.0-OpenSSH\r\n"), None);
        assert_eq!(status(b"HTTP/1.1 20 OK\r\n"), None);
    }

    #[tokio::test]
    async fn the_request_is_a_bare_form_post() {
        let (client, mut server) = tokio::io::duplex(4096);
        let target = parse("https://news.test/u/8a1f?l=w").unwrap();
        let answer = tokio::spawn(async move {
            let mut got = vec![0u8; 4096];
            let n = server.read(&mut got).await.unwrap();
            server
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            String::from_utf8(got[..n].to_vec()).unwrap()
        });
        let code = exchange(client, &target, "List-Unsubscribe=One-Click")
            .await
            .unwrap();
        let request = answer.await.unwrap();
        assert_eq!(code, 200);
        assert!(request.starts_with("POST /u/8a1f?l=w HTTP/1.1\r\nHost: news.test\r\n"));
        assert!(request.ends_with("\r\n\r\nList-Unsubscribe=One-Click"));
        assert!(request.contains("Content-Length: 26\r\n"));
        assert!(!request.to_ascii_lowercase().contains("cookie"));
    }

    #[tokio::test]
    async fn a_silent_close_is_not_http() {
        let (client, server) = tokio::io::duplex(4096);
        drop(server);
        let target = parse("https://news.test/u").unwrap();
        assert!(matches!(
            exchange(client, &target, "x").await,
            Err(HttpError::Unreachable { .. } | HttpError::NotHttp { .. })
        ));
    }

    #[tokio::test]
    async fn a_whole_answer_is_read() {
        for answer in [
            &b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 9\r\n\r\n{\"a\":\"b\"}xx"[..],
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n5;x=1\r\n:\"b\"}\r\n0\r\n\r\n",
            b"HTTP/1.1 200 OK\r\n\r\n{\"a\":\"b\"}",
        ] {
            let (client, mut server) = tokio::io::duplex(4096);
            let target = parse("https://login.test/token").unwrap();
            tokio::spawn(async move {
                let mut got = vec![0u8; 4096];
                let _ = server.read(&mut got).await.unwrap();
                server.write_all(answer).await.unwrap();
            });
            let got = exchange_read(client, &target, "grant_type=x").await.unwrap();
            assert_eq!(got.status, 200);
            assert_eq!(
                String::from_utf8(got.body).unwrap(),
                r#"{"a":"b"}"#,
                "{}",
                String::from_utf8_lossy(answer)
            );
        }
    }

    #[tokio::test]
    async fn a_cut_chunk_is_not_http() {
        let (client, mut server) = tokio::io::duplex(4096);
        let target = parse("https://login.test/token").unwrap();
        tokio::spawn(async move {
            let mut got = vec![0u8; 4096];
            let _ = server.read(&mut got).await.unwrap();
            server
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n9\r\n{\"a\"")
                .await
                .unwrap();
        });
        assert!(matches!(
            exchange_read(client, &target, "x").await,
            Err(HttpError::NotHttp { .. })
        ));
    }
}
