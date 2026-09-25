//! IMAP sync over an app password (PLAN.md 1.6 rung 1, 2.3).
//!
//! Every folder is opened with `EXAMINE` and every message is fetched with
//! `BODY.PEEK`, so reading mail never marks it read. Only new UIDs are fetched:
//! the resume point is the highest UID committed per folder, and a batch and
//! its resume point are committed together, so a dropped connection resumes
//! without duplicates. Expunges and flag changes are ignored on purpose: a
//! stored row outlives its message (PLAN.md Part 4), so CONDSTORE and QRESYNC
//! would add round trips and change nothing.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use async_imap::imap_proto::NameAttribute;
use futures::TryStreamExt;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

/// Messages are read up to this size. Receipts and list headers sit well
/// inside it; the rest of a large attachment is never downloaded.
const MAX_BYTES: u32 = 2 * 1024 * 1024;
const BATCH: usize = 200;

/// Servers for the app-password providers (PLAN.md 1.6). Outlook.com takes
/// OAuth only and arrives at M5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub label: &'static str,
    pub host: &'static str,
    pub port: u16,
    /// Where the provider explains app passwords.
    pub guide: &'static str,
}

pub const GMAIL: Preset = Preset {
    label: "Gmail",
    host: "imap.gmail.com",
    port: 993,
    guide: "https://support.google.com/accounts/answer/185833",
};
pub const ICLOUD: Preset = Preset {
    label: "iCloud Mail",
    host: "imap.mail.me.com",
    port: 993,
    guide: "https://support.apple.com/en-us/102654",
};
pub const FASTMAIL: Preset = Preset {
    label: "Fastmail",
    host: "imap.fastmail.com",
    port: 993,
    guide: "https://www.fastmail.help/hc/en-us/articles/360058752854",
};
pub const YAHOO: Preset = Preset {
    label: "Yahoo Mail",
    host: "imap.mail.yahoo.com",
    port: 993,
    guide: "https://help.yahoo.com/kb/SLN15241.html",
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub security: Security,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    /// Implicit TLS, the port 993 default every target provider offers.
    Tls,
    /// Plain TCP. Refused unless the host is loopback; it exists for the stub
    /// server in tests.
    PlainLoopback,
}

#[derive(Debug)]
pub struct Fetched {
    pub uid: u32,
    pub raw: Vec<u8>,
}

/// Where fetched messages go. Blocking: the store has one writer thread.
pub trait SyncTarget: Send + Sync + 'static {
    /// The highest UID already committed for `folder`. A changed
    /// `uid_validity` means the server renumbered the folder: the target
    /// forgets it and answers 0.
    fn resume(&self, folder: &str, uid_validity: u32) -> Result<u32, String>;
    /// Stores `batch` and moves the folder's resume point past it, atomically.
    fn commit(&self, folder: &str, uid_validity: u32, batch: Vec<Fetched>) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Reconnects after a network or protocol error before giving up.
    pub retries: u32,
    /// The first reconnect waits this long; each later one doubles it.
    pub backoff: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            retries: 4,
            backoff: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub fetched: u64,
    pub total: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub folders: Vec<String>,
    pub fetched: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ImapError {
    /// The server refused the credentials. S16 shows the server's words.
    #[error("sign-in refused: {0}")]
    Auth(String),
    #[error("cannot reach {host}: {source}")]
    Connect {
        host: String,
        source: std::io::Error,
    },
    #[error("TLS: {0}")]
    Tls(String),
    #[error("plain IMAP is allowed only to loopback")]
    PlainRefused,
    #[error("IMAP: {0}")]
    Protocol(String),
    #[error("storing messages: {0}")]
    Target(String),
    #[error("cancelled")]
    Cancelled,
}

impl ImapError {
    fn retryable(&self) -> bool {
        matches!(self, Self::Connect { .. } | Self::Protocol(_))
    }
}

impl From<async_imap::error::Error> for ImapError {
    fn from(err: async_imap::error::Error) -> Self {
        Self::Protocol(err.to_string())
    }
}

type Session = async_imap::Session<Stream>;

/// Syncs every folder worth reading. Reconnects with exponential backoff on
/// network and protocol errors, resuming from the last committed batch.
pub async fn sync(
    account: &Account,
    password: &str,
    target: Arc<dyn SyncTarget>,
    cancel: Arc<AtomicBool>,
    options: Options,
    progress: impl Fn(Progress),
) -> Result<Report, ImapError> {
    let mut attempt = 0;
    loop {
        match sync_once(account, password, target.clone(), &cancel, &progress).await {
            Err(err) if err.retryable() && attempt < options.retries => {
                tokio::time::sleep(options.backoff * 2u32.pow(attempt.min(5))).await;
                attempt += 1;
            }
            other => return other,
        }
    }
}

/// Signs in and out once, to check credentials before a source is saved.
pub async fn check_sign_in(account: &Account, password: &str) -> Result<(), ImapError> {
    let mut session = connect(account, password).await?;
    let _ = session.logout().await;
    Ok(())
}

async fn sync_once(
    account: &Account,
    password: &str,
    target: Arc<dyn SyncTarget>,
    cancel: &AtomicBool,
    progress: &impl Fn(Progress),
) -> Result<Report, ImapError> {
    let mut session = connect(account, password).await?;

    let names: Vec<(String, Vec<Kind>)> = session
        .list(Some(""), Some("*"))
        .await?
        .map_ok(|name| {
            let kinds = name.attributes().iter().filter_map(kind).collect();
            (name.name().to_owned(), kinds)
        })
        .try_collect()
        .await?;
    let folders = choose_folders(&names);

    // First pass: what is new in each folder, so progress has a total.
    let mut plan = Vec::new();
    for folder in &folders {
        check(cancel)?;
        let mailbox = session.examine(folder).await?;
        let uid_validity = mailbox.uid_validity.unwrap_or(0);
        let highest = blocking(target.clone(), {
            let folder = folder.clone();
            move |t| t.resume(&folder, uid_validity)
        })
        .await?;
        let mut uids: Vec<u32> = if mailbox.exists == 0 {
            Vec::new()
        } else {
            session
                .uid_search(format!("UID {}:*", highest.saturating_add(1)))
                .await?
                .into_iter()
                .filter(|uid| *uid > highest)
                .collect()
        };
        uids.sort_unstable();
        plan.push((folder.clone(), uid_validity, uids));
    }

    let total = plan.iter().map(|(_, _, uids)| uids.len() as u64).sum();
    let mut fetched = 0;
    progress(Progress { fetched, total });

    for (folder, uid_validity, uids) in plan {
        session.examine(&folder).await?;
        for chunk in uids.chunks(BATCH) {
            check(cancel)?;
            let set = chunk
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let batch: Vec<Fetched> = session
                .uid_fetch(set, format!("(UID BODY.PEEK[]<0.{MAX_BYTES}>)"))
                .await?
                .try_filter_map(|fetch| async move {
                    Ok(fetch.uid.zip(fetch.body()).map(|(uid, raw)| Fetched {
                        uid,
                        raw: raw.to_vec(),
                    }))
                })
                .try_collect()
                .await?;
            // A FETCH cut off by a closed connection ends like a complete one:
            // async-imap stops the stream at end of input without an error.
            // A short batch is either expunged mail or a lost connection, and
            // a NOOP tells them apart before anything is committed.
            if batch.len() < chunk.len() {
                session.noop().await?;
            }
            let count = batch.len() as u64;
            blocking(target.clone(), {
                let folder = folder.clone();
                move |t| t.commit(&folder, uid_validity, batch)
            })
            .await?;
            fetched += count;
            progress(Progress { fetched, total });
        }
    }

    let _ = session.logout().await;
    Ok(Report { folders, fetched })
}

fn check(cancel: &AtomicBool) -> Result<(), ImapError> {
    if cancel.load(Ordering::Relaxed) {
        Err(ImapError::Cancelled)
    } else {
        Ok(())
    }
}

async fn blocking<T: Send + 'static>(
    target: Arc<dyn SyncTarget>,
    f: impl FnOnce(&dyn SyncTarget) -> Result<T, String> + Send + 'static,
) -> Result<T, ImapError> {
    tokio::task::spawn_blocking(move || f(target.as_ref()))
        .await
        .map_err(|err| ImapError::Target(err.to_string()))?
        .map_err(ImapError::Target)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    NoSelect,
    All,
    Skip,
}

fn kind(attribute: &NameAttribute<'_>) -> Option<Kind> {
    match attribute {
        NameAttribute::NoSelect => Some(Kind::NoSelect),
        NameAttribute::All => Some(Kind::All),
        NameAttribute::Sent
        | NameAttribute::Drafts
        | NameAttribute::Trash
        | NameAttribute::Junk => Some(Kind::Skip),
        _ => None,
    }
}

/// Folders that hold mail someone sent the user. Where the server marks an
/// `\All` folder (Gmail), that one alone holds everything once. Otherwise every
/// selectable folder except sent, drafts, trash and junk, recognised by
/// special-use attribute or, on servers without one, by common name.
fn choose_folders(names: &[(String, Vec<Kind>)]) -> Vec<String> {
    if let Some((name, _)) = names.iter().find(|(_, kinds)| kinds.contains(&Kind::All)) {
        return vec![name.clone()];
    }
    const SKIPPED: &[&str] = &[
        "sent",
        "sent items",
        "sent mail",
        "sent messages",
        "drafts",
        "draft",
        "trash",
        "deleted",
        "deleted items",
        "deleted messages",
        "bin",
        "junk",
        "junk email",
        "junk e-mail",
        "spam",
        "bulk mail",
        "outbox",
    ];
    names
        .iter()
        .filter(|(_, kinds)| !kinds.contains(&Kind::NoSelect) && !kinds.contains(&Kind::Skip))
        .filter(|(name, _)| {
            let leaf = name
                .rsplit(['/', '.'])
                .next()
                .unwrap_or(name)
                .to_lowercase();
            !SKIPPED.contains(&leaf.as_str())
        })
        .map(|(name, _)| name.clone())
        .collect()
}

async fn connect(account: &Account, password: &str) -> Result<Session, ImapError> {
    let tcp = TcpStream::connect((account.host.as_str(), account.port))
        .await
        .map_err(|source| ImapError::Connect {
            host: account.host.clone(),
            source,
        })?;
    let stream = match account.security {
        Security::Tls => Stream::Tls(Box::new(
            crate::tls::connect(&account.host, tcp)
                .await
                .map_err(ImapError::Tls)?,
        )),
        Security::PlainLoopback => {
            let loopback = tcp
                .peer_addr()
                .map(|a| a.ip().is_loopback())
                .unwrap_or(false);
            if !loopback {
                return Err(ImapError::PlainRefused);
            }
            Stream::Plain(tcp)
        }
    };
    let mut client = async_imap::Client::new(stream);
    client
        .read_response()
        .await
        .map_err(|source| ImapError::Connect {
            host: account.host.clone(),
            source,
        })?
        .ok_or_else(|| ImapError::Protocol("no greeting".into()))?;
    client
        .login(&account.username, password)
        .await
        .map_err(|(err, _)| match err {
            async_imap::error::Error::No(text) => ImapError::Auth(text),
            other => other.into(),
        })
}

#[derive(Debug)]
enum Stream {
    Tls(Box<TlsStream<TcpStream>>),
    Plain(TcpStream),
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Tls(s) => Pin::new(s.as_mut()).poll_read(cx, buf),
            Stream::Plain(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Stream::Tls(s) => Pin::new(s.as_mut()).poll_write(cx, buf),
            Stream::Plain(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Tls(s) => Pin::new(s.as_mut()).poll_flush(cx),
            Stream::Plain(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Tls(s) => Pin::new(s.as_mut()).poll_shutdown(cx),
            Stream::Plain(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[(&str, &[Kind])]) -> Vec<(String, Vec<Kind>)> {
        list.iter()
            .map(|(n, k)| (n.to_string(), k.to_vec()))
            .collect()
    }

    #[test]
    fn gmail_reads_all_mail_alone() {
        let list = names(&[
            ("INBOX", &[]),
            ("[Gmail]", &[Kind::NoSelect]),
            ("[Gmail]/All Mail", &[Kind::All]),
            ("[Gmail]/Sent Mail", &[Kind::Skip]),
            ("Receipts", &[]),
        ]);
        assert_eq!(choose_folders(&list), vec!["[Gmail]/All Mail"]);
    }

    #[test]
    fn special_use_folders_are_skipped() {
        let list = names(&[
            ("INBOX", &[]),
            ("Archive", &[]),
            ("Sent", &[Kind::Skip]),
            ("Spam", &[Kind::Skip]),
            ("Receipts", &[]),
        ]);
        assert_eq!(choose_folders(&list), vec!["INBOX", "Archive", "Receipts"]);
    }

    #[test]
    fn common_names_are_skipped_without_special_use() {
        let list = names(&[
            ("INBOX", &[]),
            ("INBOX.Sent Messages", &[]),
            ("Deleted Items", &[]),
            ("Junk Email", &[]),
            ("Bulk Mail", &[]),
            ("Newsletters", &[]),
        ]);
        assert_eq!(choose_folders(&list), vec!["INBOX", "Newsletters"]);
    }
}
