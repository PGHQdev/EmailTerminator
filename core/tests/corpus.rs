//! The synthetic corpus (PLAN.md 2.9, Part 9). `tests/corpus/out/` is
//! generated and committed. After changing the generator, rewrite it with
//! `cargo test -p et-core --test corpus -- --ignored regenerate`.

#[path = "corpus/generator/mod.rs"]
mod generator;

use std::collections::BTreeMap;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use et_core::extract::{Extraction, extract};
use generator::{SEED, SeriesExpected, generate};
use mail_parser::MessageParser;
use serde_json::Value;

fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/out")
}

/// Every file under `root`, keyed by its `/`-separated relative path.
fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("read corpus directory") {
            let path = entry.expect("read corpus entry").path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("inside root")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                files.insert(relative, fs::read(&path).expect("read corpus file"));
            }
        }
    }
    let mut files = BTreeMap::new();
    if root.exists() {
        walk(root, root, &mut files);
    }
    files
}

fn is_series(path: &str) -> bool {
    path.starts_with("series/")
}

/// Single messages: `(stem, raw, golden)`.
fn goldens(files: &BTreeMap<String, Vec<u8>>) -> Vec<(&str, &[u8], Extraction)> {
    files
        .iter()
        .filter(|(path, _)| path.ends_with(".json") && !is_series(path))
        .map(|(path, json)| {
            let stem = path.strip_suffix(".json").expect("json");
            let raw = files
                .get(&format!("{stem}.eml"))
                .unwrap_or_else(|| panic!("{path} has no .eml beside it"));
            let golden = serde_json::from_slice(json)
                .unwrap_or_else(|e| panic!("{path} is not an Extraction: {e}"));
            (stem, raw.as_slice(), golden)
        })
        .collect()
}

/// A series member: its path under `out/` and its bytes.
type Member<'a> = (&'a str, &'a [u8]);

/// Series directories: name, expected result, members in file order.
fn series(files: &BTreeMap<String, Vec<u8>>) -> Vec<(String, SeriesExpected, Vec<Member<'_>>)> {
    let mut by_name: BTreeMap<String, (Option<SeriesExpected>, Vec<Member<'_>>)> = BTreeMap::new();
    for (path, bytes) in files.iter().filter(|(path, _)| is_series(path)) {
        let (name, file) = path["series/".len()..]
            .split_once('/')
            .unwrap_or_else(|| panic!("{path} is not inside a series directory"));
        let entry = by_name.entry(name.to_owned()).or_default();
        if file == "expected.json" {
            entry.0 = Some(
                serde_json::from_slice(bytes)
                    .unwrap_or_else(|e| panic!("{path} is not a SeriesExpected: {e}")),
            );
        } else {
            assert!(file.ends_with(".eml"), "unexpected file {path}");
            entry.1.push((path.as_str(), bytes.as_slice()));
        }
    }
    by_name
        .into_iter()
        .map(|(name, (expected, members))| {
            let expected = expected.unwrap_or_else(|| panic!("series/{name} has no expected.json"));
            (name, expected, members)
        })
        .collect()
}

#[test]
#[ignore = "rewrites tests/corpus/out; run after changing the generator"]
fn regenerate() {
    let out = out_dir();
    if out.exists() {
        fs::remove_dir_all(&out).expect("delete old corpus");
    }
    for (path, bytes) in generate(SEED) {
        let target = out.join(path);
        fs::create_dir_all(target.parent().expect("has parent")).expect("create directory");
        fs::write(target, bytes).expect("write corpus file");
    }
}

#[test]
fn generation_is_deterministic() {
    assert!(generate(SEED) == generate(SEED));
}

#[test]
fn corpus_is_current() {
    let expected = generate(SEED);
    let actual = read_tree(&out_dir());
    let mut problems = Vec::new();
    for (path, bytes) in &expected {
        match actual.get(path) {
            None => problems.push(format!("missing: {path}")),
            Some(on_disk) if on_disk != bytes => problems.push(format!("differs: {path}")),
            Some(_) => {}
        }
    }
    for path in actual.keys().filter(|path| !expected.contains_key(*path)) {
        problems.push(format!("not generated: {path}"));
    }
    assert!(
        problems.is_empty(),
        "tests/corpus/out is not what the generator produces. Run\n  cargo test -p et-core --test corpus -- --ignored regenerate\n{}",
        problems.join("\n")
    );
}

#[test]
fn every_message_parses() {
    let files = read_tree(&out_dir());
    assert!(!files.is_empty(), "the corpus is empty; run regenerate");
    let mut failures = Vec::new();
    for (path, raw) in files.iter().filter(|(path, _)| path.ends_with(".eml")) {
        match catch_unwind(AssertUnwindSafe(|| {
            MessageParser::default().parse(raw).is_some()
        })) {
            Ok(true) => {}
            Ok(false) => failures.push(format!("{path}: mail-parser returned None")),
            Err(_) => failures.push(format!("{path}: mail-parser panicked")),
        }
        if !is_series(path) {
            let stem = path.strip_suffix(".eml").expect("eml");
            if !files.contains_key(&format!("{stem}.json")) {
                failures.push(format!("{path}: no golden .json beside it"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // Both deserialise with deny_unknown_fields, so this proves the goldens
    // match the types.
    assert!(!goldens(&files).is_empty());
    assert!(!series(&files).is_empty());
}

#[test]
fn series_are_well_formed() {
    let files = read_tree(&out_dir());
    for (name, expected, members) in series(&files) {
        assert!(!members.is_empty(), "series/{name} is empty");
        let mut previous = None;
        for (i, (path, raw)) in members.iter().enumerate() {
            assert!(
                path.ends_with(&format!("/{:03}.eml", i + 1)),
                "series/{name}: {path} is out of sequence"
            );
            let message = MessageParser::default()
                .parse(*raw)
                .unwrap_or_else(|| panic!("{path} does not parse"));
            let date = message
                .date()
                .unwrap_or_else(|| panic!("{path} has no date"))
                .to_timestamp();
            if let Some(previous) = previous {
                assert!(
                    date >= previous,
                    "{path} is dated before the file ahead of it"
                );
            }
            previous = Some(date);
        }
        assert!(
            expected.charge_count as usize <= members.len(),
            "series/{name} expects more charges than it has files"
        );
        assert_eq!(
            expected.latest_amount_minor_units.is_some(),
            expected.currency.is_some(),
            "series/{name}: an amount needs a currency"
        );
        assert!(!expected.group_key.is_empty());
    }
}

/// Field-level differences between two JSON values.
fn diff(at: &str, expected: &Value, actual: &Value, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for key in e.keys().chain(a.keys().filter(|k| !e.contains_key(*k))) {
                let path = if at.is_empty() {
                    key.clone()
                } else {
                    format!("{at}.{key}")
                };
                diff(
                    &path,
                    e.get(key).unwrap_or(&Value::Null),
                    a.get(key).unwrap_or(&Value::Null),
                    out,
                );
            }
        }
        _ if expected != actual => out.push(format!("  {at}: expected {expected}, got {actual}")),
        _ => {}
    }
}

#[test]
fn extraction_matches_goldens() {
    let files = read_tree(&out_dir());
    let mut failures = Vec::new();
    let goldens = goldens(&files);
    for (stem, raw, golden) in &goldens {
        let actual = match catch_unwind(|| extract(raw)) {
            Ok(actual) => actual,
            Err(_) => {
                failures.push(format!("{stem}.eml: extract() panicked"));
                continue;
            }
        };
        if &actual != golden {
            let mut fields = Vec::new();
            diff(
                "",
                &serde_json::to_value(golden).expect("serialise golden"),
                &serde_json::to_value(&actual).expect("serialise extraction"),
                &mut fields,
            );
            failures.push(format!("{stem}.eml\n{}", fields.join("\n")));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} messages differ from their goldens:\n{}",
        failures.len(),
        goldens.len(),
        failures.join("\n")
    );
}

#[test]
fn series_match_expectations() {
    let files = read_tree(&out_dir());
    let mut failures = Vec::new();
    for (name, expected, members) in series(&files) {
        let extractions: Vec<Extraction> = members.iter().map(|(_, raw)| extract(raw)).collect();
        let groups = et_core::scan::group::series(&extractions);
        let Some(summary) = groups.get(&expected.group_key) else {
            failures.push(format!(
                "series/{name}: no service keyed {:?}; got {:?}",
                expected.group_key,
                groups.keys().collect::<Vec<_>>()
            ));
            continue;
        };
        let actual = (
            summary.cadence.map(|c| c.as_str().to_owned()),
            summary.charge_count as u32,
            summary.price_increase,
            summary.latest.as_ref().map(|(amount, _)| *amount),
            summary
                .latest
                .as_ref()
                .map(|(_, currency)| currency.clone()),
        );
        let wanted = (
            expected.cadence.map(|c| {
                serde_json::to_value(c)
                    .expect("cadence")
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            }),
            expected.charge_count,
            expected.price_increase,
            expected.latest_amount_minor_units,
            expected.currency.clone(),
        );
        if actual != wanted || groups.len() != 1 {
            failures.push(format!(
                "series/{name}: got {actual:?} in {} group(s), expected {wanted:?}",
                groups.len()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
