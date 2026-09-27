//! mbox import (PLAN.md 1.6 rung 2, M5): a plain or gzipped file, read as a
//! stream with flat memory.
//!
//! A message starts at a `From ` line at the top of the file or after a blank
//! line, which covers `mboxo`, `mboxrd` and Google Takeout. Body lines that a
//! writer escaped as `>From ` (or `>>From ` in `mboxrd`) lose one `>`. A
//! message's locator is the byte offset of its `From ` line in the
//! decompressed stream, so an evidence link reads it again from there.
//!
//! Takeout carries Gmail's labels in `X-Gmail-Labels`. Mail labelled spam,
//! trash, draft, chat or sent is skipped, as the IMAP path skips those
//! folders.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use flate2::read::MultiGzDecoder;

use super::{MAX_BYTES, Message};

/// Gmail's system labels for mail nobody sent the user.
const SKIPPED_LABELS: &[&str] = &["spam", "trash", "draft", "drafts", "chat", "sent"];

#[derive(Debug, thiserror::Error)]
pub enum MboxError {
    #[error("{0}")]
    Io(#[from] io::Error),
    /// The first line is not a `From ` line.
    #[error("this file is not an mbox")]
    NotMbox,
    /// Reading stopped partway: a truncated or damaged file. Everything read
    /// before `message` is intact.
    #[error("stopped at message {message}: {reason}")]
    Stopped { message: u64, reason: String },
}

pub struct Mbox {
    lines: Lines,
    /// The offset of the `From ` line of the message being read.
    start: u64,
    /// Messages read so far, skipped ones included.
    pub read: u64,
    done: bool,
}

/// Opens `path`, gzipped or not, and checks that it starts like an mbox.
pub fn open(path: &Path) -> Result<Mbox, MboxError> {
    let mut lines = Lines::new(reader(path)?);
    match lines.next_line() {
        Ok(Some(first)) if first.starts_with(b"From ") => Ok(Mbox {
            lines,
            start: 0,
            read: 0,
            done: false,
        }),
        Ok(_) => Err(MboxError::NotMbox),
        Err(err) => Err(err.into()),
    }
}

/// How many messages the file holds, skipped ones included: the total for
/// progress.
pub fn count(path: &Path) -> Result<u64, MboxError> {
    let mut mbox = open(path)?;
    let mut n = 1;
    let mut blank = false;
    loop {
        match mbox.lines.next_line() {
            Ok(Some(line)) => {
                if blank && line.starts_with(b"From ") {
                    n += 1;
                }
                blank = is_blank(line);
            }
            Ok(None) => return Ok(n),
            Err(err) => {
                return Err(MboxError::Stopped {
                    message: n,
                    reason: err.to_string(),
                });
            }
        }
    }
}

/// The message whose `From ` line is at `offset`, or `None` when the file is
/// gone or no longer has one there.
pub fn read_at(path: &Path, offset: u64) -> Result<Option<Vec<u8>>, MboxError> {
    let mut mbox = match open(path) {
        Ok(mbox) => mbox,
        Err(MboxError::Io(err)) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(MboxError::NotMbox) => return Ok(None),
        Err(err) => return Err(err),
    };
    // A gzip stream cannot seek; reading forward is the only way there.
    while let Some(message) = mbox.next_any() {
        let (start, raw) = message?;
        if start == offset {
            return Ok(Some(raw));
        }
        if start > offset {
            break;
        }
    }
    Ok(None)
}

impl Mbox {
    /// The next message, skipped labels included, with its offset.
    fn next_any(&mut self) -> Option<Result<(u64, Vec<u8>), MboxError>> {
        if self.done {
            return None;
        }
        let start = self.start;
        let mut raw = Vec::new();
        let mut blank = false;
        let mut pending_blank: Option<Vec<u8>> = None;
        loop {
            let at = self.lines.offset;
            let line = match self.lines.next_line() {
                Ok(Some(line)) => line,
                Ok(None) => {
                    self.done = true;
                    break;
                }
                Err(err) => {
                    self.done = true;
                    return Some(Err(MboxError::Stopped {
                        message: self.read + 1,
                        reason: err.to_string(),
                    }));
                }
            };
            if blank && line.starts_with(b"From ") {
                self.start = at;
                break;
            }
            // The blank line before a separator belongs to the mbox, not the
            // message; it is held back until the next line shows which.
            if let Some(held) = pending_blank.take() {
                push(&mut raw, &held);
            }
            blank = is_blank(line);
            if blank {
                pending_blank = Some(line.to_vec());
            } else {
                push(&mut raw, unescape(line));
            }
        }
        self.read += 1;
        Some(Ok((start, raw)))
    }
}

impl Iterator for Mbox {
    type Item = Result<Message, MboxError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.next_any()? {
                Ok((start, raw)) if !skipped(&raw) => {
                    return Some(Ok(Message {
                        locator: start.to_string(),
                        raw,
                    }));
                }
                Ok(_) => continue,
                Err(err) => return Some(Err(err)),
            }
        }
    }
}

/// Appends up to [`MAX_BYTES`]; the rest of a large message is dropped.
fn push(raw: &mut Vec<u8>, line: &[u8]) {
    let room = MAX_BYTES.saturating_sub(raw.len());
    raw.extend_from_slice(&line[..line.len().min(room)]);
}

fn is_blank(line: &[u8]) -> bool {
    line == b"\n" || line == b"\r\n"
}

/// `>From ` and `>>From ` lose one `>`.
fn unescape(line: &[u8]) -> &[u8] {
    let quotes = line.iter().take_while(|&&b| b == b'>').count();
    if quotes > 0 && line[quotes..].starts_with(b"From ") {
        &line[1..]
    } else {
        line
    }
}

/// Whether Takeout labelled the message as mail nobody sent the user.
fn skipped(raw: &[u8]) -> bool {
    let head_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .or_else(|| raw.windows(2).position(|w| w == b"\n\n"))
        .unwrap_or(raw.len());
    let head = String::from_utf8_lossy(&raw[..head_end]);
    let mut labels = String::new();
    let mut inside = false;
    for line in head.lines() {
        if inside && line.starts_with([' ', '\t']) {
            labels.push_str(line);
            continue;
        }
        inside = false;
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("x-gmail-labels")
        {
            labels.push_str(value);
            inside = true;
        }
    }
    labels
        .split(',')
        .map(|l| l.trim().trim_matches('"').to_ascii_lowercase())
        .any(|l| SKIPPED_LABELS.contains(&l.as_str()))
}

fn reader(path: &Path) -> io::Result<Box<dyn BufRead + Send>> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 2];
    let n = file.read(&mut magic)?;
    let file = File::open(path)?;
    Ok(if n == 2 && magic == [0x1f, 0x8b] {
        Box::new(BufReader::with_capacity(
            1 << 16,
            MultiGzDecoder::new(BufReader::new(file)),
        ))
    } else {
        Box::new(BufReader::with_capacity(1 << 16, file))
    })
}

/// Lines with their endings, and the offset of the next one. One buffer is
/// reused; a line longer than [`MAX_BYTES`] keeps only its start.
struct Lines {
    reader: Box<dyn BufRead + Send>,
    buf: Vec<u8>,
    offset: u64,
}

impl Lines {
    fn new(reader: Box<dyn BufRead + Send>) -> Self {
        Self {
            reader,
            buf: Vec::new(),
            offset: 0,
        }
    }

    fn next_line(&mut self) -> io::Result<Option<&[u8]>> {
        self.buf.clear();
        loop {
            let available = self.reader.fill_buf()?;
            if available.is_empty() {
                break;
            }
            let (take, end) = match available.iter().position(|&b| b == b'\n') {
                Some(i) => (i + 1, true),
                None => (available.len(), false),
            };
            let room = MAX_BYTES.saturating_sub(self.buf.len());
            self.buf.extend_from_slice(&available[..take.min(room)]);
            self.reader.consume(take);
            self.offset += take as u64;
            if end {
                break;
            }
        }
        Ok((!self.buf.is_empty()).then_some(self.buf.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn file(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(bytes).unwrap();
        f
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }

    const SAMPLE: &[u8] = b"From a@x.test Mon Jan  5 10:00:00 2026\n\
From: a@x.test\n\
Subject: one\n\
\n\
Hello.\n\
>From the archive, an escaped line.\n\
>>From a quoted escaped line.\n\
\n\
From 1712345678@xxx Tue Jan  6 10:00:00 +0000 2026\n\
X-Gmail-Labels: Spam,Unread\n\
From: spam@x.test\n\
\n\
Buy now.\n\
\n\
From b@x.test Wed Jan  7 10:00:00 2026\n\
X-Gmail-Labels: Inbox,\n\
 Category Purchases\n\
From: b@x.test\n\
\n\
Body.\n\
From here on, not a separator: no blank line before it.\n";

    fn read(bytes: &[u8]) -> Vec<Message> {
        open(file(bytes).path())
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn messages_split_unescape_and_skip_labels() {
        let messages = read(SAMPLE);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].locator, "0");
        assert_eq!(
            String::from_utf8_lossy(&messages[0].raw),
            "From: a@x.test\nSubject: one\n\nHello.\nFrom the archive, an escaped line.\n\
             >From a quoted escaped line.\n"
        );
        assert!(
            String::from_utf8_lossy(&messages[1].raw)
                .ends_with("From here on, not a separator: no blank line before it.\n")
        );
        assert_eq!(count(file(SAMPLE).path()).unwrap(), 3);
    }

    #[test]
    fn a_gzipped_file_reads_the_same_and_offsets_find_messages_again() {
        let plain = read(SAMPLE);
        let zipped = file(&gzip(SAMPLE));
        assert_eq!(
            open(zipped.path())
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            plain
        );
        let offset: u64 = plain[1].locator.parse().unwrap();
        assert_eq!(
            read_at(zipped.path(), offset).unwrap(),
            Some(plain[1].raw.clone())
        );
        assert_eq!(read_at(zipped.path(), offset + 1).unwrap(), None);
    }

    #[test]
    fn crlf_mboxes_split_too() {
        let crlf = String::from_utf8_lossy(SAMPLE).replace('\n', "\r\n");
        let messages = read(crlf.as_bytes());
        assert_eq!(messages.len(), 2);
        assert!(messages[0].raw.ends_with(b"line.\r\n"));
    }

    #[test]
    fn a_file_that_is_not_an_mbox_is_refused() {
        assert!(matches!(
            open(file(b"Subject: hi\n\nbody\n").path()),
            Err(MboxError::NotMbox)
        ));
        assert!(matches!(open(file(b"").path()), Err(MboxError::NotMbox)));
    }

    #[test]
    fn a_truncated_gzip_stops_after_the_intact_messages() {
        let mut big = Vec::new();
        for n in 0..400 {
            big.extend_from_slice(
                format!(
                    "From x Mon Jan  5 10:00:00 2026\nSubject: {n}\n\n{}\n\n",
                    "y".repeat(500)
                )
                .as_bytes(),
            );
        }
        let zipped = gzip(&big);
        let cut = file(&zipped[..zipped.len() / 2]);
        let mut mbox = open(cut.path()).unwrap();
        let mut good = 0;
        let stopped = loop {
            match mbox.next() {
                Some(Ok(_)) => good += 1,
                Some(Err(err)) => break err,
                None => panic!("a truncated stream ended cleanly"),
            }
        };
        assert!(good > 0);
        assert!(matches!(stopped, MboxError::Stopped { message, .. } if message == good + 1));
    }

    #[test]
    fn a_huge_message_keeps_only_its_start() {
        let mut big = b"From x Mon Jan  5 10:00:00 2026\nSubject: big\n\n".to_vec();
        big.extend(std::iter::repeat_n(b'z', MAX_BYTES * 2));
        big.extend_from_slice(b"\n\nFrom y Mon Jan  5 10:00:00 2026\nSubject: next\n\nok\n");
        let messages = read(&big);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].raw.len(), MAX_BYTES);
        assert_eq!(messages[1].raw, b"Subject: next\n\nok\n");
    }
}
