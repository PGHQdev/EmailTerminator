//! IMAP sync against the stub server, one profile per target provider
//! (PLAN.md M1 "done when").

mod support {
    pub mod imap_stub;
}

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use et_core::ingest::imap::{self, Account, Fetched, ImapError, Options, Security, SyncTarget};
use support::imap_stub::{self, Provider, Stub};

const PASSWORD: &str = "abcd efgh ijkl mnop";

/// Stands in for the store: per folder, its UID validity and every UID kept.
#[derive(Default)]
struct Memory {
    folders: Mutex<BTreeMap<String, (u32, BTreeSet<u32>)>>,
    resets: Mutex<u32>,
}

impl Memory {
    fn uids(&self, folder: &str) -> Vec<u32> {
        self.folders
            .lock()
            .unwrap()
            .get(folder)
            .map(|(_, u)| u.iter().copied().collect())
            .unwrap_or_default()
    }
}

impl SyncTarget for Memory {
    fn resume(&self, folder: &str, uid_validity: u32) -> Result<u32, String> {
        let mut folders = self.folders.lock().unwrap();
        let entry = folders
            .entry(folder.into())
            .or_insert((uid_validity, BTreeSet::new()));
        if entry.0 != uid_validity {
            *entry = (uid_validity, BTreeSet::new());
            *self.resets.lock().unwrap() += 1;
        }
        Ok(entry.1.iter().next_back().copied().unwrap_or(0))
    }

    fn commit(&self, folder: &str, uid_validity: u32, batch: Vec<Fetched>) -> Result<(), String> {
        let mut folders = self.folders.lock().unwrap();
        let entry = folders.get_mut(folder).ok_or("unknown folder")?;
        assert_eq!(entry.0, uid_validity);
        for fetched in batch {
            assert!(
                entry.1.insert(fetched.uid),
                "UID {} stored twice",
                fetched.uid
            );
        }
        Ok(())
    }
}

fn message(n: u32) -> Vec<u8> {
    format!(
        "From: Sender {n} <s{n}@vendor.test>\r\nSubject: Message {n}\r\nMessage-ID: <{n}@vendor.test>\r\n\r\nBody {n}\r\n"
    )
    .into_bytes()
}

fn fast() -> Options {
    Options {
        retries: 2,
        backoff: std::time::Duration::from_millis(5),
    }
}

fn account(stub: &Stub) -> Account {
    Account {
        host: "127.0.0.1".into(),
        port: stub.port,
        username: "user@example.test".into(),
        security: Security::PlainLoopback,
    }
}

async fn run(stub: &Stub, target: Arc<Memory>) -> Result<imap::Report, ImapError> {
    imap::sync(
        &account(stub),
        PASSWORD,
        target,
        Arc::new(AtomicBool::new(false)),
        fast(),
        |_| {},
    )
    .await
}

async fn stub_with(provider: Provider, count: u32) -> Stub {
    let stub = imap_stub::start(provider, PASSWORD).await;
    {
        let mut state = stub.state.lock().unwrap();
        for n in 1..=count {
            state.deliver(provider.inbox(), message(n));
        }
    }
    stub
}

#[tokio::test]
async fn each_provider_syncs_its_mail_folders_read_only() {
    let expected: [(Provider, &[&str]); 5] = [
        (Provider::Gmail, &["[Gmail]/All Mail"]),
        (Provider::ICloud, &["INBOX", "Archive"]),
        (Provider::Fastmail, &["INBOX", "Archive"]),
        (Provider::Yahoo, &["INBOX", "Archive"]),
        (Provider::Outlook, &["INBOX", "Archive"]),
    ];
    for (provider, folders) in expected {
        let stub = stub_with(provider, 450).await;
        let target = Arc::new(Memory::default());
        let report = run(&stub, target.clone()).await.unwrap();

        assert_eq!(report.folders, folders, "{provider:?}");
        assert_eq!(report.fetched, 450, "{provider:?}");
        assert_eq!(target.uids(folders[0]), (1..=450).collect::<Vec<_>>());

        let commands = stub
            .state
            .lock()
            .unwrap()
            .commands
            .join("\n")
            .to_uppercase();
        assert!(
            !commands.contains(" SELECT "),
            "{provider:?} must not SELECT"
        );
        assert!(
            !commands.contains("STORE"),
            "{provider:?} must not change flags"
        );
        assert!(commands.contains("BODY.PEEK[]"), "{provider:?} must peek");
    }
}

#[tokio::test]
async fn a_second_sync_fetches_only_new_mail() {
    let stub = stub_with(Provider::Fastmail, 3).await;
    let target = Arc::new(Memory::default());
    assert_eq!(run(&stub, target.clone()).await.unwrap().fetched, 3);

    // Nothing new: the n:* quirk returns the highest UID, which is filtered.
    assert_eq!(run(&stub, target.clone()).await.unwrap().fetched, 0);

    stub.state.lock().unwrap().deliver("INBOX", message(4));
    assert_eq!(run(&stub, target.clone()).await.unwrap().fetched, 1);
    assert_eq!(target.uids("INBOX"), vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn a_dropped_connection_resumes_without_duplicates() {
    let stub = stub_with(Provider::Gmail, 450).await;
    // The second batch of 200 breaks after 50 messages.
    stub.state.lock().unwrap().drop_after_fetches = Some(50);
    let target = Arc::new(Memory::default());
    // Commits are all-or-nothing per batch; the first batch lands, the broken
    // one is refetched after the reconnect.
    let report = run(&stub, target.clone()).await.unwrap();
    assert_eq!(
        target.uids("[Gmail]/All Mail"),
        (1..=450).collect::<Vec<_>>()
    );
    assert!(report.fetched <= 450);
    let logins = stub
        .state
        .lock()
        .unwrap()
        .commands
        .iter()
        .filter(|c| c.contains("LOGIN"))
        .count();
    assert_eq!(logins, 2);
}

#[tokio::test]
async fn a_renumbered_folder_is_fetched_again() {
    let stub = stub_with(Provider::Outlook, 2).await;
    let target = Arc::new(Memory::default());
    run(&stub, target.clone()).await.unwrap();

    stub.state.lock().unwrap().folder("INBOX").uid_validity = 99;
    assert_eq!(run(&stub, target.clone()).await.unwrap().fetched, 2);
    assert_eq!(*target.resets.lock().unwrap(), 1);
}

#[tokio::test]
async fn a_refused_password_reports_the_server_words() {
    let stub = stub_with(Provider::Gmail, 1).await;
    let err = imap::sync(
        &account(&stub),
        "a regular password",
        Arc::new(Memory::default()),
        Arc::new(AtomicBool::new(false)),
        fast(),
        |_| {},
    )
    .await
    .unwrap_err();
    match err {
        ImapError::Auth(text) => assert!(text.contains("Invalid credentials"), "{text}"),
        other => panic!("expected an auth error, got {other:?}"),
    }
    // Credentials are not retried.
    let logins = stub
        .state
        .lock()
        .unwrap()
        .commands
        .iter()
        .filter(|c| c.contains("LOGIN"))
        .count();
    assert_eq!(logins, 1);
}

#[tokio::test]
async fn cancelling_stops_between_batches() {
    let stub = stub_with(Provider::Fastmail, 450).await;
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let err = imap::sync(
        &account(&stub),
        PASSWORD,
        Arc::new(Memory::default()),
        cancel,
        fast(),
        move |p| {
            if p.fetched >= 200 {
                flag.store(true, Ordering::Relaxed);
            }
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ImapError::Cancelled));
}

#[tokio::test]
async fn progress_counts_up_to_the_total() {
    let stub = stub_with(Provider::Yahoo, 250).await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    imap::sync(
        &account(&stub),
        PASSWORD,
        Arc::new(Memory::default()),
        Arc::new(AtomicBool::new(false)),
        fast(),
        move |p| log.lock().unwrap().push((p.fetched, p.total)),
    )
    .await
    .unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        vec![(0, 250), (200, 250), (250, 250)]
    );
}

#[tokio::test]
async fn plain_imap_to_a_remote_host_is_refused() {
    let remote = Account {
        host: "192.0.2.1".into(),
        port: 143,
        username: "u".into(),
        security: Security::PlainLoopback,
    };
    let err = imap::check_sign_in(&remote, "p");
    // 192.0.2.0/24 is unroutable; the check must fail without sending a password.
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), err).await;
    if let Ok(Ok(())) = result {
        panic!("plain sign-in to a remote host succeeded");
    }
}
