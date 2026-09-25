//! The EmailTerminator domain. No Tauri dependency (PLAN.md 1.2).

pub mod action;
pub mod crypt;
pub mod extract;
pub mod ingest;
pub mod local;
pub mod scan;
pub mod setting;
pub mod source;
pub mod store;
mod tls;
pub mod view;
