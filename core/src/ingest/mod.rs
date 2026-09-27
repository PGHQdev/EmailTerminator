//! Getting mail onto the machine (PLAN.md 1.6).

pub mod graph;
pub mod imap;
pub mod maildir;
pub mod mbox;
pub mod outlook;

/// Messages are read up to this size, from every source. Receipts and list
/// headers sit well inside it; the rest of a large attachment is skipped.
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

/// One message from a Graph folder or a file, with the locator that finds it
/// again (PLAN.md Part 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub locator: String,
    pub raw: Vec<u8>,
}
