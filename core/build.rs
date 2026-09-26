//! Bundles `data/` into the binary (PLAN.md 2.6: bundle-only at v0). The data
//! is parsed here first, so a malformed entry fails the build with its file
//! and line instead of failing at launch.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
    println!("cargo:rerun-if-changed={}", root.display());

    let mut paths = Vec::new();
    collect(&root, &mut paths);
    paths.sort();

    let texts: Vec<(String, String)> = paths
        .iter()
        .map(|path| {
            println!("cargo:rerun-if-changed={}", path.display());
            let relative = path
                .strip_prefix(&root)
                .expect("under data/")
                .to_string_lossy()
                .replace('\\', "/");
            (
                relative,
                fs::read_to_string(path).expect("readable data file"),
            )
        })
        .collect();
    let files: Vec<et_data::File<'_>> = texts
        .iter()
        .map(|(path, text)| et_data::File { path, text })
        .collect();
    if let Err(problems) = et_data::parse(&files) {
        for problem in problems {
            println!("cargo:warning=data/{problem}");
        }
        panic!("data/ does not validate; run `cargo run -p et-data -- validate`");
    }

    let mut out = String::from("pub(crate) static FILES: &[(&str, &str)] = &[\n");
    for (relative, path) in texts.iter().map(|(r, _)| r).zip(&paths) {
        let path = path.display().to_string();
        writeln!(out, "    ({relative:?}, include_str!({path:?})),").unwrap();
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("data_files.rs");
    fs::write(dest, out).unwrap();
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            out.push(path);
        }
    }
}
