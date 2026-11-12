//! A FHIR read client over the family's transport seam.

use std::time::Duration;

use pk_cli_core::transport::{require_secure_url, HttpRequest, HttpResponse, Transport};
use pk_cli_core::{CliError, Reason};
use pk_cli_secrets::Secret;

use crate::outcome::{outcome_error, status_error};
use crate::raw::{RawBundle, RawResource};
use crate::resources::OperationOutcome;
use crate::{normalize_base, Resource, FHIR_JSON};

/// How many search pages [`FhirClient::search`] follows unless told otherwise.
pub const DEFAULT_PAGE_CAP: u32 = 100;

/// The reason on an error for a request that got no complete HTTP answer
/// after its retries: the connection failed, timed out, or broke before the
/// whole answer was read. A server that answered with an error status has
/// no such reason. The transport does not say which of those it was, so
/// this is not proof the network is gone: one slow page that timed out
/// three times reads the same. A reason the transport set is kept.
pub const UNREACHABLE: Reason = Reason::Provider("unreachable");

/// A bearer that can change during a run: called before each request with
/// `None`, and once more after a 401 with the token that was refused.
type TokenSource = Box<dyn Fn(Option<&Secret>) -> Result<Secret, CliError> + Send + Sync>;

/// Told of each wait before a retry ([`FhirClient::on_retry`]).
type RetryReport = Box<dyn Fn(&str) + Send + Sync>;

enum Bearer {
    None,
    Fixed(Secret),
    Source(TokenSource),
}

/// Whether and how long to wait and ask again after an answer that says
/// nothing (a 429, a 5xx, an edge's HTML 403).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Requests in all, the first included: `1` never asks again.
    pub attempts: u32,
    /// The wait before the second request, doubled before each later one,
    /// when the server names none (`Retry-After`).
    pub delay: Duration,
    /// The longest wait. A server asking for longer is not waited for: its
    /// answer is the error.
    pub max_wait: Duration,
}

impl RetryPolicy {
    /// One request, no retry.
    pub const fn none() -> Self {
        RetryPolicy {
            attempts: 1,
            delay: Duration::ZERO,
            max_wait: Duration::ZERO,
        }
    }
}

/// Three requests at most, 1 s then 2 s apart, waiting 30 s at most.
impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy {
            attempts: 3,
            delay: Duration::from_secs(1),
            max_wait: Duration::from_secs(30),
        }
    }
}

/// A search's resources across its pages.
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    /// Entries of mode `match`, or of no mode.
    pub resources: Vec<RawResource>,
    /// Entries of mode `include` (`_include`, `_revinclude`).
    pub included: Vec<RawResource>,
    /// OperationOutcomes the server sent beside the results (mode `outcome`),
    /// warnings mostly.
    pub outcomes: Vec<OperationOutcome>,
    /// Pages read.
    pub pages: u32,
    /// Whether a `next` page was left unread at the page cap.
    pub truncated: bool,
    /// `Bundle.total` of the first page, when the server counts.
    pub total: Option<u64>,
    /// Why each entry whose resource did not read was left out, its page's
    /// other entries kept ([`crate::RawEntry::unreadable`]). No value of the
    /// resource is in a reason.
    pub unreadable: Vec<String>,
}

/// Reads and searches one FHIR server. Every request goes to the base's own
/// origin, so the bearer and the extra headers never leave it: a `next` link
/// or a Binary reference elsewhere is refused. Nothing is printed.
///
/// ```no_run
/// # fn main() -> Result<(), pk_cli_core::CliError> {
/// use pk_med_fhir::FhirClient;
/// let transport = pk_cli_http::BlockingTransport::new()?;
/// let client = FhirClient::new(&transport, "https://fhir.example.org/r4", "mychart", "0.1.0")?
///     .bearer(pk_cli_secrets::Secret::new("token from the keychain"));
/// let page = pk_cli_core::block_on(
///     client.search("Observation", &[("patient", "p1".into()), ("category", "laboratory".into())]),
/// )?;
/// println!("{} observations over {} pages", page.resources.len(), page.pages);
/// # Ok(()) }
/// ```
pub struct FhirClient<T> {
    transport: T,
    base: String,
    origin: String,
    binary: String,
    version: String,
    bearer: Bearer,
    headers: Vec<(String, String)>,
    page_cap: u32,
    retry: RetryPolicy,
    sleeper: fn(Duration),
    clock: fn() -> i64,
    on_retry: Option<RetryReport>,
}

impl<T: Transport> FhirClient<T> {
    /// A client for the server at `base` (normalized, [`normalize_base`]),
    /// which must be `https` or `http` to this machine. `binary` and `version`
    /// make the `User-Agent`.
    pub fn new(
        transport: T,
        base: &str,
        binary: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, CliError> {
        let base = normalize_base(base);
        require_secure_url(&base)?;
        let origin = origin_of(&base)
            .ok_or_else(|| CliError::Usage("a FHIR base must be an absolute URL".into()))?;
        Ok(FhirClient {
            transport,
            base,
            origin,
            binary: binary.into(),
            version: version.into(),
            bearer: Bearer::None,
            headers: Vec::new(),
            page_cap: DEFAULT_PAGE_CAP,
            retry: RetryPolicy::default(),
            sleeper: std::thread::sleep,
            clock: now_unix,
            on_retry: None,
        })
    }

    /// Send `Authorization: Bearer <token>` on every request.
    pub fn bearer(mut self, token: Secret) -> Self {
        self.bearer = Bearer::Fixed(token);
        self
    }

    /// Take each request's bearer from `source`, for a token that lapses
    /// during a run (an export longer than the token's hour). It is called
    /// before every request, a retry included, with `None`. After a 401 it is
    /// called once more with the refused token, so it can renew
    /// (`pk_med_smart::Renewer::after_refusal`), and the request goes again
    /// with what it returns, unless that is the same token. An error from it
    /// is the request's error.
    pub fn bearer_with(
        mut self,
        source: impl Fn(Option<&Secret>) -> Result<Secret, CliError> + Send + Sync + 'static,
    ) -> Self {
        self.bearer = Bearer::Source(Box::new(source));
        self
    }

    /// Told of each wait before a retry, with one line for a person
    /// (`the FHIR server at https://… answered HTTP 429 for Observation;
    /// asking again in 5 s (2 of 3)`), so a long wait is not silent. The
    /// line names no query, id or token.
    pub fn on_retry(mut self, report: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.on_retry = Some(Box::new(report));
        self
    }

    /// Send a header on every request (Epic's `Epic-Client-ID`).
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Follow at most this many search pages (at least 1).
    pub fn page_cap(mut self, pages: u32) -> Self {
        self.page_cap = pages.max(1);
        self
    }

    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    /// How to wait between retries; `std::thread::sleep` unless set, for a
    /// host without threads or a test.
    pub fn sleeper(mut self, sleep: fn(Duration)) -> Self {
        self.sleeper = sleep;
        self
    }

    /// The normalized base.
    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// `GET <base>/<type>/<id>`.
    pub async fn read(&self, resource_type: &str, id: &str) -> Result<RawResource, CliError> {
        check_type(resource_type)?;
        check_id(id)?;
        let url = format!("{}/{resource_type}/{id}", self.base);
        let response = self.get(resource_type, url).await?;
        self.resource(resource_type, &response.body)
    }

    /// [`Self::read`], as one of this crate's types.
    pub async fn read_as<R: Resource>(&self, id: &str) -> Result<R, CliError> {
        self.read(R::TYPE, id).await?.parse_as()
    }

    /// `GET <base>/<type>?<params>`, following `Bundle.link[next]` as the
    /// server wrote it, on the base's origin only, up to the page cap. No
    /// parameter is added: `_count`, `patient` and the rest are the caller's.
    pub async fn search(
        &self,
        resource_type: &str,
        params: &[(&str, String)],
    ) -> Result<SearchResult, CliError> {
        check_type(resource_type)?;
        let mut url = HttpRequest::get(format!("{}/{resource_type}", self.base))
            .query(params)
            .url;
        let mut result = SearchResult::default();
        loop {
            let response = self.get(resource_type, url.clone()).await?;
            if let Some(err) = self.outcome_instead(resource_type, &response.body) {
                return Err(err);
            }
            let page = RawBundle::parse(&response.body)?;
            result.pages += 1;
            if result.pages == 1 {
                result.total = page.total;
            }
            let next = page.link("next").map(str::to_string);
            for entry in page.entry {
                let mode = entry.search.as_ref().and_then(|s| s.mode.clone());
                if let Some(why) = entry.unreadable {
                    result.unreadable.push(why);
                    continue;
                }
                let Some(resource) = entry.resource else {
                    continue;
                };
                match mode.as_deref() {
                    Some("include") => result.included.push(resource),
                    Some("outcome") => result.outcomes.extend(resource.parse_as().ok()),
                    _ if resource.resource_type == OperationOutcome::TYPE => {
                        result.outcomes.extend(resource.parse_as().ok())
                    }
                    _ => result.resources.push(resource),
                }
            }
            let Some(next) = next else {
                break;
            };
            if next == url {
                // The server named a next page and it is this one: stop, and
                // say pages may be missing rather than loop.
                result.truncated = true;
                break;
            }
            if result.pages >= self.page_cap {
                result.truncated = true;
                break;
            }
            if origin_of(&next).as_deref() != Some(self.origin.as_str()) {
                return Err(CliError::Upstream(format!(
                    "the FHIR server at {} sent a next page on another server; not following it",
                    self.origin
                )));
            }
            url = next;
        }
        Ok(result)
    }

    /// A Binary by its reference: `Binary/<id>`, or an absolute URL on the
    /// base's origin (a DocumentReference's `attachment.url`). It is asked for
    /// as FHIR JSON, its content in base64 `data` ([`crate::Binary::bytes`]):
    /// a server that answers with the raw file instead is
    /// [`CliError::Upstream`].
    pub async fn binary(&self, reference: &str) -> Result<RawResource, CliError> {
        let url = self.resolve(reference)?;
        let path = url.split(['?', '#']).next().unwrap_or_default();
        let id = path.rsplit('/').next().unwrap_or_default();
        let parent = path
            .rsplit('/')
            .nth(1)
            .ok_or_else(|| CliError::Usage("a Binary reference names no id".into()))?;
        if parent != "Binary" {
            return Err(CliError::Usage(
                "a Binary reference must name Binary/<id>".into(),
            ));
        }
        check_id(id)?;
        let response = self.get("Binary", url).await?;
        if let Some(kind) = response.header_value("content-type") {
            let kind = kind.to_ascii_lowercase();
            if !kind.contains("json") {
                return Err(CliError::Upstream(format!(
                    "the FHIR server at {} answered a Binary with {}, not FHIR JSON",
                    self.origin,
                    crate::outcome::printable(kind.split(';').next().unwrap_or_default())
                )));
            }
        }
        self.resource("Binary", &response.body)
    }

    /// `GET` a path under the base (`metadata`, `Patient/p1/$everything`), or
    /// an absolute URL on the base's origin, and return the body of a 2xx.
    pub async fn get_raw(&self, path: &str) -> Result<String, CliError> {
        let url = self.resolve(path)?;
        let what = url
            .strip_prefix(&self.base)
            .unwrap_or(&url)
            .trim_start_matches('/')
            .split(['/', '?'])
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("the base")
            .to_string();
        Ok(self.get(&what, url).await?.body)
    }

    fn resolve(&self, reference: &str) -> Result<String, CliError> {
        let reference = reference.trim();
        if reference.contains("://") {
            if origin_of(reference).as_deref() != Some(self.origin.as_str()) {
                return Err(CliError::Upstream(format!(
                    "a reference points at another server than {}; not following it",
                    self.origin
                )));
            }
            let path = reference
                .split_once("://")
                .and_then(|(_, rest)| rest.find('/').map(|i| &rest[i..]))
                .unwrap_or_default();
            if has_dot_segment(path) {
                return Err(CliError::Usage(
                    "a FHIR reference may not climb with `.` or `..`".into(),
                ));
            }
            return Ok(reference.to_string());
        }
        if reference.is_empty() || reference.starts_with(['#', '/']) || has_dot_segment(reference) {
            return Err(CliError::Usage(
                "a FHIR path must be relative to the base, as `Binary/<id>`".into(),
            ));
        }
        Ok(format!("{}/{reference}", self.base))
    }

    fn resource(&self, resource_type: &str, body: &str) -> Result<RawResource, CliError> {
        if let Some(err) = self.outcome_instead(resource_type, body) {
            return Err(err);
        }
        let resource = RawResource::parse(body)?;
        if resource.resource_type != resource_type {
            return Err(CliError::Upstream(format!(
                "the FHIR server at {} answered a {resource_type} with a {}",
                self.origin, resource.resource_type
            )));
        }
        Ok(resource)
    }

    /// An OperationOutcome sent with a 2xx in place of what was asked for.
    fn outcome_instead(&self, resource_type: &str, body: &str) -> Option<CliError> {
        if resource_type == OperationOutcome::TYPE {
            return None;
        }
        let raw = RawResource::parse(body).ok()?;
        if raw.resource_type != OperationOutcome::TYPE {
            return None;
        }
        let outcome: OperationOutcome = raw.parse_as().ok()?;
        Some(
            outcome_error(resource_type, &self.origin, &outcome).unwrap_or_else(|| {
                CliError::Upstream(format!(
                    "the FHIR server at {} answered {resource_type} with an OperationOutcome ({})",
                    self.origin,
                    outcome.summary()
                ))
            }),
        )
    }

    /// One GET, with its bearer, its retries and at most one renewal.
    async fn get(&self, what: &str, url: String) -> Result<HttpResponse, CliError> {
        let mut attempt = 1;
        // A token the source gave after a 401, for the one request again.
        let mut renewed: Option<Secret> = None;
        let mut renewal_spent = false;
        loop {
            let mut request = HttpRequest::get(url.clone())
                .header("Accept", FHIR_JSON)
                .user_agent(&self.binary, &self.version);
            for (name, value) in &self.headers {
                request = request.header(name.clone(), value.clone());
            }
            let sourced = match &self.bearer {
                Bearer::None => None,
                Bearer::Fixed(token) => {
                    request = request.header("Authorization", format!("Bearer {}", token.expose()));
                    None
                }
                Bearer::Source(source) => {
                    let token = match renewed.take() {
                        Some(token) => token,
                        None => source(None)?,
                    };
                    request = request.header("Authorization", format!("Bearer {}", token.expose()));
                    Some(token)
                }
            };
            let response = match self.transport.send(request).await {
                Ok(response) => response,
                // A connection that failed (exit 5) is asked again like a 5xx;
                // a refusal before sending (exit 2: an insecure URL) is not.
                Err(e) if e.exit_code() == 5 => {
                    if attempt < self.retry.attempts {
                        let wait = self.backoff(attempt);
                        self.report_retry(
                            &format!(
                                "could not reach the FHIR server at {} for {what}",
                                self.origin
                            ),
                            wait,
                            attempt,
                        );
                        (self.sleeper)(wait);
                        attempt += 1;
                        continue;
                    }
                    return Err(if e.reason().is_some() {
                        e
                    } else {
                        e.because(UNREACHABLE)
                    });
                }
                Err(e) => return Err(e),
            };
            if response.is_success() {
                return Ok(response);
            }
            if response.status == 401 && !renewal_spent {
                if let (Bearer::Source(source), Some(refused)) = (&self.bearer, &sourced) {
                    renewal_spent = true;
                    let fresh = source(Some(refused))?;
                    if fresh.expose() != refused.expose() {
                        renewed = Some(fresh);
                        continue;
                    }
                }
            }
            let now = (self.clock)();
            if attempt < self.retry.attempts {
                if let Some(inconclusive) = response.inconclusive(now) {
                    let wait = inconclusive
                        .retry_after
                        .unwrap_or_else(|| self.backoff(attempt));
                    if wait <= self.retry.max_wait {
                        self.report_retry(
                            &format!(
                                "the FHIR server at {} answered HTTP {} for {what}",
                                self.origin, response.status
                            ),
                            wait,
                            attempt,
                        );
                        (self.sleeper)(wait);
                        attempt += 1;
                        continue;
                    }
                }
            }
            return Err(status_error(what, &self.origin, &response, now));
        }
    }

    fn backoff(&self, attempt: u32) -> Duration {
        self.retry.delay * 2u32.saturating_pow(attempt - 1)
    }

    fn report_retry(&self, what_happened: &str, wait: Duration, attempt: u32) {
        if let Some(report) = &self.on_retry {
            let secs = wait.as_secs_f64().ceil() as u64;
            report(&format!(
                "{what_happened}; asking again in {secs} s ({} of {})",
                attempt + 1,
                self.retry.attempts
            ));
        }
    }
}

impl<T> std::fmt::Debug for FhirClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FhirClient")
            .field("base", &self.base)
            .field(
                "bearer",
                &match self.bearer {
                    Bearer::None => "none",
                    Bearer::Fixed(_) => "fixed",
                    Bearer::Source(_) => "source",
                },
            )
            .field(
                "headers",
                &self.headers.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            )
            .field("page_cap", &self.page_cap)
            .field("retry", &self.retry)
            .finish_non_exhaustive()
    }
}

/// `scheme://authority`, lower case, of an absolute URL.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() {
        return None;
    }
    Some(format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase()
    ))
}

fn check_type(resource_type: &str) -> Result<(), CliError> {
    let ok = resource_type
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
        && resource_type.chars().all(|c| c.is_ascii_alphanumeric());
    if ok {
        Ok(())
    } else {
        Err(CliError::Usage(format!(
            "not a FHIR resource type: {}",
            crate::outcome::printable(resource_type)
        )))
    }
}

/// FHIR's `id`: 1 to 64 of `A-Z a-z 0-9 - .`.
/// Whether a path's segments, before its query, include `.` or `..`, spelled
/// out or percent-encoded (`%2e%2e`): a URL parser resolves both.
fn has_dot_segment(path: &str) -> bool {
    path.split(['?', '#'])
        .next()
        .unwrap_or_default()
        .split('/')
        .any(|segment| {
            let decoded = segment.replace("%2e", ".").replace("%2E", ".");
            decoded == "." || decoded == ".."
        })
}

fn check_id(id: &str) -> Result<(), CliError> {
    let ok = (1..=64).contains(&id.len())
        && !id.chars().all(|c| c == '.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    if ok {
        Ok(())
    } else {
        Err(CliError::Usage(
            "not a FHIR id: 1 to 64 of A-Z a-z 0-9 - .".into(),
        ))
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pk_cli_core::{block_on, Reason};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// Answers from a queue, keeping each request.
    #[derive(Default)]
    struct Canned {
        answers: RefCell<VecDeque<HttpResponse>>,
        seen: RefCell<Vec<HttpRequest>>,
    }

    impl Canned {
        fn with(answers: Vec<HttpResponse>) -> Self {
            Canned {
                answers: RefCell::new(answers.into()),
                seen: RefCell::default(),
            }
        }
    }

    impl Transport for Canned {
        async fn send(&self, request: HttpRequest) -> Result<HttpResponse, CliError> {
            self.seen.borrow_mut().push(request);
            self.answers
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| CliError::Other("no answer left".into()))
        }
    }

    const BASE: &str = "https://fhir.example.test/r4";

    fn client(answers: Vec<HttpResponse>) -> FhirClient<Canned> {
        FhirClient::new(Canned::with(answers), BASE, "pk-test", "0.0.0")
            .unwrap()
            .bearer(Secret::new("synthetic-token"))
            .sleeper(|_| {})
    }

    fn json(body: &str) -> HttpResponse {
        HttpResponse::new(200, body).header("content-type", "application/fhir+json")
    }

    fn page(entries: &str, next: Option<&str>) -> HttpResponse {
        let link = next
            .map(|n| format!(r#","link":[{{"relation":"next","url":"{n}"}}]"#))
            .unwrap_or_default();
        json(&format!(
            r#"{{"resourceType":"Bundle","type":"searchset","total":3,"entry":[{entries}]{link}}}"#
        ))
    }

    fn obs(id: &str) -> String {
        format!(
            r#"{{"resource":{{"resourceType":"Observation","id":"{id}","valueQuantity":{{"value":1.50}}}},"search":{{"mode":"match"}}}}"#
        )
    }

    #[test]
    fn a_read_sends_the_bearer_and_accept_and_keeps_the_text() {
        let c = client(vec![json(
            r#"{"resourceType":"Patient","id":"p1","birthDate":"1970-01-01"}"#,
        )])
        .header("Epic-Client-ID", "synthetic-client");
        let p = pk_cli_core::block_on(c.read("Patient", "p1")).unwrap();
        assert_eq!(p.id, "p1");
        let seen = c.transport().seen.borrow();
        assert_eq!(seen[0].url, format!("{BASE}/Patient/p1"));
        assert_eq!(seen[0].header_value("accept"), Some(FHIR_JSON));
        assert_eq!(
            seen[0].header_value("authorization"),
            Some("Bearer synthetic-token")
        );
        assert_eq!(
            seen[0].header_value("epic-client-id"),
            Some("synthetic-client")
        );
    }

    #[test]
    fn a_search_follows_next_and_sorts_entries_by_mode() {
        let c = client(vec![
            page(
                &format!(
                    r#"{},{{"resource":{{"resourceType":"Practitioner","id":"d1"}},"search":{{"mode":"include"}}}},{{"resource":{{"resourceType":"OperationOutcome","issue":[{{"severity":"warning","code":"informational"}}]}},"search":{{"mode":"outcome"}}}}"#,
                    obs("o1")
                ),
                Some("https://fhir.example.test/r4?page=2"),
            ),
            page(&format!("{},{}", obs("o2"), obs("o3")), None),
        ]);
        let r = pk_cli_core::block_on(c.search(
            "Observation",
            &[("patient", "p1".into()), ("_count", "1".into())],
        ))
        .unwrap();
        assert_eq!(r.resources.len(), 3);
        assert_eq!(r.included.len(), 1);
        assert_eq!(r.outcomes.len(), 1);
        assert_eq!((r.pages, r.truncated, r.total), (2, false, Some(3)));
        assert!(r.resources[0].raw.contains("1.50"));
        let seen = c.transport().seen.borrow();
        assert_eq!(
            seen[0].url,
            format!("{BASE}/Observation?patient=p1&_count=1")
        );
        assert_eq!(seen[1].url, "https://fhir.example.test/r4?page=2");
    }

    #[test]
    fn the_page_cap_stops_and_says_so() {
        let next = Some("https://fhir.example.test/r4?page=2");
        let c = client(vec![page(&obs("o1"), next), page(&obs("o2"), next)]).page_cap(1);
        let r = pk_cli_core::block_on(c.search("Observation", &[])).unwrap();
        assert_eq!((r.pages, r.truncated, r.resources.len()), (1, true, 1));
    }

    #[test]
    fn a_next_page_on_another_server_is_refused() {
        let c = client(vec![page(
            &obs("o1"),
            Some("https://elsewhere.test/r4?page=2"),
        )]);
        let err = pk_cli_core::block_on(c.search("Observation", &[])).unwrap_err();
        assert_eq!(err.exit_code(), 5);
        assert_eq!(c.transport().seen.borrow().len(), 1);
    }

    #[test]
    fn a_busy_server_is_asked_again_within_the_policy() {
        let busy = || HttpResponse::new(429, "").header("retry-after", "1");
        let c = client(vec![
            busy(),
            json(r#"{"resourceType":"Patient","id":"p1"}"#),
        ]);
        assert!(pk_cli_core::block_on(c.read("Patient", "p1")).is_ok());
        let c = client(vec![busy(), busy(), busy()]);
        let err = pk_cli_core::block_on(c.read("Patient", "p1")).unwrap_err();
        assert_eq!(err.exit_code(), 5);
        assert_eq!(c.transport().seen.borrow().len(), 3);
        let long = HttpResponse::new(503, "").header("retry-after", "3600");
        let c = client(vec![long]);
        assert!(pk_cli_core::block_on(c.read("Patient", "p1")).is_err());
        assert_eq!(c.transport().seen.borrow().len(), 1);
        let c = client(vec![HttpResponse::new(404, "")]);
        assert_eq!(
            pk_cli_core::block_on(c.read("Patient", "p1"))
                .unwrap_err()
                .exit_code(),
            4
        );
        assert_eq!(c.transport().seen.borrow().len(), 1);
    }

    #[test]
    fn an_outcome_in_place_of_a_resource_is_its_error() {
        let c = client(vec![json(
            r#"{"resourceType":"OperationOutcome","issue":[{"severity":"error","code":"forbidden"}]}"#,
        )]);
        let err = pk_cli_core::block_on(c.search("Coverage", &[])).unwrap_err();
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn a_binary_is_asked_for_as_json_on_this_server_only() {
        let c = client(vec![json(
            r#"{"resourceType":"Binary","id":"b1","contentType":"application/pdf","data":"JVBERi0="}"#,
        )]);
        let b = pk_cli_core::block_on(c.binary("https://FHIR.example.test/r4/Binary/b1")).unwrap();
        assert_eq!(
            b.parse_as::<crate::Binary>().unwrap().bytes().unwrap(),
            b"%PDF-"
        );
        for bad in [
            "https://elsewhere.test/r4/Binary/b1",
            "Patient/p1",
            "/Binary/b1",
            "Binary/../x",
        ] {
            assert!(pk_cli_core::block_on(c.binary(bad)).is_err(), "{bad}");
        }
        let c = client(vec![
            HttpResponse::new(200, "%PDF-1.4").header("content-type", "application/pdf")
        ]);
        let err = pk_cli_core::block_on(c.binary("Binary/b1")).unwrap_err();
        assert!(err.to_string().contains("application/pdf"), "{err}");
    }

    #[test]
    fn bad_names_are_usage_errors_before_any_request() {
        let c = client(vec![]);
        for (t, id) in [
            ("patient", "p1"),
            ("Patient", "p/1"),
            ("Patient", ""),
            ("Pa tient", "p"),
        ] {
            assert_eq!(
                pk_cli_core::block_on(c.read(t, id))
                    .unwrap_err()
                    .exit_code(),
                2
            );
        }
        assert!(c.transport().seen.borrow().is_empty());
        assert!(FhirClient::new(Canned::default(), "http://fhir.example.test", "b", "v").is_err());
    }

    #[test]
    fn debug_shows_no_token() {
        let c = client(vec![]);
        assert!(!format!("{c:?}").contains("synthetic-token"));
    }

    #[test]
    fn a_dot_segment_is_refused_however_it_is_spelled() {
        let c = client(vec![]);
        for path in [
            "../admin",
            "%2e%2e/%2e%2e/admin/x",
            "Patient/%2E/x",
            "https://fhir.example.test/r4/Patient/p1/Binary/..",
        ] {
            let err = block_on(c.get_raw(path)).unwrap_err();
            assert_eq!(err.exit_code(), 2, "{path}");
        }
        let err =
            block_on(c.binary("https://fhir.example.test/r4/Patient/p1/Binary/..")).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        for id in [".", ".."] {
            assert_eq!(
                block_on(c.read("Patient", id)).unwrap_err().exit_code(),
                2,
                "{id}"
            );
        }
        assert!(c.transport().seen.borrow().is_empty(), "nothing was sent");
        assert!(check_id("a..b").is_ok(), "dots inside an id are allowed");
    }

    #[test]
    fn a_warning_never_changes_the_exit_code() {
        let warning = |status: u16, code: &str| {
            HttpResponse::new(
                status,
                format!(
                    r#"{{"resourceType":"OperationOutcome","issue":[{{"severity":"warning","code":"{code}"}}]}}"#
                ),
            )
            .header("content-type", "application/fhir+json")
        };
        let err =
            block_on(client(vec![warning(400, "not-found")]).read("Patient", "p1")).unwrap_err();
        assert_eq!(
            (err.exit_code(), err.reason()),
            (5, Some(Reason::BadRequest))
        );
        let err =
            block_on(client(vec![warning(404, "suppressed")]).read("Patient", "p1")).unwrap_err();
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn a_next_link_to_the_same_page_ends_the_search_as_truncated() {
        let same = format!("{BASE}/Observation?patient=p");
        let c = client(vec![page(&obs("o1"), Some(&same))]);
        let r = block_on(c.search("Observation", &[("patient", "p".into())])).unwrap();
        assert_eq!((r.resources.len(), r.pages, r.truncated), (1, 1, true));
    }

    /// Fails to connect once, then answers.
    struct Flaky {
        failures: RefCell<u32>,
        sent: RefCell<u32>,
    }

    impl Transport for Flaky {
        async fn send(&self, _: HttpRequest) -> Result<HttpResponse, CliError> {
            *self.sent.borrow_mut() += 1;
            let mut failures = self.failures.borrow_mut();
            if *failures > 0 {
                *failures -= 1;
                return Err(CliError::Upstream("connection reset".into()));
            }
            Ok(json(r#"{"resourceType":"Patient","id":"p1"}"#))
        }
    }

    #[test]
    fn a_failed_connection_is_asked_again_within_the_policy() {
        let flaky = |failures| {
            FhirClient::new(
                Flaky {
                    failures: RefCell::new(failures),
                    sent: RefCell::new(0),
                },
                BASE,
                "pk-test",
                "0",
            )
            .unwrap()
            .sleeper(|_| {})
        };
        let c = flaky(2);
        assert!(block_on(c.read("Patient", "p1")).is_ok());
        assert_eq!(*c.transport().sent.borrow(), 3);
        let c = flaky(3);
        assert_eq!(
            block_on(c.read("Patient", "p1")).unwrap_err().exit_code(),
            5
        );
        assert_eq!(*c.transport().sent.borrow(), 3, "3 requests at most");
        let err = block_on(flaky(3).read("Patient", "p1")).unwrap_err();
        assert_eq!(err.reason(), Some(UNREACHABLE));

        let answered = client(vec![
            HttpResponse::new(503, ""),
            HttpResponse::new(503, ""),
            HttpResponse::new(503, ""),
        ]);
        let err = block_on(answered.read("Patient", "p1")).unwrap_err();
        assert_eq!(err.exit_code(), 5);
        assert_eq!(
            err.reason(),
            None,
            "a server that answered is not unreachable"
        );
    }

    type Calls = Arc<Mutex<Vec<Option<String>>>>;

    fn sourced(answers: Vec<HttpResponse>, renew_to: &'static str) -> (FhirClient<Canned>, Calls) {
        let calls: Calls = Arc::default();
        let log = calls.clone();
        let renewed = Arc::new(AtomicBool::new(false));
        let c = FhirClient::new(Canned::with(answers), BASE, "pk-test", "0")
            .unwrap()
            .sleeper(|_| {})
            .bearer_with(move |refused| {
                log.lock()
                    .unwrap()
                    .push(refused.map(|t| t.expose().to_string()));
                if refused.is_some() {
                    renewed.store(true, Ordering::SeqCst);
                }
                Ok(Secret::new(if renewed.load(Ordering::SeqCst) {
                    renew_to
                } else {
                    "token-a"
                }))
            });
        (c, calls)
    }

    fn bearers(c: &FhirClient<Canned>) -> Vec<String> {
        c.transport()
            .seen
            .borrow()
            .iter()
            .map(|r| r.header_value("authorization").unwrap_or("").to_string())
            .collect()
    }

    const PATIENT: &str = r#"{"resourceType":"Patient","id":"p1"}"#;

    #[test]
    fn a_token_source_is_asked_per_request_and_renews_once_after_a_401() {
        let (c, calls) = sourced(
            vec![HttpResponse::new(401, ""), json(PATIENT), json(PATIENT)],
            "token-b",
        );
        block_on(c.read("Patient", "p1")).unwrap();
        block_on(c.read("Patient", "p1")).unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec![None, Some("token-a".to_string()), None]
        );
        assert_eq!(
            bearers(&c),
            ["Bearer token-a", "Bearer token-b", "Bearer token-b"]
        );
    }

    #[test]
    fn the_source_is_asked_again_for_each_retry_and_a_renewed_token_survives_one() {
        let (c, calls) = sourced(vec![HttpResponse::new(429, ""), json(PATIENT)], "token-b");
        block_on(c.read("Patient", "p1")).unwrap();
        assert_eq!(*calls.lock().unwrap(), vec![None, None]);

        let (c, calls) = sourced(
            vec![
                HttpResponse::new(401, ""),
                HttpResponse::new(503, ""),
                json(PATIENT),
            ],
            "token-b",
        );
        block_on(c.read("Patient", "p1")).unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec![None, Some("token-a".to_string()), None]
        );
        assert_eq!(
            bearers(&c),
            ["Bearer token-a", "Bearer token-b", "Bearer token-b"]
        );
    }

    /// A transport whose failure already carries a reason.
    struct Typed;

    impl Transport for Typed {
        async fn send(&self, _: HttpRequest) -> Result<HttpResponse, CliError> {
            Err(CliError::Upstream("blocked".into()).because(Reason::ClientBlocked))
        }
    }

    #[test]
    fn a_reason_the_transport_set_is_kept() {
        let c = FhirClient::new(Typed, BASE, "pk-test", "0")
            .unwrap()
            .sleeper(|_| {});
        let err = block_on(c.read("Patient", "p1")).unwrap_err();
        assert_eq!(err.reason(), Some(Reason::ClientBlocked));
    }

    #[test]
    fn a_client_is_send_and_sync_whatever_its_bearer() {
        fn send_sync<C: Send + Sync>(_: &C) {}
        let c = FhirClient::new(BlockingStub, BASE, "pk-test", "0")
            .unwrap()
            .bearer_with(|_| Ok(Secret::new("t")))
            .on_retry(|_| {});
        send_sync(&c);
    }

    /// A transport that is itself `Send + Sync`, unlike the test doubles
    /// above, which keep their state in `RefCell`s.
    struct BlockingStub;

    impl Transport for BlockingStub {
        async fn send(&self, _: HttpRequest) -> Result<HttpResponse, CliError> {
            Ok(json(PATIENT))
        }
    }

    #[test]
    fn a_401_the_source_cannot_renew_is_exit_3_after_one_request() {
        let (c, calls) = sourced(vec![HttpResponse::new(401, "")], "token-a");
        let err = block_on(c.read("Patient", "p1")).unwrap_err();
        assert_eq!(
            (err.exit_code(), err.reason()),
            (3, Some(Reason::CredentialRefused))
        );
        assert_eq!(c.transport().seen.borrow().len(), 1);
        assert_eq!(calls.lock().unwrap().len(), 2);

        // A renewed token refused too: no second renewal, no loop.
        let (c, calls) = sourced(
            vec![HttpResponse::new(401, ""), HttpResponse::new(401, "")],
            "token-b",
        );
        assert_eq!(
            block_on(c.read("Patient", "p1")).unwrap_err().exit_code(),
            3
        );
        assert_eq!(c.transport().seen.borrow().len(), 2);
        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_token_source_error_is_the_request_error_and_nothing_is_sent() {
        let c = FhirClient::new(Canned::with(vec![json(PATIENT)]), BASE, "pk-test", "0")
            .unwrap()
            .bearer_with(|_| {
                Err(CliError::Auth("not signed in".into()).because(Reason::NoCredential))
            });
        let err = block_on(c.read("Patient", "p1")).unwrap_err();
        assert_eq!(err.reason(), Some(Reason::NoCredential));
        assert!(c.transport().seen.borrow().is_empty());
    }

    #[test]
    fn a_fixed_bearer_is_not_renewed() {
        let c = client(vec![HttpResponse::new(401, ""), json(PATIENT)]);
        assert_eq!(
            block_on(c.read("Patient", "p1")).unwrap_err().exit_code(),
            3
        );
        assert_eq!(c.transport().seen.borrow().len(), 1);
    }

    #[test]
    fn each_wait_is_reported_without_the_query() {
        let lines: Arc<Mutex<Vec<String>>> = Arc::default();
        let log = lines.clone();
        let c = client(vec![
            HttpResponse::new(429, "").header("retry-after", "2"),
            HttpResponse::new(503, ""),
            page(&obs("o1"), None),
        ])
        .on_retry(move |line| log.lock().unwrap().push(line.to_string()));
        block_on(c.search("Observation", &[("patient", "p1".into())])).unwrap();
        assert_eq!(
            *lines.lock().unwrap(),
            [
                "the FHIR server at https://fhir.example.test answered HTTP 429 for Observation; asking again in 2 s (2 of 3)",
                "the FHIR server at https://fhir.example.test answered HTTP 503 for Observation; asking again in 2 s (3 of 3)",
            ]
        );

        let log = lines.clone();
        lines.lock().unwrap().clear();
        let flaky = FhirClient::new(
            Flaky {
                failures: RefCell::new(1),
                sent: RefCell::new(0),
            },
            BASE,
            "pk-test",
            "0",
        )
        .unwrap()
        .sleeper(|_| {})
        .on_retry(move |line| log.lock().unwrap().push(line.to_string()));
        block_on(flaky.read("Patient", "p1")).unwrap();
        assert_eq!(
            *lines.lock().unwrap(),
            ["could not reach the FHIR server at https://fhir.example.test for Patient; asking again in 1 s (2 of 3)"]
        );
    }

    #[test]
    fn an_entry_that_does_not_read_costs_only_itself() {
        let entries = [
            obs("o1"),
            r#"{"resource":{"id":"111111111"}}"#.to_string(),
            r#"{"resource":["Observation"]}"#.to_string(),
            obs("o2"),
        ]
        .join(",");
        let c = client(vec![page(&entries, None)]);
        let r = block_on(c.search("Observation", &[])).unwrap();
        let ids: Vec<_> = r.resources.iter().map(|r| r.id.clone()).collect();
        assert_eq!(ids, ["o1", "o2"]);
        assert_eq!(r.unreadable.len(), 2, "{:?}", r.unreadable);
        assert!(r.unreadable.iter().all(|why| !why.contains("111111111")));
    }
}
