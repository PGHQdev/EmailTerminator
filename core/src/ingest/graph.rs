//! Outlook.com mail over Microsoft Graph with the read-only `Mail.Read`
//! permission (PLAN.md 1.6).
//!
//! Each folder is read through its delta query. A first pass collects the ids
//! of new messages and the delta link that ends the folder's pass; the second
//! fetches each message's MIME and commits it in batches. The delta link is
//! stored only once the folder is done, so an interrupted scan walks the same
//! changes again and skips the messages it already stored.
//!
//! Microsoft allows 4 requests at a time per mailbox and answers 429 with a
//! `Retry-After` when an app reads too fast; the reader waits as told.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::{StreamExt, TryStreamExt};
use serde::Deserialize;

use super::outlook::encode;
use super::{MAX_BYTES, Message};

const GRAPH: &str = "https://graph.microsoft.com/v1.0";
/// Messages per commit.
const BATCH: usize = 50;
/// Microsoft's limit of concurrent requests per mailbox.
const PARALLEL: usize = 4;
/// Waits for a throttled or failed request before giving up.
const RETRIES: u32 = 6;
/// Folders that hold no mail someone sent the user, by well-known name.
const SKIPPED: &[&str] = &[
    "sentitems",
    "deleteditems",
    "junkemail",
    "drafts",
    "outbox",
    "conversationhistory",
];

/// Where fetched messages go. Blocking: the store has one writer thread.
pub trait GraphTarget: Send + Sync + 'static {
    /// The delta link the folder's last finished pass ended with.
    fn delta_link(&self, folder: &str) -> Result<Option<String>, String>;
    /// Which of `ids` the folder already stores.
    fn stored(&self, folder: &str, ids: Vec<String>) -> Result<HashSet<String>, String>;
    fn commit(&self, folder: &str, batch: Vec<Message>) -> Result<(), String>;
    /// Records that the folder's pass is done; the next one starts at `link`.
    fn finish(&self, folder: &str, link: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub fetched: u64,
    pub total: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub folders: usize,
    pub fetched: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// 401: the access token expired or was revoked. A fresh one may work.
    #[error("sign-in expired: {0}")]
    Expired(String),
    /// 403: the mailbox refused the app. S16 shows Microsoft's words.
    #[error("access refused: {0}")]
    Refused(String),
    #[error("cannot reach Microsoft Graph: {0}")]
    Connect(String),
    /// 404: the message or folder is gone.
    #[error("not found")]
    NotFound,
    /// 410: a delta link Microsoft no longer knows.
    #[error("the folder's change link expired")]
    Stale,
    #[error("Microsoft Graph: {0}")]
    Api(String),
    #[error("storing messages: {0}")]
    Target(String),
    #[error("cancelled")]
    Cancelled,
}

struct Graph {
    http: reqwest::Client,
    token: String,
    /// [`GRAPH`], or a loopback stub in tests.
    base: String,
}

impl Graph {
    fn new(token: &str) -> Result<Self, GraphError> {
        Ok(Self {
            http: crate::tls::http_client().map_err(GraphError::Connect)?,
            token: token.to_owned(),
            base: GRAPH.to_owned(),
        })
    }

    /// A GET that waits out throttling and transient failures. The token is
    /// sent to Graph alone: a next or delta link elsewhere is refused.
    async fn get(&self, url: &str) -> Result<reqwest::Response, GraphError> {
        if !url.starts_with(&format!("{}/", self.base)) {
            return Err(GraphError::Api(format!("a link outside Graph: {url}")));
        }
        let mut attempt = 0;
        loop {
            let sent = self
                .http
                .get(url)
                .bearer_auth(&self.token)
                .header("Prefer", "odata.maxpagesize=500")
                .send()
                .await;
            let wait = match sent {
                Ok(r) if r.status().is_success() => return Ok(r),
                Ok(r) if matches!(r.status().as_u16(), 429 | 500 | 502 | 503 | 504) => {
                    retry_after(&r).unwrap_or(Duration::from_secs(2u64.pow(attempt + 1)))
                }
                Ok(r) => {
                    let status = r.status().as_u16();
                    let words = words(r).await;
                    return Err(match status {
                        401 => GraphError::Expired(words),
                        403 => GraphError::Refused(words),
                        404 => GraphError::NotFound,
                        410 => GraphError::Stale,
                        _ => GraphError::Api(format!("HTTP {status}: {words}")),
                    });
                }
                Err(err) if attempt >= RETRIES => return Err(GraphError::Connect(err.to_string())),
                Err(_) => Duration::from_secs(2u64.pow(attempt + 1)),
            };
            if attempt >= RETRIES {
                return Err(GraphError::Api(
                    "Microsoft kept asking the app to wait".into(),
                ));
            }
            tokio::time::sleep(wait.min(Duration::from_secs(120))).await;
            attempt += 1;
        }
    }

    async fn json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T, GraphError> {
        let body = self
            .get(url)
            .await?
            .bytes()
            .await
            .map_err(|e| GraphError::Connect(e.to_string()))?;
        serde_json::from_slice(&body).map_err(|e| GraphError::Api(e.to_string()))
    }

    /// Every folder worth reading, by id: all but the well-known ones that
    /// hold sent, deleted, junk and draft mail.
    async fn folders(&self) -> Result<Vec<String>, GraphError> {
        #[derive(Deserialize)]
        struct Id {
            id: String,
        }
        let mut skipped = HashSet::new();
        for name in SKIPPED {
            match self
                .json::<Id>(&format!("{}/me/mailFolders/{name}?$select=id", self.base))
                .await
            {
                Ok(folder) => {
                    skipped.insert(folder.id);
                }
                Err(GraphError::NotFound) => {}
                Err(err) => return Err(err),
            }
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Folder {
            id: String,
            child_folder_count: Option<u32>,
        }
        let mut found = Vec::new();
        let mut queue = vec![format!(
            "{}/me/mailFolders?$select=id,childFolderCount&$top=250",
            self.base
        )];
        while let Some(url) = queue.pop() {
            let page: Page<Folder> = self.json(&url).await?;
            queue.extend(page.next);
            for folder in page.value {
                if skipped.contains(&folder.id) {
                    continue;
                }
                if folder.child_folder_count.unwrap_or(0) > 0 {
                    queue.push(format!(
                        "{}/me/mailFolders/{}/childFolders?$select=id,childFolderCount&$top=250",
                        self.base,
                        encode(&folder.id)
                    ));
                }
                found.push(folder.id);
            }
        }
        Ok(found)
    }

    /// The ids a folder's delta query reports since `link`, and the delta
    /// link that ends this pass. A stale link starts the folder over.
    async fn changes(
        &self,
        folder: &str,
        link: Option<String>,
    ) -> Result<(Vec<String>, String), GraphError> {
        #[derive(Deserialize)]
        struct Change {
            id: String,
            #[serde(rename = "@removed")]
            removed: Option<serde_json::Value>,
        }
        let first = format!(
            "{}/me/mailFolders/{}/messages/delta?$select=id",
            self.base,
            encode(folder)
        );
        let mut url = link.clone().unwrap_or_else(|| first.clone());
        let mut ids = Vec::new();
        loop {
            let page: Page<Change> = match self.json(&url).await {
                Err(GraphError::Stale) if url != first => {
                    ids.clear();
                    url = first.clone();
                    continue;
                }
                other => other?,
            };
            ids.extend(
                page.value
                    .into_iter()
                    .filter(|c| c.removed.is_none())
                    .map(|c| c.id),
            );
            match (page.next, page.delta) {
                (Some(next), _) => url = next,
                (None, Some(delta)) => return Ok((ids, delta)),
                (None, None) => return Err(GraphError::Api("a delta page without a link".into())),
            }
        }
    }

    /// A message's MIME, up to [`MAX_BYTES`]; the rest is never downloaded.
    async fn mime(&self, id: &str) -> Result<Vec<u8>, GraphError> {
        let mut response = self
            .get(&format!("{}/me/messages/{}/$value", self.base, encode(id)))
            .await?;
        let mut raw = Vec::new();
        while raw.len() < MAX_BYTES {
            match response.chunk().await {
                Ok(Some(chunk)) => raw.extend_from_slice(&chunk),
                Ok(None) => break,
                Err(err) => return Err(GraphError::Connect(err.to_string())),
            }
        }
        raw.truncate(MAX_BYTES);
        Ok(raw)
    }
}

#[derive(Deserialize)]
struct Page<T> {
    value: Vec<T>,
    #[serde(rename = "@odata.nextLink")]
    next: Option<String>,
    #[serde(rename = "@odata.deltaLink")]
    delta: Option<String>,
}

fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    let seconds = response
        .headers()
        .get("Retry-After")?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(seconds))
}

/// Microsoft's own words from an error answer.
async fn words(response: reqwest::Response) -> String {
    #[derive(Deserialize)]
    struct Answer {
        error: Detail,
    }
    #[derive(Deserialize)]
    struct Detail {
        message: String,
    }
    let status = response.status();
    match response.bytes().await {
        Ok(body) => serde_json::from_slice::<Answer>(&body)
            .map(|a| a.error.message)
            .unwrap_or_else(|_| status.to_string()),
        Err(_) => status.to_string(),
    }
}

/// Reads the inbox's id once, to check a new token before a source is saved.
pub async fn check_sign_in(token: &str) -> Result<(), GraphError> {
    let graph = Graph::new(token)?;
    graph
        .get(&format!("{}/me/mailFolders/inbox?$select=id", graph.base))
        .await
        .map(drop)
}

/// Syncs every folder worth reading.
pub async fn sync(
    token: &str,
    target: Arc<dyn GraphTarget>,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(Progress),
) -> Result<Report, GraphError> {
    run(&Graph::new(token)?, target, cancel, progress).await
}

async fn run(
    graph: &Graph,
    target: Arc<dyn GraphTarget>,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(Progress),
) -> Result<Report, GraphError> {
    let folders = graph.folders().await?;

    let mut plan = Vec::new();
    for folder in folders {
        check(&cancel)?;
        let link = blocking(target.clone(), {
            let folder = folder.clone();
            move |t| t.delta_link(&folder)
        })
        .await?;
        let (ids, delta) = graph.changes(&folder, link).await?;
        let stored = blocking(target.clone(), {
            let (folder, ids) = (folder.clone(), ids.clone());
            move |t| t.stored(&folder, ids)
        })
        .await?;
        let mut seen = HashSet::new();
        let new: Vec<String> = ids
            .into_iter()
            .filter(|id| !stored.contains(id) && seen.insert(id.clone()))
            .collect();
        plan.push((folder, new, delta));
    }

    let total = plan.iter().map(|(_, ids, _)| ids.len() as u64).sum();
    let mut fetched = 0;
    progress(Progress { fetched, total });

    let folders = plan.len();
    for (folder, ids, delta) in plan {
        for chunk in ids.chunks(BATCH) {
            check(&cancel)?;
            // Owned ids: a borrowed item makes the future's lifetime too
            // general for a Tauri command to prove it `Send`.
            let batch: Vec<Message> = futures::stream::iter(chunk.to_vec())
                .map(|id| async move {
                    match graph.mime(&id).await {
                        Ok(raw) => Ok(Some(Message { locator: id, raw })),
                        // Deleted between the delta query and the fetch.
                        Err(GraphError::NotFound) => Ok(None),
                        Err(err) => Err(err),
                    }
                })
                .buffer_unordered(PARALLEL)
                .try_filter_map(|f| async move { Ok(f) })
                .try_collect()
                .await?;
            blocking(target.clone(), {
                let folder = folder.clone();
                move |t| t.commit(&folder, batch)
            })
            .await?;
            fetched += chunk.len() as u64;
            progress(Progress { fetched, total });
        }
        blocking(target.clone(), {
            let folder = folder.clone();
            move |t| t.finish(&folder, &delta)
        })
        .await?;
    }
    Ok(Report { folders, fetched })
}

/// One message, for an evidence link (S14), or `None` once it is gone.
pub async fn fetch_one(token: &str, id: &str) -> Result<Option<Vec<u8>>, GraphError> {
    match Graph::new(token)?.mime(id).await {
        Ok(raw) => Ok(Some(raw)),
        Err(GraphError::NotFound) => Ok(None),
        Err(err) => Err(err),
    }
}

fn check(cancel: &AtomicBool) -> Result<(), GraphError> {
    if cancel.load(Ordering::Relaxed) {
        Err(GraphError::Cancelled)
    } else {
        Ok(())
    }
}

async fn blocking<T: Send + 'static>(
    target: Arc<dyn GraphTarget>,
    f: impl FnOnce(&dyn GraphTarget) -> Result<T, String> + Send + 'static,
) -> Result<T, GraphError> {
    tokio::task::spawn_blocking(move || f(target.as_ref()))
        .await
        .map_err(|err| GraphError::Target(err.to_string()))?
        .map_err(GraphError::Target)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    type Answer = (u16, String);

    /// A loopback stand-in for Graph: each path answers from a script, and a
    /// scripted answer list is used up one request at a time.
    struct Stub {
        base: String,
        routes: Arc<Mutex<BTreeMap<String, Vec<Answer>>>>,
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl Stub {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}/v1.0", listener.local_addr().unwrap());
            let routes: Arc<Mutex<BTreeMap<String, Vec<Answer>>>> = Arc::default();
            let seen: Arc<Mutex<Vec<String>>> = Arc::default();
            let (r, s) = (routes.clone(), seen.clone());
            tokio::spawn(async move {
                while let Ok((mut socket, _)) = listener.accept().await {
                    let (routes, seen) = (r.clone(), s.clone());
                    tokio::spawn(async move {
                        let mut buf = vec![0u8; 8192];
                        let n = socket.read(&mut buf).await.unwrap_or(0);
                        let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                        let path = request.split(' ').nth(1).unwrap_or("").to_owned();
                        let bearer = request.contains("authorization: Bearer good");
                        seen.lock().unwrap().push(path.clone());
                        let (status, body) = if !bearer {
                            (401, r#"{"error":{"message":"Token expired."}}"#.to_owned())
                        } else {
                            let mut routes = routes.lock().unwrap();
                            match routes.get_mut(&path) {
                                Some(list) if list.len() > 1 => list.remove(0),
                                Some(list) => list[0].clone(),
                                None => (404, r#"{"error":{"message":"Not found."}}"#.into()),
                            }
                        };
                        let answer = format!(
                            "HTTP/1.1 {status} X\r\nRetry-After: 0\r\nContent-Length: {}\r\n\
                             Connection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = socket.write_all(answer.as_bytes()).await;
                    });
                }
            });
            Self { base, routes, seen }
        }

        fn route(&self, path: &str, answers: &[(u16, &str)]) {
            self.routes.lock().unwrap().insert(
                format!("/v1.0{path}"),
                answers
                    .iter()
                    .map(|(s, b)| (*s, b.replace("{base}", &self.base)))
                    .collect(),
            );
        }

        fn graph(&self, token: &str) -> Graph {
            Graph {
                // Plain HTTP to loopback; the TLS setup is still required.
                http: reqwest::Client::builder()
                    .tls_backend_preconfigured(crate::tls::config().unwrap())
                    .build()
                    .unwrap(),
                token: token.into(),
                base: self.base.clone(),
            }
        }
    }

    #[derive(Default)]
    struct Memory {
        links: Mutex<BTreeMap<String, String>>,
        stored: Mutex<BTreeMap<String, Vec<u8>>>,
    }

    impl GraphTarget for Memory {
        fn delta_link(&self, folder: &str) -> Result<Option<String>, String> {
            Ok(self.links.lock().unwrap().get(folder).cloned())
        }
        fn stored(&self, _: &str, ids: Vec<String>) -> Result<HashSet<String>, String> {
            let stored = self.stored.lock().unwrap();
            Ok(ids
                .into_iter()
                .filter(|id| stored.contains_key(id))
                .collect())
        }
        fn commit(&self, _: &str, batch: Vec<Message>) -> Result<(), String> {
            let mut stored = self.stored.lock().unwrap();
            for m in batch {
                stored.insert(m.locator, m.raw);
            }
            Ok(())
        }
        fn finish(&self, folder: &str, link: &str) -> Result<(), String> {
            self.links
                .lock()
                .unwrap()
                .insert(folder.into(), link.into());
            Ok(())
        }
    }

    fn mailbox(stub: &Stub) {
        stub.route(
            "/me/mailFolders/sentitems?$select=id",
            &[(200, r#"{"id":"S"}"#)],
        );
        stub.route(
            "/me/mailFolders?$select=id,childFolderCount&$top=250",
            &[(
                200,
                r#"{"value":[{"id":"I","childFolderCount":1},{"id":"S","childFolderCount":0}]}"#,
            )],
        );
        stub.route(
            "/me/mailFolders/I/childFolders?$select=id,childFolderCount&$top=250",
            &[(200, r#"{"value":[{"id":"C","childFolderCount":0}]}"#)],
        );
        stub.route(
            "/me/mailFolders/I/messages/delta?$select=id",
            &[(
                200,
                r#"{"value":[{"id":"m1"},{"id":"gone","@removed":{"reason":"deleted"}}],
                    "@odata.nextLink":"{base}/me/mailFolders/I/messages/delta?$skiptoken=2"}"#,
            )],
        );
        stub.route(
            "/me/mailFolders/I/messages/delta?$skiptoken=2",
            &[(
                200,
                r#"{"value":[{"id":"m2"},{"id":"m1"}],
                    "@odata.deltaLink":"{base}/me/mailFolders/I/messages/delta?$deltatoken=a"}"#,
            )],
        );
        stub.route(
            "/me/mailFolders/C/messages/delta?$select=id",
            &[(
                200,
                r#"{"value":[{"id":"m3"}],
                    "@odata.deltaLink":"{base}/me/mailFolders/C/messages/delta?$deltatoken=b"}"#,
            )],
        );
        stub.route(
            "/me/messages/m1/$value",
            &[(200, "From: a@x.test\r\n\r\none")],
        );
        // Throttled once, then answered.
        stub.route(
            "/me/messages/m2/$value",
            &[(429, ""), (200, "From: b@x.test\r\n\r\ntwo")],
        );
        stub.route(
            "/me/messages/m3/$value",
            &[(200, "From: c@x.test\r\n\r\nthree")],
        );
    }

    async fn sync(stub: &Stub, target: Arc<Memory>, token: &str) -> Result<Report, GraphError> {
        run(
            &stub.graph(token),
            target,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await
    }

    #[tokio::test]
    async fn every_folder_but_the_skipped_ones_is_read_once() {
        let stub = Stub::start().await;
        mailbox(&stub);
        let target = Arc::new(Memory::default());

        let report = sync(&stub, target.clone(), "good").await.unwrap();
        assert_eq!(
            report,
            Report {
                folders: 2,
                fetched: 3
            }
        );
        assert_eq!(
            target.stored.lock().unwrap().keys().collect::<Vec<_>>(),
            ["m1", "m2", "m3"]
        );
        assert!(!stub.seen.lock().unwrap().iter().any(|p| p.contains("/S/")));
        assert!(target.links.lock().unwrap()["I"].ends_with("$deltatoken=a"));

        // The next pass starts at the delta link and fetches only what is new.
        stub.route(
            "/me/mailFolders/I/messages/delta?$deltatoken=a",
            &[(
                200,
                r#"{"value":[{"id":"m1"},{"id":"m4"}],
                    "@odata.deltaLink":"{base}/me/mailFolders/I/messages/delta?$deltatoken=c"}"#,
            )],
        );
        stub.route(
            "/me/mailFolders/C/messages/delta?$deltatoken=b",
            &[(200, r#"{"value":[],"@odata.deltaLink":"{base}/me/mailFolders/C/messages/delta?$deltatoken=b"}"#)],
        );
        stub.route(
            "/me/messages/m4/$value",
            &[(200, "From: d@x.test\r\n\r\nfour")],
        );
        stub.seen.lock().unwrap().clear();
        let report = sync(&stub, target.clone(), "good").await.unwrap();
        assert_eq!(report.fetched, 1);
        let fetched: Vec<String> = stub
            .seen
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.ends_with("$value"))
            .cloned()
            .collect();
        assert_eq!(fetched, ["/v1.0/me/messages/m4/$value"]);
    }

    #[tokio::test]
    async fn an_expired_token_and_a_foreign_link_stop_the_sync() {
        let stub = Stub::start().await;
        mailbox(&stub);
        assert!(matches!(
            sync(&stub, Arc::new(Memory::default()), "old").await,
            Err(GraphError::Expired(words)) if words == "Token expired."
        ));

        stub.route(
            "/me/mailFolders/C/messages/delta?$select=id",
            &[(
                200,
                r#"{"value":[],"@odata.nextLink":"https://elsewhere.test/steal"}"#,
            )],
        );
        assert!(matches!(
            sync(&stub, Arc::new(Memory::default()), "good").await,
            Err(GraphError::Api(words)) if words.contains("outside Graph")
        ));
    }

    #[tokio::test]
    async fn a_gone_message_is_none() {
        let stub = Stub::start().await;
        mailbox(&stub);
        let graph = stub.graph("good");
        assert_eq!(
            graph.mime("m1").await.unwrap(),
            b"From: a@x.test\r\n\r\none"
        );
        assert!(matches!(
            graph.mime("nope").await,
            Err(GraphError::NotFound)
        ));
    }
}
