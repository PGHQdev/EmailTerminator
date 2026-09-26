//! Validates the community data in `data/` (PLAN.md 2.6): every file must
//! parse into the schema in `lib.rs`, and no two entries may claim a sender.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use et_data::{File, Problem, parse};

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
    let mut paths = Vec::new();
    collect_toml(root, &mut paths).map_err(|err| vec![format!("{}: {err}", root.display())])?;

    let mut texts = Vec::new();
    for path in &paths {
        let text =
            fs::read_to_string(path).map_err(|err| vec![format!("{}: {err}", path.display())])?;
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        texts.push((relative, text));
    }
    let files: Vec<File<'_>> = texts
        .iter()
        .map(|(path, text)| File { path, text })
        .collect();
    let shown = |p: &Problem| format!("{}/{p}", root.display());
    parse(&files)
        .map(|_| files.len())
        .map_err(|problems| problems.iter().map(shown).collect())
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
    fn a_malformed_file_is_reported_with_its_path_and_line() {
        let dir = tempfile::tempdir().unwrap();
        let services = dir.path().join("services");
        fs::create_dir_all(&services).unwrap();
        fs::write(
            services.join("good.toml"),
            "name = \"Good\"\ndomains = [\"good.test\"]\n",
        )
        .unwrap();
        fs::write(services.join("bad.toml"), "name = \"Bad\"\nbroken =\n").unwrap();

        let errors = validate(dir.path()).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("services/bad.toml:2:"), "{}", errors[0]);
    }
}
