//! The community data bundled from `data/` (PLAN.md 2.6). `build.rs` has
//! already validated it, so parsing here cannot fail.

use std::sync::LazyLock;

pub use et_data::{Catalog, Critical, Matcher, Playbook, Service, Step};

include!(concat!(env!("OUT_DIR"), "/data_files.rs"));

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let files: Vec<et_data::File<'_>> = FILES
        .iter()
        .map(|(path, text)| et_data::File { path, text })
        .collect();
    et_data::parse(&files).expect("data/ was validated by build.rs")
});

/// The bundled catalog.
pub fn catalog() -> &'static Catalog {
    &CATALOG
}
