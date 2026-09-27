//! Maildir import (PLAN.md 1.6 rung 2, M5): one file per message under a
//! folder's `new/` and `cur/`.
//!
//! The chosen directory is a Maildir itself, or holds Maildir folders:
//! Maildir++ `.Name` folders or plain nested ones. Sent, draft, trash and junk
//! folders are skipped by name, as over IMAP. A message's locator is its
//! folder and the unique part of its file name; the part after `:` holds
//! flags that change when the message is read, so it is left out.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::imap::skipped_name;
use super::{MAX_BYTES, Message};

/// How deep folders are looked for below the chosen directory.
const DEPTH: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum MaildirError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("this folder holds no Maildir")]
    NotMaildir,
}

pub struct Maildir {
    files: std::vec::IntoIter<(String, PathBuf)>,
    total: u64,
}

/// Lists every message file below `root`.
pub fn open(root: &Path) -> Result<Maildir, MaildirError> {
    let mut folders = Vec::new();
    walk(root, "", 0, &mut folders)?;
    if folders.is_empty() {
        return Err(MaildirError::NotMaildir);
    }
    let mut files = Vec::new();
    for (folder, dir) in folders {
        if skipped_name(folder.trim_start_matches('.')) {
            continue;
        }
        for sub in ["new", "cur"] {
            let Ok(entries) = fs::read_dir(dir.join(sub)) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || !entry.file_type().is_ok_and(|t| t.is_file()) {
                    continue;
                }
                files.push((format!("{folder}/{}", unique(&name)), entry.path()));
            }
        }
    }
    files.sort();
    let total = files.len() as u64;
    Ok(Maildir {
        files: files.into_iter(),
        total,
    })
}

impl Maildir {
    pub fn total(&self) -> u64 {
        self.total
    }
}

impl Iterator for Maildir {
    type Item = Message;

    /// The next readable message. A file that vanished or cannot be read is
    /// passed over: one bad file does not stop an import.
    fn next(&mut self) -> Option<Message> {
        loop {
            let (locator, path) = self.files.next()?;
            if let Ok(raw) = read_capped(&path) {
                return Some(Message { locator, raw });
            }
        }
    }
}

/// The message a locator names, or `None` once it is gone.
pub fn read_one(root: &Path, locator: &str) -> Result<Option<Vec<u8>>, MaildirError> {
    let Some((folder, wanted)) = locator.rsplit_once('/') else {
        return Ok(None);
    };
    let dir = if folder.is_empty() {
        root.to_path_buf()
    } else {
        root.join(folder)
    };
    for sub in ["cur", "new"] {
        let Ok(entries) = fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for entry in entries.flatten() {
            if unique(&entry.file_name().to_string_lossy()) == wanted {
                return Ok(Some(read_capped(&entry.path())?));
            }
        }
    }
    Ok(None)
}

/// Collects `(folder, directory)` for every Maildir at or below `dir`.
fn walk(
    dir: &Path,
    folder: &str,
    depth: usize,
    found: &mut Vec<(String, PathBuf)>,
) -> io::Result<()> {
    if dir.join("cur").is_dir() || dir.join("new").is_dir() {
        found.push((folder.to_owned(), dir.to_path_buf()));
    }
    if depth == DEPTH {
        return Ok(());
    }
    for entry in fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches!(name.as_str(), "cur" | "new" | "tmp")
            || !entry.file_type().is_ok_and(|t| t.is_dir())
        {
            continue;
        }
        let sub = if folder.is_empty() {
            name
        } else {
            format!("{folder}/{name}")
        };
        // An unreadable subfolder is passed over, as an unreadable file is.
        let _ = walk(&entry.path(), &sub, depth + 1, found);
    }
    Ok(())
}

/// A file name without its flags: `1712.M1P2.host:2,S` is `1712.M1P2.host`.
/// Windows tools write `!` in place of `:`.
fn unique(name: &str) -> &str {
    name.split([':', '!']).next().unwrap_or(name)
}

fn read_capped(path: &Path) -> io::Result<Vec<u8>> {
    let mut raw = Vec::new();
    fs::File::open(path)?
        .take(MAX_BYTES as u64)
        .read_to_end(&mut raw)?;
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows forbids `:` in a file name; its Maildir tools write `!`.
    const INFO: &str = if cfg!(windows) { "!" } else { ":" };

    fn flagged(name: &str, flags: &str) -> String {
        format!("{name}{INFO}{flags}")
    }

    fn put(root: &Path, path: &str, body: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn folders_are_found_and_sent_mail_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(
            root,
            &flagged("cur/1.a.host", "2,S"),
            "Subject: read\n\nx\n",
        );
        put(root, "new/2.b.host", "Subject: new\n\nx\n");
        fs::create_dir_all(root.join("tmp")).unwrap();
        put(
            root,
            &flagged(".Receipts/cur/3.c.host", "2,"),
            "Subject: receipt\n\nx\n",
        );
        put(
            root,
            &flagged(".Sent/cur/4.d.host", "2,S"),
            "Subject: mine\n\nx\n",
        );
        put(root, "Archive/2025/new/5.e.host", "Subject: nested\n\nx\n");
        put(root, "cur/.hidden", "not mail");

        let maildir = open(root).unwrap();
        assert_eq!(maildir.total(), 4);
        let locators: Vec<String> = maildir.map(|m| m.locator).collect();
        assert_eq!(
            locators,
            [
                ".Receipts/3.c.host",
                "/1.a.host",
                "/2.b.host",
                "Archive/2025/5.e.host"
            ]
        );
    }

    #[test]
    fn a_locator_survives_a_flag_change() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(root, ".Receipts/new/3.c.host", "Subject: receipt\n\nx\n");
        let locator = open(root).unwrap().next().unwrap().locator;

        fs::create_dir_all(root.join(".Receipts/cur")).unwrap();
        fs::rename(
            root.join(".Receipts/new/3.c.host"),
            root.join(flagged(".Receipts/cur/3.c.host", "2,S")),
        )
        .unwrap();
        assert_eq!(
            read_one(root, &locator).unwrap().unwrap(),
            b"Subject: receipt\n\nx\n"
        );
        fs::remove_file(root.join(flagged(".Receipts/cur/3.c.host", "2,S"))).unwrap();
        assert_eq!(read_one(root, &locator).unwrap(), None);
    }

    #[test]
    fn a_folder_without_a_maildir_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "notes.txt", "hello");
        assert!(matches!(open(dir.path()), Err(MaildirError::NotMaildir)));
    }
}
