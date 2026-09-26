//! Unsubscribing one sender: pick the route its newest list mail offers,
//! take it, and log the attempt with that mail as the evidence.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use specta::Type;

use super::http::post_form;
use super::{Kind, NewEntry, Outcome, record};
use crate::extract::unsubscribe::{first, uris};
use crate::store::StoreError;

/// RFC 8058 §3.1: the whole body of a one-click POST.
pub const ONE_CLICK_BODY: &str = "List-Unsubscribe=One-Click";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Route {
    /// RFC 8058: one POST, and the sender stops.
    OneClick { url: String },
    /// The sender's web page. The user finishes there.
    Page { url: String },
    /// A `mailto:` address. The user's mail client sends it.
    Mail { uri: String },
    /// The mail carries no `List-Unsubscribe`.
    Nothing,
}

impl Route {
    /// From a stored `List-Unsubscribe` and the message's one-click verdict.
    /// HTTPS is preferred to plain HTTP, and a web page to email.
    pub fn from_header(list_unsubscribe: Option<&str>, one_click: bool) -> Self {
        let found = uris(list_unsubscribe.unwrap_or(""));
        if let Some(url) = first(&found, "https") {
            return if one_click {
                Route::OneClick {
                    url: url.to_owned(),
                }
            } else {
                Route::Page {
                    url: url.to_owned(),
                }
            };
        }
        if let Some(url) = first(&found, "http") {
            return Route::Page {
                url: url.to_owned(),
            };
        }
        match first(&found, "mailto") {
            Some(uri) => Route::Mail {
                uri: uri.to_owned(),
            },
            None => Route::Nothing,
        }
    }
}

/// One sender, ready to unsubscribe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub sender_id: i64,
    pub service_id: Option<i64>,
    pub name: String,
    /// The newest list message with a `List-Unsubscribe`, else the newest
    /// list message: the evidence either way.
    pub message: Option<i64>,
    pub route: Route,
}

pub fn plan(conn: &Connection, sender_id: i64) -> Result<Option<Attempt>, StoreError> {
    let Some((name, address, service_id)) = conn
        .query_row(
            "SELECT coalesce(display_name, address), address, service_id FROM sender
             WHERE id = ?1",
            [sender_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    let newest: Option<(i64, Option<String>, bool)> = conn
        .query_row(
            "SELECT id, list_unsubscribe, one_click FROM message
             WHERE sender_id = ?1 AND is_list = 1
             ORDER BY list_unsubscribe IS NULL, date DESC, id DESC LIMIT 1",
            [sender_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (message, route) = match newest {
        Some((id, header, one_click)) => {
            (Some(id), Route::from_header(header.as_deref(), one_click))
        }
        None => (None, Route::Nothing),
    };
    // No header to act on: the vendor's own preferences page, where `data/`
    // names one, is where the user finishes.
    let route = match route {
        Route::Nothing => crate::data::catalog()
            .service_for(&address)
            .and_then(|s| s.unsubscribe.clone())
            .map_or(Route::Nothing, |url| Route::Page { url }),
        route => route,
    };
    Ok(Some(Attempt {
        sender_id,
        service_id,
        name,
        message,
        route,
    }))
}

/// What taking a route produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub outcome: Outcome,
    pub detail: String,
    pub request: Option<String>,
}

/// The only step that touches the network, and only for `OneClick`.
pub async fn send(route: &Route) -> Sent {
    match route {
        Route::OneClick { url } => {
            let request = Some(format!("POST {url}"));
            match post_form(url, ONE_CLICK_BODY).await {
                Ok(code) if (200..300).contains(&code) => Sent {
                    outcome: Outcome::Succeeded,
                    detail: format!("the sender answered {code}"),
                    request,
                },
                Ok(code) => Sent {
                    outcome: Outcome::Failed,
                    detail: format!("the sender answered {code}"),
                    request,
                },
                Err(err) => Sent {
                    outcome: Outcome::Failed,
                    detail: err.to_string(),
                    request,
                },
            }
        }
        Route::Page { .. } => Sent {
            outcome: Outcome::NeedsYou,
            detail: "no one-click unsubscribe: finish on the sender's page".into(),
            request: None,
        },
        Route::Mail { .. } => Sent {
            outcome: Outcome::NeedsYou,
            detail: "this sender unsubscribes by email only".into(),
            request: None,
        },
        Route::Nothing => Sent {
            outcome: Outcome::Failed,
            detail: "no unsubscribe header in the mail".into(),
            request: None,
        },
    }
}

/// Logs the attempt, and marks the sender unsubscribed when it worked.
pub fn finish(
    conn: &Connection,
    attempt: &Attempt,
    sent: &Sent,
    at: &str,
) -> Result<i64, StoreError> {
    let id = record(
        conn,
        &NewEntry {
            kind: Kind::Unsubscribe,
            target: attempt.name.clone(),
            sender_id: Some(attempt.sender_id),
            service_id: attempt.service_id,
            source_id: None,
            at: at.to_owned(),
            outcome: sent.outcome,
            detail: sent.detail.clone(),
            request: sent.request.clone(),
            message: attempt.message,
        },
    )?;
    if sent.outcome == Outcome::Succeeded {
        conn.execute(
            "UPDATE sender SET unsubscribed_at = ?2 WHERE id = ?1",
            params![attempt.sender_id, at],
        )?;
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_prefer_one_click_then_the_web_then_email() {
        let both = "<mailto:u@a.test>, <https://a.test/u>";
        assert_eq!(
            Route::from_header(Some(both), true),
            Route::OneClick {
                url: "https://a.test/u".into()
            }
        );
        assert_eq!(
            Route::from_header(Some(both), false),
            Route::Page {
                url: "https://a.test/u".into()
            }
        );
        assert_eq!(
            Route::from_header(Some("<http://a.test/u>, <mailto:u@a.test>"), true),
            Route::Page {
                url: "http://a.test/u".into()
            }
        );
        assert_eq!(
            Route::from_header(Some("<mailto:u@a.test?subject=unsubscribe>"), false),
            Route::Mail {
                uri: "mailto:u@a.test?subject=unsubscribe".into()
            }
        );
        assert_eq!(Route::from_header(None, false), Route::Nothing);
    }

    #[tokio::test]
    async fn routes_without_a_post_send_nothing() {
        for (route, outcome) in [
            (
                Route::Page {
                    url: "https://a.test/u".into(),
                },
                Outcome::NeedsYou,
            ),
            (
                Route::Mail {
                    uri: "mailto:u@a.test".into(),
                },
                Outcome::NeedsYou,
            ),
            (Route::Nothing, Outcome::Failed),
        ] {
            let sent = send(&route).await;
            assert_eq!(sent.outcome, outcome);
            assert_eq!(sent.request, None);
        }
    }
}
