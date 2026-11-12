//! A loopback mock SMART + FHIR server and the synthetic patient it serves,
//! for this workspace's tests and its consumers' (feature `test-util`).
//!
//! [`MockServer::start`] listens on `127.0.0.1` on a port the system picks and
//! serves, under [`MockServer::fhir_base`]:
//!
//! | Path | Answer |
//! |---|---|
//! | `GET /fhir/.well-known/smart-configuration` | SMART discovery |
//! | `GET /fhir/metadata` | a CapabilityStatement with the `oauth-uris` extension |
//! | `GET /authorize` | a 302 to the loopback redirect with `code`, `state` and `iss`; PKCE S256 required |
//! | `POST /token` | the authorization-code grant (the verifier checked), the refresh grant (rotating), the JWT-bearer grant |
//! | `POST /register` | dynamic registration (RFC 7591) |
//! | `GET /fhir/<Type>/<id>`, `GET /fhir/<Type>?…` | the synthetic patient's resources, searches paged by `_page` |
//! | `GET /fhir/Binary/<id>` | FHIR JSON for `Accept: application/fhir+json`, the PDF otherwise |
//!
//! FHIR paths need `Authorization: Bearer` with a token the server issued
//! ([`MockServer::access_token`] issues one directly), or answer 401 with an
//! OperationOutcome. [`Fault`] makes chosen requests fail (a 429 with
//! `Retry-After`, an OperationOutcome error); [`MockServer::requests`] shows
//! what was sent. Every value is synthetic (`fixtures/README.md`).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::raw::RawResource;

/// The synthetic patient's FHIR id.
pub const PATIENT_ID: &str = "pat-0001";
/// The registered client the mock accepts, public or with [`CLIENT_SECRET`].
pub const CLIENT_ID: &str = "synthetic-client";
/// [`CLIENT_ID`]'s secret, for `client_secret_basic`.
pub const CLIENT_SECRET: &str = "synthetic-secret";
/// The client id `/register` issues.
pub const DYNAMIC_CLIENT_ID: &str = "synthetic-dynamic-client";
/// Search results per page unless [`MockServer::page_size`] says otherwise.
pub const DEFAULT_PAGE_SIZE: usize = 2;
/// The PDF the synthetic Binary carries.
pub const PDF: &[u8] = include_bytes!("../fixtures/synthetic/document.pdf");

/// The synthetic patient's resources by type, in the order a pull writes them.
const SYNTHETIC: &[(&str, &str)] = &[
    (
        "Patient",
        include_str!("../fixtures/synthetic/Patient.json"),
    ),
    (
        "Organization",
        include_str!("../fixtures/synthetic/Organization.json"),
    ),
    (
        "Practitioner",
        include_str!("../fixtures/synthetic/Practitioner.json"),
    ),
    (
        "Encounter",
        include_str!("../fixtures/synthetic/Encounter.json"),
    ),
    (
        "Condition",
        include_str!("../fixtures/synthetic/Condition.json"),
    ),
    (
        "Observation",
        include_str!("../fixtures/synthetic/Observation.json"),
    ),
    (
        "DiagnosticReport",
        include_str!("../fixtures/synthetic/DiagnosticReport.json"),
    ),
    (
        "DocumentReference",
        include_str!("../fixtures/synthetic/DocumentReference.json"),
    ),
    ("Binary", include_str!("../fixtures/synthetic/Binary.json")),
    (
        "MedicationRequest",
        include_str!("../fixtures/synthetic/MedicationRequest.json"),
    ),
    (
        "MedicationStatement",
        include_str!("../fixtures/synthetic/MedicationStatement.json"),
    ),
    (
        "AllergyIntolerance",
        include_str!("../fixtures/synthetic/AllergyIntolerance.json"),
    ),
    (
        "Immunization",
        include_str!("../fixtures/synthetic/Immunization.json"),
    ),
    (
        "Procedure",
        include_str!("../fixtures/synthetic/Procedure.json"),
    ),
    (
        "ImagingStudy",
        include_str!("../fixtures/synthetic/ImagingStudy.json"),
    ),
    (
        "Coverage",
        include_str!("../fixtures/synthetic/Coverage.json"),
    ),
    (
        "ExplanationOfBenefit",
        include_str!("../fixtures/synthetic/ExplanationOfBenefit.json"),
    ),
    (
        "Provenance",
        include_str!("../fixtures/synthetic/Provenance.json"),
    ),
];

const SMART_CONFIGURATION: &str = include_str!("../fixtures/smart-configuration.json");
const CAPABILITY_STATEMENT: &str = include_str!("../fixtures/capability-statement.json");
const OUTCOME_LOGIN: &str = include_str!("../fixtures/operation-outcome-login.json");
const OUTCOME_NOT_FOUND: &str = include_str!("../fixtures/operation-outcome-not-found.json");
const OUTCOME_NOT_SUPPORTED: &str =
    include_str!("../fixtures/operation-outcome-not-supported.json");

/// The resource types the synthetic patient has, in pull order.
pub fn synthetic_types() -> Vec<&'static str> {
    SYNTHETIC.iter().map(|(t, _)| *t).collect()
}

/// The synthetic patient's resources of one type, each as its fixture text
/// minified (decimals as written).
pub fn synthetic(resource_type: &str) -> Vec<RawResource> {
    SYNTHETIC
        .iter()
        .filter(|(t, _)| *t == resource_type)
        .flat_map(|(_, text)| split_array(text))
        .map(|text| RawResource::parse(&text).expect("a synthetic fixture reads"))
        .collect()
}

/// Every synthetic resource, in pull order.
pub fn synthetic_all() -> Vec<RawResource> {
    synthetic_types().into_iter().flat_map(synthetic).collect()
}

/// The elements of a JSON array's text, each as its own text, so a decimal
/// keeps its spelling.
fn split_array(text: &str) -> Vec<String> {
    let items: Vec<Box<serde_json::value::RawValue>> =
        serde_json::from_str(text).expect("a fixture is a JSON array");
    items.into_iter().map(|r| r.get().to_string()).collect()
}

/// A request the mock received. Header names are lower case.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    /// Path and query, as sent.
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The path, without the query.
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }
}

/// An answer the mock gives in place of the real one, to the next `times`
/// requests whose method matches and whose target starts with `prefix`.
#[derive(Debug, Clone)]
pub struct Fault {
    pub method: String,
    pub prefix: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub times: u32,
}

impl Fault {
    pub fn new(method: &str, prefix: &str, status: u16, body: impl Into<String>) -> Self {
        Fault {
            method: method.to_string(),
            prefix: prefix.to_string(),
            status,
            headers: vec![("Content-Type".into(), "application/fhir+json".into())],
            body: body.into(),
            times: 1,
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn times(mut self, times: u32) -> Self {
        self.times = times;
        self
    }

    /// A 429 asking to wait `seconds`.
    pub fn too_many_requests(prefix: &str, seconds: u64) -> Self {
        Fault::new("GET", prefix, 429, "").header("Retry-After", &seconds.to_string())
    }

    /// A 400 with an OperationOutcome `not-supported`.
    pub fn not_supported(prefix: &str) -> Self {
        Fault::new("GET", prefix, 400, OUTCOME_NOT_SUPPORTED)
    }

    /// A 401 with an OperationOutcome `login`.
    pub fn unauthorized(prefix: &str) -> Self {
        Fault::new("GET", prefix, 401, OUTCOME_LOGIN)
    }

    /// A 503 with no body.
    pub fn unavailable(prefix: &str) -> Self {
        Fault::new("GET", prefix, 503, "")
    }
}

struct Grant {
    challenge: String,
    redirect_uri: String,
    client_id: String,
    scope: String,
}

#[derive(Default)]
struct State {
    issued: u32,
    access: Vec<String>,
    refresh: HashMap<String, String>,
    codes: HashMap<String, Grant>,
    faults: Vec<Fault>,
    requests: Vec<Recorded>,
    page_size: usize,
    deny_next: bool,
}

/// The mock server. It stops when dropped.
pub struct MockServer {
    origin: String,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MockServer {
    pub fn start() -> MockServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the mock server");
        let port = listener.local_addr().expect("a local address").port();
        let origin = format!("http://127.0.0.1:{port}");
        let state = Arc::new(Mutex::new(State {
            page_size: DEFAULT_PAGE_SIZE,
            ..State::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let (state, stop, origin) = (Arc::clone(&state), Arc::clone(&stop), origin.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let (state, origin) = (Arc::clone(&state), origin.clone());
                    std::thread::spawn(move || serve(stream, &state, &origin));
                }
            })
        };
        MockServer {
            origin,
            state,
            stop,
            handle: Some(handle),
        }
    }

    /// `http://127.0.0.1:<port>`.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// `http://127.0.0.1:<port>/fhir`, the FHIR base and the SMART issuer.
    pub fn fhir_base(&self) -> String {
        format!("{}/fhir", self.origin)
    }

    /// A fresh access token the FHIR paths accept, as if a sign-in had issued
    /// it.
    pub fn access_token(&self) -> String {
        let mut state = self.state.lock().unwrap();
        state.issued += 1;
        let token = format!("synthetic-access-{}", state.issued);
        state.access.push(token.clone());
        token
    }

    /// Answer as `fault` says, before any other route.
    pub fn fail(&self, fault: Fault) {
        self.state.lock().unwrap().faults.push(fault);
    }

    /// Search results per page.
    pub fn page_size(&self, size: usize) {
        self.state.lock().unwrap().page_size = size.max(1);
    }

    /// Answer the next authorization with `error=access_denied`, as when the
    /// person declines.
    pub fn deny_next_authorization(&self) {
        self.state.lock().unwrap().deny_next = true;
    }

    /// Every request so far.
    pub fn requests(&self) -> Vec<Recorded> {
        self.state.lock().unwrap().requests.clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.origin.trim_start_matches("http://"));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// What [`http_get`] got back. Header names are lower case.
#[derive(Debug, Clone)]
pub struct BrowserAnswer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl BrowserAnswer {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// A one-request HTTP/1.1 GET to `http://127.0.0.1…`, as a browser would send
/// it, following no redirect. For a test to play the browser in a sign-in: GET the
/// authorization URL, then GET its `location` to hand the code to the
/// loopback listener.
pub fn http_get(url: &str) -> std::io::Result<BrowserAnswer> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| std::io::Error::other("http_get takes an http:// URL"))?;
    let (authority, target) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let mut stream = TcpStream::connect(authority)?;
    stream.set_read_timeout(Some(Duration::from_secs(20)))?;
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    Ok(BrowserAnswer {
        status,
        headers,
        body: body.to_string(),
    })
}

struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Answer {
    fn json(status: u16, body: impl Into<String>) -> Self {
        Answer {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: body.into().into_bytes(),
        }
    }

    fn fhir(status: u16, body: impl Into<String>) -> Self {
        Answer {
            status,
            headers: vec![("Content-Type".into(), "application/fhir+json".into())],
            body: body.into().into_bytes(),
        }
    }

    fn redirect(location: String) -> Self {
        Answer {
            status: 302,
            headers: vec![("Location".into(), location)],
            body: Vec::new(),
        }
    }

    fn oauth_error(status: u16, error: &str) -> Self {
        Answer::json(status, json!({ "error": error }).to_string())
    }
}

fn serve(mut stream: TcpStream, state: &Mutex<State>, origin: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Some(request) = read_request(&mut stream) else {
        return;
    };
    let fault = {
        let mut state = state.lock().unwrap();
        state.requests.push(request.clone());
        let found = state.faults.iter_mut().find(|f| {
            f.times > 0 && f.method == request.method && request.target.starts_with(&f.prefix)
        });
        found.map(|f| {
            f.times -= 1;
            Answer {
                status: f.status,
                headers: f.headers.clone(),
                body: f.body.clone().into_bytes(),
            }
        })
    };
    let answer = fault.unwrap_or_else(|| route(&request, state, origin));
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        answer.status,
        reason(answer.status),
        answer.body.len()
    );
    for (name, value) in &answer.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&answer.body);
    let _ = stream.flush();
}

fn read_request(stream: &mut TcpStream) -> Option<Recorded> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        raw.extend_from_slice(&buf[..n]);
        if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if raw.len() > 64 * 1024 {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let target = first.next()?.to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = raw[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
    }
    Some(Recorded {
        method,
        target,
        headers,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}

fn route(request: &Recorded, state: &Mutex<State>, origin: &str) -> Answer {
    let base = format!("{origin}/fhir");
    let (path, query) = request
        .target
        .split_once('?')
        .unwrap_or((&request.target, ""));
    let params = parse_form(query);
    match (request.method.as_str(), path) {
        ("GET", "/fhir/.well-known/smart-configuration") => Answer::json(
            200,
            SMART_CONFIGURATION
                .replace("{{BASE}}", &base)
                .replace("{{ORIGIN}}", origin),
        ),
        ("GET", "/fhir/metadata") => {
            Answer::fhir(200, CAPABILITY_STATEMENT.replace("{{ORIGIN}}", origin))
        }
        ("GET", "/jwks") => Answer::json(200, r#"{"keys":[]}"#),
        ("GET", "/authorize") => authorize(&params, state, &base),
        ("POST", "/token") => token(request, state, &base),
        ("POST", "/register") => register(request, state),
        ("GET", p) if p.starts_with("/fhir/") => {
            if !bearer_ok(request, state) {
                return Answer::fhir(401, OUTCOME_LOGIN);
            }
            fhir(request, &p["/fhir/".len()..], query, &params, state, &base)
        }
        _ => Answer::fhir(404, OUTCOME_NOT_FOUND),
    }
}

fn one<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

fn authorize(params: &[(String, String)], state: &Mutex<State>, base: &str) -> Answer {
    let redirect = one(params, "redirect_uri").unwrap_or_default();
    if !(redirect.starts_with("http://127.0.0.1:") || redirect.starts_with("http://localhost:")) {
        return Answer::json(
            400,
            r#"{"error":"invalid_request","error_description":"redirect_uri"}"#,
        );
    }
    let sign_in_state = one(params, "state").unwrap_or_default();
    let back = |query: String| {
        let joiner = if redirect.contains('?') { '&' } else { '?' };
        Answer::redirect(format!(
            "{redirect}{joiner}{query}&state={}&iss={}",
            encode(sign_in_state),
            encode(base)
        ))
    };
    let client_id = one(params, "client_id").unwrap_or_default();
    let valid = one(params, "response_type") == Some("code")
        && [CLIENT_ID, DYNAMIC_CLIENT_ID].contains(&client_id)
        && one(params, "code_challenge_method") == Some("S256")
        && one(params, "code_challenge").is_some_and(|c| !c.is_empty())
        && !sign_in_state.is_empty()
        && one(params, "aud").is_none_or(|aud| aud == base);
    if !valid {
        return back("error=invalid_request".into());
    }
    let mut state = state.lock().unwrap();
    if std::mem::take(&mut state.deny_next) {
        return back("error=access_denied&error_description=the%20person%20declined".into());
    }
    state.issued += 1;
    let code = format!("synthetic-code-{}", state.issued);
    state.codes.insert(
        code.clone(),
        Grant {
            challenge: one(params, "code_challenge")
                .unwrap_or_default()
                .to_string(),
            redirect_uri: redirect.to_string(),
            client_id: client_id.to_string(),
            scope: one(params, "scope").unwrap_or_default().to_string(),
        },
    );
    back(format!("code={code}"))
}

fn token(request: &Recorded, state: &Mutex<State>, base: &str) -> Answer {
    let form = parse_form(&request.body);
    let basic = request
        .header("authorization")
        .and_then(|h| h.strip_prefix("Basic "))
        .map(|b| {
            base64::engine::general_purpose::STANDARD
                .decode(b.trim())
                .map(|d| String::from_utf8_lossy(&d).to_string())
                .unwrap_or_default()
        });
    let client_id = match basic {
        Some(pair) if pair == format!("{CLIENT_ID}:{CLIENT_SECRET}") => CLIENT_ID.to_string(),
        Some(_) => return Answer::oauth_error(401, "invalid_client"),
        None => one(&form, "client_id").unwrap_or_default().to_string(),
    };
    let mut state = state.lock().unwrap();
    match one(&form, "grant_type") {
        Some("authorization_code") => {
            let Some(grant) = one(&form, "code").and_then(|c| state.codes.remove(c)) else {
                return Answer::oauth_error(400, "invalid_grant");
            };
            let verifier = one(&form, "code_verifier").unwrap_or_default();
            let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(Sha256::digest(verifier.as_bytes()));
            if challenge != grant.challenge
                || one(&form, "redirect_uri") != Some(grant.redirect_uri.as_str())
                || client_id != grant.client_id
            {
                return Answer::oauth_error(400, "invalid_grant");
            }
            issue(&mut state, &grant.scope, &client_id, base, true)
        }
        Some("refresh_token") => {
            let Some(scope) = one(&form, "refresh_token").and_then(|r| state.refresh.remove(r))
            else {
                return Answer::oauth_error(400, "invalid_grant");
            };
            let scope = one(&form, "scope").map_or(scope, str::to_string);
            issue(&mut state, &scope, &client_id, base, true)
        }
        Some("urn:ietf:params:oauth:grant-type:jwt-bearer") => {
            let issuer = one(&form, "assertion")
                .and_then(|a| a.split('.').nth(1))
                .and_then(|p| {
                    base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .decode(p)
                        .ok()
                })
                .and_then(|p| serde_json::from_slice::<Value>(&p).ok())
                .and_then(|claims| {
                    claims
                        .get("iss")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            if issuer.as_deref() != Some(DYNAMIC_CLIENT_ID) {
                return Answer::oauth_error(400, "invalid_client");
            }
            let scope = one(&form, "scope").unwrap_or("patient/*.read").to_string();
            issue(&mut state, &scope, DYNAMIC_CLIENT_ID, base, false)
        }
        _ => Answer::oauth_error(400, "unsupported_grant_type"),
    }
}

fn issue(state: &mut State, scope: &str, client_id: &str, base: &str, refresh: bool) -> Answer {
    state.issued += 1;
    let n = state.issued;
    let access = format!("synthetic-access-{n}");
    state.access.push(access.clone());
    let mut body = json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": 3600,
        "scope": scope,
        "patient": PATIENT_ID,
    });
    if refresh && scope.split(' ').any(|s| s == "offline_access") {
        let refresh_token = format!("synthetic-refresh-{n}");
        state
            .refresh
            .insert(refresh_token.clone(), scope.to_string());
        body["refresh_token"] = refresh_token.into();
    }
    if scope.split(' ').any(|s| s == "openid") {
        body["id_token"] = id_token(base, client_id).into();
    }
    Answer::json(200, body.to_string())
}

/// An unsigned id_token naming the synthetic patient, built here rather than
/// kept in a fixture.
fn id_token(base: &str, client_id: &str) -> String {
    let part = |v: Value| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string().as_bytes())
    };
    format!(
        "{}.{}.",
        part(json!({ "alg": "none", "typ": "JWT" })),
        part(json!({
            "iss": base,
            "sub": PATIENT_ID,
            "aud": client_id,
            "fhirUser": format!("{base}/Patient/{PATIENT_ID}"),
            "iat": 0,
            "exp": 0,
        }))
    )
}

fn register(request: &Recorded, state: &Mutex<State>) -> Answer {
    if let Some(token) = request
        .header("authorization")
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        if !state.lock().unwrap().access.iter().any(|t| t == token) {
            return Answer::oauth_error(401, "invalid_token");
        }
    }
    let Ok(Value::Object(mut metadata)) = serde_json::from_str::<Value>(&request.body) else {
        return Answer::oauth_error(400, "invalid_client_metadata");
    };
    metadata.insert("client_id".into(), DYNAMIC_CLIENT_ID.into());
    metadata.insert("client_id_issued_at".into(), 0.into());
    Answer::json(201, Value::Object(metadata).to_string())
}

fn bearer_ok(request: &Recorded, state: &Mutex<State>) -> bool {
    request
        .header("authorization")
        .and_then(|h| h.strip_prefix("Bearer "))
        .is_some_and(|token| state.lock().unwrap().access.iter().any(|t| t == token))
}

fn fhir(
    request: &Recorded,
    path: &str,
    query: &str,
    params: &[(String, String)],
    state: &Mutex<State>,
    base: &str,
) -> Answer {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        ["Binary", id] => {
            let Some(binary) = synthetic("Binary").into_iter().find(|b| b.id == *id) else {
                return Answer::fhir(404, OUTCOME_NOT_FOUND);
            };
            let wants_json = request.header("accept").is_some_and(|a| a.contains("json"));
            if wants_json {
                Answer::fhir(200, binary.raw)
            } else {
                Answer {
                    status: 200,
                    headers: vec![("Content-Type".into(), "application/pdf".into())],
                    body: PDF.to_vec(),
                }
            }
        }
        [resource_type, id] => match synthetic(resource_type).into_iter().find(|r| r.id == *id) {
            Some(resource) => Answer::fhir(200, resource.raw),
            None => Answer::fhir(404, OUTCOME_NOT_FOUND),
        },
        [resource_type] => {
            let all = synthetic(resource_type);
            let size = state.lock().unwrap().page_size;
            let page: usize = one(params, "_page")
                .and_then(|p| p.parse().ok())
                .unwrap_or(1)
                .max(1);
            let rest: Vec<&str> = query
                .split('&')
                .filter(|p| !p.is_empty() && !p.starts_with("_page="))
                .collect();
            let next_query = |n: usize| {
                let mut parts = rest.clone();
                let page = format!("_page={n}");
                parts.push(&page);
                format!("{base}/{resource_type}?{}", parts.join("&"))
            };
            let entries: Vec<String> = all
                .iter()
                .skip((page - 1) * size)
                .take(size)
                .map(|r| {
                    format!(
                        r#"{{"fullUrl":"{base}/{}/{}","resource":{},"search":{{"mode":"match"}}}}"#,
                        r.resource_type, r.id, r.raw
                    )
                })
                .collect();
            let mut links = vec![format!(
                r#"{{"relation":"self","url":"{base}/{resource_type}?{query}"}}"#
            )];
            if page * size < all.len() {
                links.push(format!(
                    r#"{{"relation":"next","url":"{}"}}"#,
                    next_query(page + 1)
                ));
            }
            Answer::fhir(
                200,
                format!(
                    r#"{{"resourceType":"Bundle","type":"searchset","total":{},"link":[{}],"entry":[{}]}}"#,
                    all.len(),
                    links.join(","),
                    entries.join(",")
                ),
            )
        }
        _ => Answer::fhir(404, OUTCOME_NOT_FOUND),
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// `application/x-www-form-urlencoded` pairs, decoded.
fn parse_form(text: &str) -> Vec<(String, String)> {
    text.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (n, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(n), decode(v))
        })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(text.get(i + 1..i + 3).unwrap_or("zz"), 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_decoding_handles_plus_percent_and_bad_escapes() {
        assert_eq!(decode("a+b%20c"), "a b c");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
        assert_eq!(decode("%41"), "A");
        assert_eq!(encode("http://h/x y"), "http%3A%2F%2Fh%2Fx%20y");
    }

    #[test]
    fn every_synthetic_fixture_reads_as_its_type() {
        for t in synthetic_types() {
            let resources = synthetic(t);
            assert!(!resources.is_empty(), "{t}");
            for r in resources {
                assert_eq!(r.resource_type, t);
                assert!(!r.id.is_empty(), "{t} without an id");
            }
        }
    }
}
