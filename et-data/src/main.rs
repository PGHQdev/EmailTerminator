//! Validates the community data in `data/` (PLAN.md 2.6). Until M4 adds the
//! service schema, it checks that every TOML file parses.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (Some("validate"), dir) => {
            let root = dir
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("data"));
            report(validate(&root))
        }
        _ => {
            eprintln!("usage: et-data validate [DATA_DIR]");
            ExitCode::from(2)
        }
    }
}

fn report(result: Result<usize, Vec<String>>) -> ExitCode {
    match result {
        Ok(count) => {
            println!("{count} file(s) valid");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in &errors {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

fn validate(root: &Path) -> Result<usize, Vec<String>> {
    let mut files = Vec::new();
    collect_toml(root, &mut files).map_err(|err| vec![format!("{}: {err}", root.display())])?;
    files.sort();

    let errors: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let text = match fs::read_to_string(path) {
                Ok(text) => text,
                Err(err) => return Some(format!("{}: {err}", path.display())),
            };
            text.parse::<toml::Table>().err().map(|err| {
                let line = err
                    .span()
                    .map(|span| text[..span.start].lines().count().max(1))
                    .unwrap_or(1);
                format!("{}:{line}: {}", path.display(), err.message())
            })
        })
        .collect();

    if errors.is_empty() {
        Ok(files.len())
    } else {
        Err(errors)
    }
}

fn collect_toml(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_toml(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_has_nothing_to_reject() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(validate(&dir.path().join("absent")), Ok(0));
    }

    #[test]
    fn a_malformed_file_is_reported_with_its_line() {
        let dir = tempfile::tempdir().unwrap();
        let services = dir.path().join("services");
        fs::create_dir_all(&services).unwrap();
        fs::write(services.join("good.toml"), "name = \"Good\"\n").unwrap();
        fs::write(services.join("bad.toml"), "name = \"Bad\"\nbroken =\n").unwrap();

        let errors = validate(dir.path()).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("bad.toml:2:"), "{}", errors[0]);
    }
}
