//! A scripted IMAP server on loopback (PLAN.md 2.9). It models one mailbox per
//! provider profile, answers only the commands the client sends, records every
//! command, and can drop the connection mid-fetch or refuse the password.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[derive(Clone, Copy, Debug)]
pub enum Provider {
    Gmail,
    ICloud,
    Fastmail,
    Yahoo,
    Outlook,
}

pub struct Folder {
    pub name: String,
    pub attributes: String,
    pub uid_validity: u32,
    pub messages: BTreeMap<u32, Vec<u8>>,
}

pub struct State {
    pub folders: Vec<Folder>,
    pub password: String,
    /// Close the socket after this many FETCH responses, once.
    pub drop_after_fetches: Option<usize>,
    pub commands: Vec<String>,
    /// The words a refused LOGIN carries, per provider.
    pub auth_failure: String,
}

impl State {
    /// INBOX matches in any case (RFC 3501 5.1); other names are exact.
    pub fn folder(&mut self, name: &str) -> &mut Folder {
        let inbox = name.eq_ignore_ascii_case("inbox");
        self.folders
            .iter_mut()
            .find(|f| f.name == name || (inbox && f.name.eq_ignore_ascii_case("inbox")))
            .unwrap_or_else(|| panic!("no folder {name}"))
    }

    /// Adds a message to `folder` under the next UID.
    pub fn deliver(&mut self, folder: &str, raw: Vec<u8>) -> u32 {
        let folder = self.folder(folder);
        let uid = folder.messages.keys().next_back().copied().unwrap_or(0) + 1;
        folder.messages.insert(uid, raw);
        uid
    }
}

pub struct Stub {
    pub port: u16,
    pub state: Arc<Mutex<State>>,
}

fn folder(name: &str, attributes: &str) -> Folder {
    Folder {
        name: name.into(),
        attributes: attributes.into(),
        uid_validity: 1,
        messages: BTreeMap::new(),
    }
}

impl Provider {
    fn folders(self) -> Vec<Folder> {
        match self {
            Provider::Gmail => vec![
                folder("INBOX", r"\HasNoChildren"),
                folder("[Gmail]", r"\HasChildren \Noselect"),
                folder("[Gmail]/All Mail", r"\HasNoChildren \All"),
                folder("[Gmail]/Sent Mail", r"\HasNoChildren \Sent"),
                folder("[Gmail]/Spam", r"\HasNoChildren \Junk"),
                folder("[Gmail]/Trash", r"\HasNoChildren \Trash"),
            ],
            Provider::ICloud => vec![
                folder("INBOX", ""),
                folder("Sent Messages", ""),
                folder("Deleted Messages", ""),
                folder("Junk", ""),
                folder("Archive", ""),
            ],
            Provider::Fastmail => vec![
                folder("INBOX", r"\HasNoChildren"),
                folder("Archive", r"\HasNoChildren \Archive"),
                folder("Sent", r"\HasNoChildren \Sent"),
                folder("Spam", r"\HasNoChildren \Junk"),
                folder("Trash", r"\HasNoChildren \Trash"),
            ],
            Provider::Yahoo => vec![
                folder("Inbox", r"\HasNoChildren"),
                folder("Bulk Mail", r"\HasNoChildren"),
                folder("Draft", r"\HasNoChildren \Drafts"),
                folder("Sent", r"\HasNoChildren \Sent"),
                folder("Archive", r"\HasNoChildren \Archive"),
            ],
            Provider::Outlook => vec![
                folder("INBOX", r"\HasNoChildren"),
                folder("Sent Items", r"\HasNoChildren \Sent"),
                folder("Deleted Items", r"\HasNoChildren \Trash"),
                folder("Junk Email", r"\HasNoChildren \Junk"),
                folder("Archive", r"\HasNoChildren \Archive"),
            ],
        }
    }

    fn auth_failure(self) -> &'static str {
        match self {
            Provider::Gmail => "[AUTHENTICATIONFAILED] Invalid credentials (Failure)",
            Provider::ICloud => "[AUTHENTICATIONFAILED] Authentication failed.",
            Provider::Fastmail => "[AUTHENTICATIONFAILED] Authentication failed",
            Provider::Yahoo => "[AUTHENTICATIONFAILED] LOGIN Invalid credentials",
            Provider::Outlook => "LOGIN failed.",
        }
    }

    /// The folder that receives new mail.
    pub fn inbox(self) -> &'static str {
        match self {
            Provider::Gmail => "[Gmail]/All Mail",
            Provider::Yahoo => "Inbox",
            _ => "INBOX",
        }
    }
}

pub async fn start(provider: Provider, password: &str) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(Mutex::new(State {
        folders: provider.folders(),
        password: password.into(),
        drop_after_fetches: None,
        commands: Vec::new(),
        auth_failure: provider.auth_failure().into(),
    }));
    let shared = state.clone();
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(serve(socket, shared.clone()));
        }
    });
    Stub { port, state }
}

async fn serve(socket: tokio::net::TcpStream, state: Arc<Mutex<State>>) {
    let (read, mut write) = socket.into_split();
    let mut lines = BufReader::new(read);
    let _ = write
        .write_all(b"* OK [CAPABILITY IMAP4rev1] stub ready\r\n")
        .await;
    let mut selected: Option<String> = None;
    let mut line = String::new();
    loop {
        line.clear();
        if lines.read_line(&mut line).await.unwrap_or(0) == 0 {
            return;
        }
        let command = line.trim_end().to_owned();
        state.lock().unwrap().commands.push(command.clone());
        let (tag, rest) = command.split_once(' ').unwrap_or((&command, ""));
        let words = split_words(rest);
        let verb = words.first().map(|w| w.to_uppercase()).unwrap_or_default();
        let mut out: Vec<u8> = Vec::new();

        match verb.as_str() {
            "LOGIN" => {
                let st = state.lock().unwrap();
                if words.get(2).map(String::as_str) == Some(st.password.as_str()) {
                    out.extend(format!("{tag} OK LOGIN completed\r\n").bytes());
                } else {
                    out.extend(format!("{tag} NO {}\r\n", st.auth_failure).bytes());
                }
            }
            "LIST" => {
                let st = state.lock().unwrap();
                for f in &st.folders {
                    out.extend(
                        format!("* LIST ({}) \"/\" \"{}\"\r\n", f.attributes, f.name).bytes(),
                    );
                }
                out.extend(format!("{tag} OK LIST completed\r\n").bytes());
            }
            "EXAMINE" | "SELECT" => {
                let name = words.get(1).cloned().unwrap_or_default();
                let mut st = state.lock().unwrap();
                let f = st.folder(&name);
                let next = f.messages.keys().next_back().copied().unwrap_or(0) + 1;
                out.extend(format!("* {} EXISTS\r\n* 0 RECENT\r\n", f.messages.len()).bytes());
                out.extend(format!("* OK [UIDVALIDITY {}] UIDs valid\r\n", f.uid_validity).bytes());
                out.extend(format!("* OK [UIDNEXT {next}] Predicted next UID\r\n").bytes());
                out.extend(format!("{tag} OK [READ-ONLY] {verb} completed\r\n").bytes());
                selected = Some(name);
            }
            "UID" if words.get(1).map(|w| w.to_uppercase()) == Some("SEARCH".into()) => {
                let range = words.get(3).cloned().unwrap_or_default();
                let from: u32 = range.split(':').next().unwrap_or("1").parse().unwrap_or(1);
                let mut st = state.lock().unwrap();
                let f = st.folder(selected.as_deref().unwrap());
                let mut hits: Vec<u32> =
                    f.messages.keys().copied().filter(|u| *u >= from).collect();
                // RFC 3501: n:* always includes the highest UID, even below n.
                if hits.is_empty() {
                    hits.extend(f.messages.keys().next_back());
                }
                let list: Vec<String> = hits.iter().map(u32::to_string).collect();
                out.extend(
                    format!(
                        "* SEARCH {}\r\n{tag} OK SEARCH completed\r\n",
                        list.join(" ")
                    )
                    .bytes(),
                );
            }
            "UID" if words.get(1).map(|w| w.to_uppercase()) == Some("FETCH".into()) => {
                let set = words.get(2).cloned().unwrap_or_default();
                let (responses, drop_after) = {
                    let mut st = state.lock().unwrap();
                    let name = selected.clone().unwrap();
                    let uids: Vec<u32> = set.split(',').filter_map(|u| u.parse().ok()).collect();
                    let drop_after = st.drop_after_fetches;
                    let f = st.folder(&name);
                    let mut responses = Vec::new();
                    for (seq, (uid, raw)) in f.messages.iter().enumerate() {
                        if uids.contains(uid) {
                            let mut r = format!(
                                "* {} FETCH (UID {uid} BODY[]<0> {{{}}}\r\n",
                                seq + 1,
                                raw.len()
                            )
                            .into_bytes();
                            r.extend(raw);
                            r.extend(b")\r\n");
                            responses.push(r);
                        }
                    }
                    let drop_after = drop_after.filter(|limit| *limit < responses.len());
                    if drop_after.is_some() {
                        st.drop_after_fetches = None;
                    }
                    (responses, drop_after)
                };
                if let Some(limit) = drop_after {
                    let partial: Vec<u8> = responses.into_iter().take(limit).flatten().collect();
                    let _ = write.write_all(&partial).await;
                    return;
                }
                for r in responses {
                    out.extend(r);
                }
                out.extend(format!("{tag} OK FETCH completed\r\n").bytes());
            }
            "LOGOUT" => {
                let _ = write
                    .write_all(format!("* BYE\r\n{tag} OK LOGOUT completed\r\n").as_bytes())
                    .await;
                return;
            }
            _ => out.extend(format!("{tag} BAD unsupported\r\n").bytes()),
        }
        if write.write_all(&out).await.is_err() {
            return;
        }
    }
}

/// Splits an IMAP command into words, honouring double quotes.
fn split_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => quoted = !quoted,
            '\\' if quoted => current.extend(chars.next()),
            ' ' if !quoted => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}
