//! The community data format (PLAN.md 2.6). These structs are the schema:
//! the validator and the app parse `data/` through the same code.

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;
use toml::value::Datetime;

/// One file under `data/`, its path relative to that directory.
pub struct File<'a> {
    pub path: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

#[derive(Debug, Default)]
pub struct Catalog {
    pub services: Vec<Service>,
    pub critical: Vec<Critical>,
}

#[derive(Debug)]
pub struct Service {
    /// The file stem, and the only identifier cancel stats may carry.
    pub key: String,
    pub name: String,
    pub matcher: Matcher,
    /// The vendor's email-preferences page, for mail without a working
    /// `List-Unsubscribe` header.
    pub unsubscribe: Option<String>,
    pub playbook: Option<Playbook>,
}

#[derive(Debug)]
pub struct Critical {
    pub name: String,
    pub matcher: Matcher,
}

#[derive(Debug)]
pub struct Playbook {
    /// The vendor's own help page the steps were taken from.
    pub source: String,
    /// The day someone last checked the steps against the source, `YYYY-MM-DD`.
    pub checked: String,
    pub minutes: Option<u32>,
    pub steps: Vec<Step>,
}

#[derive(Debug)]
pub struct Step {
    pub text: String,
    pub link: Option<String>,
}

/// Which senders an entry covers: every address at a domain or below it, and
/// address patterns where `*` is the only wildcard.
#[derive(Debug, Default)]
pub struct Matcher {
    pub domains: Vec<String>,
    pub senders: Vec<String>,
}

impl Matcher {
    /// How specifically this matcher names an address, or `None`. A sender
    /// pattern beats any domain; a longer domain beats a shorter one.
    pub fn strength(&self, address: &str) -> Option<usize> {
        let address = address.trim().to_ascii_lowercase();
        let (_, host) = address.rsplit_once('@')?;
        if self.senders.iter().any(|p| pattern_matches(p, &address)) {
            return Some(usize::MAX);
        }
        self.domains
            .iter()
            .filter(|d| host == d.as_str() || host.ends_with(&format!(".{d}")))
            .map(String::len)
            .max()
    }
}

impl Catalog {
    /// The service entry that names this sender most specifically.
    pub fn service_for(&self, address: &str) -> Option<&Service> {
        best(&self.services, address, |s| &s.matcher)
    }

    pub fn critical_for(&self, address: &str) -> Option<&Critical> {
        best(&self.critical, address, |c| &c.matcher)
    }

    pub fn service(&self, key: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.key == key)
    }
}

fn best<'a, T>(items: &'a [T], address: &str, matcher: impl Fn(&T) -> &Matcher) -> Option<&'a T> {
    items
        .iter()
        .filter_map(|item| matcher(item).strength(address).map(|s| (s, item)))
        .max_by_key(|(s, _)| *s)
        .map(|(_, item)| item)
}

/// `*` matches any run of characters in the local part; the domain is literal.
fn pattern_matches(pattern: &str, address: &str) -> bool {
    fn glob(p: &[u8], s: &[u8]) -> bool {
        match p.split_first() {
            None => s.is_empty(),
            Some((b'*', rest)) => (0..=s.len()).any(|i| glob(rest, &s[i..])),
            Some((c, rest)) => s.first() == Some(c) && glob(rest, &s[1..]),
        }
    }
    glob(pattern.as_bytes(), address.as_bytes())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawService {
    name: Spanned<String>,
    #[serde(default)]
    domains: Vec<Spanned<String>>,
    #[serde(default)]
    senders: Vec<Spanned<String>>,
    unsubscribe: Option<Spanned<String>>,
    playbook: Option<RawPlaybook>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlaybook {
    source: Spanned<String>,
    checked: Spanned<Datetime>,
    minutes: Option<Spanned<u32>>,
    steps: Spanned<Vec<RawStep>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStep {
    text: Spanned<String>,
    link: Option<Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCriticalFile {
    service: Vec<RawCritical>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCritical {
    name: Spanned<String>,
    #[serde(default)]
    domains: Vec<Spanned<String>>,
    #[serde(default)]
    senders: Vec<Spanned<String>>,
}

/// Where a problem sits: which file, and a span of its text.
struct Here<'a> {
    file: &'a File<'a>,
    problems: &'a mut Vec<Problem>,
}

impl Here<'_> {
    fn at(&mut self, span: Range<usize>, message: impl Into<String>) {
        let start = span.start.min(self.file.text.len());
        self.problems.push(Problem {
            file: self.file.path.to_owned(),
            line: self.file.text[..start].matches('\n').count() + 1,
            message: message.into(),
        });
    }
}

/// Parses every file, or returns every problem found with its file and line.
pub fn parse(files: &[File<'_>]) -> Result<Catalog, Vec<Problem>> {
    let mut problems = Vec::new();
    let mut catalog = Catalog::default();
    // Every matcher across a collection, with where it was first declared.
    let mut service_claims: HashMap<String, String> = HashMap::new();
    let mut critical_claims: HashMap<String, String> = HashMap::new();

    let mut files: Vec<&File<'_>> = files.iter().collect();
    files.sort_by_key(|f| f.path);
    for file in files {
        let mut here = Here {
            file,
            problems: &mut problems,
        };
        if let Some(stem) = file
            .path
            .strip_prefix("services/")
            .and_then(|p| p.strip_suffix(".toml"))
        {
            if !is_key(stem) {
                here.at(
                    0..0,
                    "the file name must be lowercase letters, digits and hyphens",
                );
            }
            let Some(raw) = deserialize::<RawService>(&mut here) else {
                continue;
            };
            let matcher = matcher(&mut here, &raw.domains, &raw.senders, &mut service_claims);
            let service = Service {
                key: stem.to_owned(),
                name: name(&mut here, &raw.name),
                matcher,
                unsubscribe: raw.unsubscribe.as_ref().map(|u| url(&mut here, u)),
                playbook: raw.playbook.map(|p| playbook(&mut here, p)),
            };
            catalog.services.push(service);
        } else if file.path == "critical.toml" {
            let Some(raw) = deserialize::<RawCriticalFile>(&mut here) else {
                continue;
            };
            for entry in raw.service {
                let matcher = matcher(
                    &mut here,
                    &entry.domains,
                    &entry.senders,
                    &mut critical_claims,
                );
                if matcher.domains.is_empty() && matcher.senders.is_empty() {
                    here.at(
                        entry.name.span(),
                        "an entry needs at least one domain or sender",
                    );
                }
                catalog.critical.push(Critical {
                    name: name(&mut here, &entry.name),
                    matcher,
                });
            }
        } else {
            here.at(
                0..0,
                "data/ holds services/<key>.toml and critical.toml only",
            );
        }
    }
    for service in &catalog.services {
        if service.matcher.domains.is_empty() && service.matcher.senders.is_empty() {
            problems.push(Problem {
                file: format!("services/{}.toml", service.key),
                line: 1,
                message: "an entry needs at least one domain or sender".into(),
            });
        }
    }

    if problems.is_empty() {
        Ok(catalog)
    } else {
        problems.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        Err(problems)
    }
}

fn deserialize<T: serde::de::DeserializeOwned>(here: &mut Here<'_>) -> Option<T> {
    match toml::from_str::<T>(here.file.text) {
        Ok(value) => Some(value),
        Err(err) => {
            here.at(err.span().unwrap_or(0..0), err.message().trim_end());
            None
        }
    }
}

fn is_key(stem: &str) -> bool {
    !stem.is_empty()
        && !stem.starts_with('-')
        && !stem.ends_with('-')
        && stem
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn name(here: &mut Here<'_>, name: &Spanned<String>) -> String {
    let text = name.get_ref().trim();
    if text.is_empty() {
        here.at(name.span(), "the name is empty");
    }
    text.to_owned()
}

fn is_host(host: &str) -> bool {
    host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains("..")
        && host
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

fn url(here: &mut Here<'_>, url: &Spanned<String>) -> String {
    let text = url.get_ref();
    let host = text
        .strip_prefix("https://")
        .map(|rest| rest.split(['/', '?', '#']).next().unwrap_or(""));
    match host {
        None => here.at(url.span(), "a link must start with https://"),
        Some(host) if !is_host(host) || text.chars().any(char::is_whitespace) => {
            here.at(url.span(), format!("\"{text}\" is not a well-formed link"));
        }
        Some(_) => {}
    }
    text.clone()
}

fn matcher(
    here: &mut Here<'_>,
    domains: &[Spanned<String>],
    senders: &[Spanned<String>],
    claims: &mut HashMap<String, String>,
) -> Matcher {
    let mut claim = |here: &mut Here<'_>, span: Range<usize>, value: &str| {
        let owner = here.file.path.to_owned();
        match claims.get(value) {
            Some(first) if first == &owner => {
                here.at(span, format!("\"{value}\" is listed twice"));
            }
            Some(first) => here.at(span, format!("\"{value}\" is already claimed by {first}")),
            None => {
                claims.insert(value.to_owned(), owner);
            }
        }
    };

    for domain in domains {
        let value = domain.get_ref();
        if !is_host(value) {
            here.at(
                domain.span(),
                format!("\"{value}\" is not a lowercase domain such as example.com"),
            );
        }
        claim(here, domain.span(), value);
    }
    for sender in senders {
        let value = sender.get_ref();
        let well_formed = value.split_once('@').is_some_and(|(local, host)| {
            !local.is_empty()
                && local != "*"
                && local
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._+-*".contains(c))
                && is_host(host)
        });
        if !well_formed {
            here.at(
                sender.span(),
                format!(
                    "\"{value}\" is not an address pattern such as billing@example.com; \
                     `*` may stand in the part before the @, and a whole domain goes in domains"
                ),
            );
        }
        claim(here, sender.span(), value);
    }
    Matcher {
        domains: domains.iter().map(|d| d.get_ref().clone()).collect(),
        senders: senders.iter().map(|s| s.get_ref().clone()).collect(),
    }
}

fn playbook(here: &mut Here<'_>, raw: RawPlaybook) -> Playbook {
    let source = url(here, &raw.source);
    let checked = raw.checked.get_ref();
    let checked = match (&checked.date, &checked.time) {
        (Some(date), None) => date.to_string(),
        _ => {
            here.at(
                raw.checked.span(),
                "checked must be a date such as 2026-09-26",
            );
            String::new()
        }
    };
    let minutes = raw.minutes.map(|m| {
        if *m.get_ref() == 0 {
            here.at(m.span(), "minutes must be at least 1");
        }
        *m.get_ref()
    });
    let steps_span = raw.steps.span();
    let raw_steps = raw.steps.into_inner();
    if raw_steps.is_empty() {
        here.at(steps_span, "a playbook needs at least one step");
    }
    let steps = raw_steps
        .into_iter()
        .map(|step| {
            if step.text.get_ref().trim().is_empty() {
                here.at(step.text.span(), "the step text is empty");
            }
            Step {
                text: step.text.get_ref().trim().to_owned(),
                link: step.link.as_ref().map(|l| url(here, l)),
            }
        })
        .collect();
    Playbook {
        source,
        checked,
        minutes,
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETFLIX: &str = r#"name = "Netflix"
domains = ["netflix.com"]

[playbook]
source = "https://help.netflix.com/en/node/407"
checked = 2026-09-26

[[playbook.steps]]
text = "Open Account."
link = "https://www.netflix.com/account"
"#;

    fn problems(files: &[(&str, &str)]) -> Vec<String> {
        let files: Vec<File<'_>> = files
            .iter()
            .map(|(path, text)| File { path, text })
            .collect();
        parse(&files)
            .err()
            .unwrap_or_default()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn catalog(files: &[(&str, &str)]) -> Catalog {
        let files: Vec<File<'_>> = files
            .iter()
            .map(|(path, text)| File { path, text })
            .collect();
        parse(&files).unwrap()
    }

    #[test]
    fn a_service_entry_parses() {
        let catalog = catalog(&[("services/netflix.toml", NETFLIX)]);
        let netflix = &catalog.services[0];
        assert_eq!(netflix.key, "netflix");
        let playbook = netflix.playbook.as_ref().unwrap();
        assert_eq!(playbook.checked, "2026-09-26");
        assert_eq!(
            playbook.steps[0].link.as_deref(),
            Some("https://www.netflix.com/account")
        );
    }

    #[test]
    fn an_unknown_key_is_reported_with_its_line() {
        let text = NETFLIX.replace("[playbook]", "colour = \"red\"\n[playbook]");
        let found = problems(&[("services/netflix.toml", &text)]);
        assert_eq!(found.len(), 1);
        assert!(
            found[0].starts_with("services/netflix.toml:4:"),
            "{}",
            found[0]
        );
        assert!(found[0].contains("colour"), "{}", found[0]);
    }

    #[test]
    fn a_domain_claimed_twice_names_the_first_file() {
        let other = NETFLIX.replace("Netflix", "Other");
        let found = problems(&[
            ("services/netflix.toml", NETFLIX),
            ("services/other.toml", &other),
        ]);
        assert_eq!(
            found,
            vec![
                "services/other.toml:2: \"netflix.com\" is already claimed by services/netflix.toml"
            ]
        );
    }

    #[test]
    fn a_critical_domain_may_repeat_a_service_domain() {
        let critical = "[[service]]\nname = \"Netflix\"\ndomains = [\"netflix.com\"]\n";
        let catalog = catalog(&[
            ("services/netflix.toml", NETFLIX),
            ("critical.toml", critical),
        ]);
        assert_eq!(catalog.critical.len(), 1);
    }

    #[test]
    fn links_must_be_https() {
        let text = NETFLIX.replace("https://www.netflix.com/account", "http://netflix.com");
        let found = problems(&[("services/netflix.toml", &text)]);
        assert_eq!(
            found,
            vec!["services/netflix.toml:10: a link must start with https://"]
        );
    }

    #[test]
    fn a_checked_value_with_a_time_is_refused() {
        let text = NETFLIX.replace("2026-09-26", "2026-09-26T10:00:00Z");
        let found = problems(&[("services/netflix.toml", &text)]);
        assert!(
            found[0].contains(":6: checked must be a date"),
            "{}",
            found[0]
        );
    }

    #[test]
    fn bad_names_and_stray_files_are_refused() {
        let found = problems(&[
            ("services/Net_flix.toml", NETFLIX),
            ("providers.toml", "x = 1\n"),
        ]);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("providers.toml:1:"), "{}", found[0]);
        assert!(found[1].contains("lowercase letters"), "{}", found[1]);
    }

    #[test]
    fn a_whole_domain_is_not_a_sender_pattern() {
        let text = NETFLIX.replace(
            "domains = [\"netflix.com\"]",
            "senders = [\"*@netflix.com\", \"Bill@netflix.com\"]",
        );
        let found = problems(&[("services/netflix.toml", &text)]);
        assert_eq!(found.len(), 2, "{found:?}");
    }

    #[test]
    fn the_most_specific_matcher_wins() {
        let amazon = "name = \"Amazon\"\ndomains = [\"amazon.com\"]\n";
        let aws =
            "name = \"AWS\"\ndomains = [\"aws.amazon.com\"]\nsenders = [\"aws-*@amazon.com\"]\n";
        let catalog = catalog(&[("services/amazon.toml", amazon), ("services/aws.toml", aws)]);
        let key = |a: &str| catalog.service_for(a).map(|s| s.key.as_str());
        assert_eq!(key("orders@amazon.com"), Some("amazon"));
        assert_eq!(key("aws-billing@amazon.com"), Some("aws"));
        assert_eq!(key("x@mail.aws.amazon.com"), Some("aws"));
        assert_eq!(key("x@notamazon.com"), None);
        assert_eq!(key("Orders@Amazon.com"), Some("amazon"));
    }
}
