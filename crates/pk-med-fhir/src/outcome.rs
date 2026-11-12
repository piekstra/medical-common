//! A FHIR server's refusals on the family's exit codes.
//!
//! | Answer | Error | Exit |
//! |---|---|---|
//! | 401 | `Auth`, `Reason::CredentialRefused` | 3 |
//! | 403 with FHIR or JSON | `Auth`, `Reason::Forbidden` | 3 |
//! | 403 with an HTML page (an edge block), 429, 5xx | `Upstream`, with `Retry-After` | 5 |
//! | 404, 410 | `NotFound` | 4 |
//! | 400, 422, any other 4xx | `Upstream`, `Reason::BadRequest` | 5 |
//! | a 3xx | `Upstream`: redirects are not followed | 5 |
//!
//! An OperationOutcome in the body refines a 4xx by its first issue of
//! severity `error` or `fatal` (a warning never does), by the issue's code: `login`, `unknown`, `expired` are 3 with `CredentialRefused`;
//! `forbidden`, `suppressed`, `security` 3 with `Forbidden`; `not-found`,
//! `deleted` 4; `throttled`, `transient`, `timeout`, `exception` 5. Messages
//! name the server's origin and the resource type, never a query, an id or a
//! token, and quote the outcome's text cut to printable characters. What to
//! do next is the error's remedy ([`SIGN_IN_AGAIN`], [`TRY_AGAIN`]), which a
//! CLI can replace with `CliError::with_remedy`.

use pk_cli_core::transport::HttpResponse;
use pk_cli_core::{CliError, Reason};

use crate::resources::{OperationOutcome, OperationOutcomeIssue};

/// The remedy on a refused credential (exit 3, `CredentialRefused`). It is
/// the error's remedy, not part of its message, so a CLI replaces it with
/// its own (`with_remedy("run auth login --org …")`) rather than add a
/// second one.
pub const SIGN_IN_AGAIN: &str = "sign in again";
/// The remedy on an answer that says nothing (a 429, a 5xx, an edge's
/// HTML 403, a transient OperationOutcome).
pub const TRY_AGAIN: &str = "try again later";

impl OperationOutcome {
    /// The first issue of severity `fatal` or `error`. A warning or an
    /// information issue never refines an exit code.
    pub fn first_error(&self) -> Option<&OperationOutcomeIssue> {
        self.issue
            .iter()
            .find(|i| matches!(i.severity.as_deref(), Some("fatal" | "error")))
    }

    /// Whether any issue is `fatal` or `error`.
    pub fn is_error(&self) -> bool {
        self.issue
            .iter()
            .any(|i| matches!(i.severity.as_deref(), Some("fatal" | "error")))
    }

    /// One line for a person: each issue's code and text, printable, 200
    /// characters at most.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = self
            .issue
            .iter()
            .map(|i| {
                let text = i
                    .details
                    .as_ref()
                    .and_then(|d| d.text.clone())
                    .or_else(|| i.diagnostics.clone());
                match (i.code.as_deref(), text) {
                    (Some(code), Some(text)) => format!("{code}: {text}"),
                    (Some(code), None) => code.to_string(),
                    (None, Some(text)) => text,
                    (None, None) => String::new(),
                }
            })
            .filter(|s| !s.is_empty())
            .collect();
        printable(&parts.join("; "))
    }
}

/// The error for a FHIR answer that is not a 2xx. `what` names the resource
/// type asked for (`Observation`, `Binary`); `origin` the server
/// (`https://fhir.example.org`); `now` is Unix seconds, for a `Retry-After`
/// given as a date.
pub fn status_error(what: &str, origin: &str, response: &HttpResponse, now: i64) -> CliError {
    let status = response.status;
    let outcome = parse_outcome(&response.body);
    let detail = outcome
        .as_ref()
        .map(OperationOutcome::summary)
        .filter(|s| !s.is_empty())
        .map(|s| format!(" ({s})"))
        .unwrap_or_default();
    let said = format!("the FHIR server at {origin} answered HTTP {status} for {what}{detail}");

    if (300..400).contains(&status) {
        return CliError::Upstream(format!("{said}: a redirect, which is not followed"));
    }
    if let Some(inconclusive) = response.inconclusive(now) {
        let wait = inconclusive
            .retry_after
            .map(|d| format!("; it asked to wait {} s", d.as_secs()))
            .unwrap_or_default();
        return CliError::Upstream(format!("{said}{wait}")).with_remedy(TRY_AGAIN);
    }
    if let Some(code) = outcome
        .as_ref()
        .and_then(|o| o.first_error())
        .and_then(|i| i.code.as_deref())
    {
        if let Some(err) = by_issue(code, &said) {
            return err;
        }
    }
    match status {
        401 => CliError::Auth(said)
            .because(Reason::CredentialRefused)
            .with_remedy(SIGN_IN_AGAIN),
        403 => {
            CliError::Auth(format!("{said}: access to it was refused")).because(Reason::Forbidden)
        }
        404 | 410 => CliError::NotFound(said),
        400..=499 => CliError::Upstream(said).because(Reason::BadRequest),
        _ => CliError::Upstream(said),
    }
}

/// The error for an OperationOutcome that came with a 2xx in place of the
/// resource asked for, or `None` when it carries no error.
pub fn outcome_error(what: &str, origin: &str, outcome: &OperationOutcome) -> Option<CliError> {
    if !outcome.is_error() {
        return None;
    }
    let said = format!(
        "the FHIR server at {origin} answered {what} with an OperationOutcome ({})",
        outcome.summary()
    );
    let code = outcome.first_error().and_then(|i| i.code.as_deref());
    Some(
        code.and_then(|c| by_issue(c, &said))
            .unwrap_or(CliError::Upstream(said)),
    )
}

fn by_issue(code: &str, said: &str) -> Option<CliError> {
    Some(match code {
        "login" | "unknown" | "expired" => CliError::Auth(said.to_string())
            .because(Reason::CredentialRefused)
            .with_remedy(SIGN_IN_AGAIN),
        "forbidden" | "suppressed" | "security" => {
            CliError::Auth(format!("{said}: access to it was refused")).because(Reason::Forbidden)
        }
        "not-found" | "deleted" => CliError::NotFound(said.to_string()),
        "throttled" | "transient" | "timeout" | "exception" => {
            CliError::Upstream(said.to_string()).with_remedy(TRY_AGAIN)
        }
        _ => return None,
    })
}

fn parse_outcome(body: &str) -> Option<OperationOutcome> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    crate::from_value(value).ok()
}

/// Text from a server, safe to print: control characters as spaces, 200
/// characters at most.
pub fn printable(text: &str) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(200)
        .collect();
    if text.chars().count() > 200 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "https://fhir.example.test";

    fn outcome(code: &str) -> String {
        format!(
            r#"{{"resourceType":"OperationOutcome","issue":[{{"severity":"error","code":"{code}","diagnostics":"synthetic"}}]}}"#
        )
    }

    fn exit(status: u16, body: &str) -> (i32, Option<Reason>, String) {
        let e = status_error("Observation", ORIGIN, &HttpResponse::new(status, body), 0);
        (e.exit_code(), e.reason(), e.to_string())
    }

    #[test]
    fn statuses_map_to_exit_codes() {
        for (status, code, reason) in [
            (401, 3, Some(Reason::CredentialRefused)),
            (403, 3, Some(Reason::Forbidden)),
            (404, 4, None),
            (410, 4, None),
            (400, 5, Some(Reason::BadRequest)),
            (422, 5, Some(Reason::BadRequest)),
            (429, 5, None),
            (500, 5, None),
            (503, 5, None),
            (302, 5, None),
        ] {
            let (got, why, text) = exit(status, "");
            assert_eq!(got, code, "{status}: {text}");
            assert_eq!(why, reason, "{status}");
            assert!(
                text.contains(ORIGIN) && text.contains("Observation"),
                "{text}"
            );
        }
    }

    #[test]
    fn an_html_403_is_an_edge_block_not_a_refusal() {
        let page =
            HttpResponse::new(403, "<html>blocked</html>").header("content-type", "text/html");
        assert_eq!(status_error("Patient", ORIGIN, &page, 0).exit_code(), 5);
    }

    #[test]
    fn retry_after_is_named() {
        let busy = HttpResponse::new(429, "").header("retry-after", "7");
        let e = status_error("Patient", ORIGIN, &busy, 0);
        assert!(e.to_string().contains("wait 7 s"), "{e}");
    }

    #[test]
    fn an_outcome_refines_a_4xx() {
        for (code, exit_code) in [
            ("login", 3),
            ("expired", 3),
            ("forbidden", 3),
            ("security", 3),
            ("not-found", 4),
            ("deleted", 4),
            ("throttled", 5),
            ("not-supported", 5),
        ] {
            let (got, _, text) = exit(400, &outcome(code));
            assert_eq!(got, exit_code, "{code}: {text}");
            assert!(text.contains("synthetic"), "{text}");
        }
        assert_eq!(exit(400, &outcome("forbidden")).1, Some(Reason::Forbidden));
    }

    #[test]
    fn an_outcome_with_a_2xx_is_an_error_only_when_it_says_so() {
        let warning: OperationOutcome = crate::from_str(
            r#"{"resourceType":"OperationOutcome","issue":[{"severity":"warning","code":"informational"}]}"#,
        )
        .unwrap();
        assert!(outcome_error("Patient", ORIGIN, &warning).is_none());
        let error: OperationOutcome = crate::from_str(&outcome("not-found")).unwrap();
        assert_eq!(
            outcome_error("Patient", ORIGIN, &error)
                .unwrap()
                .exit_code(),
            4
        );
    }

    #[test]
    fn server_text_is_printable_and_bounded() {
        assert_eq!(printable("a\u{1b}[31mb\nc"), "a [31mb c");
        assert_eq!(printable(&"x".repeat(300)).chars().count(), 201);
    }

    #[test]
    fn a_cli_replaces_the_remedy_instead_of_adding_one() {
        let err = status_error(
            "Patient",
            "https://fhir.example.test",
            &HttpResponse::new(401, ""),
            0,
        );
        assert_eq!(err.remedy(), Some(SIGN_IN_AGAIN));
        assert_eq!(err.reason(), Some(Reason::CredentialRefused));
        let mine = err.with_remedy("run auth login --org example");
        let text = mine.to_string();
        assert!(
            text.ends_with("\u{2014} run auth login --org example"),
            "{text}"
        );
        assert!(!text.to_lowercase().contains("sign in again"), "{text}");
        assert_eq!(mine.exit_code(), 3);
        assert_eq!(mine.reason(), Some(Reason::CredentialRefused));

        let busy = status_error(
            "Patient",
            "https://fhir.example.test",
            &HttpResponse::new(503, ""),
            0,
        );
        assert_eq!(busy.remedy(), Some(TRY_AGAIN));
        assert_eq!(busy.exit_code(), 5);
    }
}
