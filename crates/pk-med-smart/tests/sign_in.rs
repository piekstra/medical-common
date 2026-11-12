//! SMART sign-in, renewal and dynamic registration against the mock server,
//! over the real HTTP transport. Offline (127.0.0.1) and no keychain: sessions
//! live in a `MemoryStore`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use pk_cli_auth::oauth::{ClientAuth, OAuthClient};
use pk_cli_auth::refresh_lock::RefreshLock;
use pk_cli_core::{block_on, CliError, Reason};
use pk_cli_http::BlockingTransport;
use pk_cli_secrets::Secret;
use pk_med_fhir::test_util::{
    http_get, Fault, MockServer, CLIENT_ID, CLIENT_SECRET, DYNAMIC_CLIENT_ID, PATIENT_ID,
};
use pk_med_smart::{
    begin, discover, register_dynamic_client, DynamicClient, JwtSigner, LaunchRequest, MemoryStore,
    NoQuirks, Renewer, ScopeSet, ScopeVersion, SessionStore, SmartApp, SmartConfiguration,
    SmartSession, SmartSignIn, VendorQuirks,
};

const NOW: u64 = 1_600_000_000;

fn transport() -> BlockingTransport {
    BlockingTransport::new().unwrap()
}

fn oauth() -> OAuthClient<BlockingTransport> {
    OAuthClient::new(transport(), "pk-test", "0.0.0")
}

/// A refresh lock in its own directory, removed when the test ends.
struct TestLock(RefreshLock, PathBuf);

impl std::ops::Deref for TestLock {
    type Target = RefreshLock;
    fn deref(&self) -> &RefreshLock {
        &self.0
    }
}

impl Drop for TestLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.1);
    }
}

fn lock() -> TestLock {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pk-med-smart-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    TestLock(
        RefreshLock::at(dir.join("refresh.lock")).wait_at_most(Duration::from_secs(5)),
        dir,
    )
}

fn config(mock: &MockServer) -> SmartConfiguration {
    block_on(discover(
        &transport(),
        &mock.fhir_base(),
        &NoQuirks,
        "pk-test",
        "0.0.0",
    ))
    .unwrap()
}

/// The browser's part: follow the authorization URL to its redirect, then
/// deliver the redirect to the loopback listener, on another thread.
fn browse(url: &str) -> std::thread::JoinHandle<()> {
    let url = url.to_string();
    std::thread::spawn(move || {
        let answer = http_get(&url).unwrap();
        assert_eq!(answer.status, 302, "{}", answer.body);
        let back = answer.header("location").unwrap().to_string();
        http_get(&back).unwrap();
    })
}

fn app(client: &ClientAuth) -> SmartApp {
    SmartApp {
        app: "pk-test".into(),
        client: match client {
            ClientAuth::SecretBasic { client_id, secret } => ClientAuth::SecretBasic {
                client_id: client_id.clone(),
                secret: Secret::new(secret.expose()),
            },
            other => ClientAuth::public(other.client_id()),
        },
        ports: Vec::new(),
    }
}

fn start(mock: &MockServer, client: &ClientAuth, quirks: &dyn VendorQuirks) -> SmartSignIn {
    let config = config(mock);
    let base = mock.fhir_base();
    begin(
        &app(client),
        LaunchRequest {
            config: &config,
            fhir_base: &base,
            aud: None,
            scopes: ScopeSet::standalone_patient().read_all(ScopeVersion::for_server(&config)),
            quirks,
        },
    )
    .unwrap()
}

fn sign_in(mock: &MockServer, client: &ClientAuth, quirks: &dyn VendorQuirks) -> SmartSession {
    let sign_in = start(mock, client, quirks);
    let browser = browse(sign_in.url());
    let code = sign_in
        .wait_for_code(Duration::from_secs(10), |reason| {
            panic!("refused: {reason}")
        })
        .unwrap();
    browser.join().unwrap();
    block_on(sign_in.exchange(&oauth(), client, &code, "org-0001", NOW)).unwrap()
}

#[test]
fn discovery_reads_the_well_known_document() {
    let mock = MockServer::start();
    let config = config(&mock);
    assert_eq!(config.issuer.as_deref(), Some(mock.fhir_base().as_str()));
    assert_eq!(config.token_endpoint, format!("{}/token", mock.origin()));
    assert!(config.supports_s256());
    let seen = mock.requests();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].path(), "/fhir/.well-known/smart-configuration");
}

#[test]
fn discovery_falls_back_to_metadata_on_a_4xx_and_not_on_a_5xx() {
    let mock = MockServer::start();
    mock.fail(Fault::new("GET", "/fhir/.well-known", 404, ""));
    let config = config(&mock);
    assert_eq!(
        config.authorization_endpoint,
        format!("{}/authorize", mock.origin())
    );
    assert_eq!(mock.requests()[1].path(), "/fhir/metadata");

    let mock = MockServer::start();
    mock.fail(Fault::unavailable("/fhir/.well-known"));
    let err = block_on(discover(
        &transport(),
        &mock.fhir_base(),
        &NoQuirks,
        "pk-test",
        "0",
    ))
    .unwrap_err();
    assert_eq!(err.exit_code(), 5);
    assert!(err.to_string().contains("503"), "{err}");
    assert_eq!(
        mock.requests().len(),
        1,
        "a 5xx is not a reason to ask elsewhere"
    );
}

struct Epicish;
impl VendorQuirks for Epicish {
    fn metadata_first(&self) -> bool {
        true
    }
    fn discovery_headers(&self) -> Vec<(String, String)> {
        vec![("Epic-Client-ID".into(), CLIENT_ID.into())]
    }
}

#[test]
fn a_quirk_asks_metadata_first_with_its_headers() {
    let mock = MockServer::start();
    let config = block_on(discover(
        &transport(),
        &mock.fhir_base(),
        &Epicish,
        "pk-test",
        "0",
    ))
    .unwrap();
    assert!(
        config.issuer.is_none(),
        "the CapabilityStatement names no issuer"
    );
    let seen = mock.requests();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].path(), "/fhir/metadata");
    assert_eq!(seen[0].header("epic-client-id"), Some(CLIENT_ID));
}

#[test]
fn a_public_client_signs_in_with_the_patient_context() {
    let mock = MockServer::start();
    let session = sign_in(&mock, &ClientAuth::public(CLIENT_ID), &NoQuirks);
    assert_eq!(session.patient.as_deref(), Some(PATIENT_ID));
    assert_eq!(
        session.fhir_user.as_deref(),
        Some(format!("{}/Patient/{PATIENT_ID}", mock.fhir_base()).as_str())
    );
    assert_eq!(session.account(), format!("smart/org-0001/{PATIENT_ID}"));
    assert!(
        session.tokens.refresh_token.is_some(),
        "offline_access gives one"
    );
    assert_eq!(session.tokens.expires_at, Some(NOW + 3600));
    assert!(
        session.scope.contains("patient/*.rs"),
        "the mock lists permission-v2"
    );
    assert!(session.renewable());
    let shown = format!("{session:?}");
    assert!(!shown.contains(&session.tokens.access_token), "{shown}");

    let authorize = mock
        .requests()
        .into_iter()
        .find(|r| r.path() == "/authorize")
        .unwrap();
    assert!(authorize.target.contains("code_challenge_method=S256"));
    assert!(authorize.target.contains("aud="), "{}", authorize.target);
}

#[test]
fn a_confidential_client_authenticates_with_its_secret() {
    let mock = MockServer::start();
    let client = ClientAuth::SecretBasic {
        client_id: CLIENT_ID.into(),
        secret: Secret::new(CLIENT_SECRET),
    };
    let session = sign_in(&mock, &client, &NoQuirks);
    assert_eq!(session.client_id, CLIENT_ID);
    let token = mock
        .requests()
        .into_iter()
        .find(|r| r.path() == "/token")
        .unwrap();
    assert!(token.header("authorization").unwrap().starts_with("Basic "));
    assert!(
        !token.body.contains(CLIENT_SECRET),
        "the secret is not in the body"
    );
}

#[test]
fn a_lapsed_session_is_renewed_once_with_its_scope() {
    let mock = MockServer::start();
    let client = ClientAuth::public(CLIENT_ID);
    let mut session = sign_in(&mock, &client, &NoQuirks);
    session.tokens.expires_at = Some(NOW - 1);
    let first_refresh = session.tokens.refresh_token.clone();
    let store = MemoryStore::default();
    store.save(&session).unwrap();
    let (lock, oauth) = (lock(), oauth());
    let renewer = Renewer::new(&store, &lock, &oauth, &client, &NoQuirks);

    let renewed = renewer.fresh(&session.account(), NOW).unwrap();
    assert_ne!(renewed.tokens.access_token, session.tokens.access_token);
    assert_ne!(
        renewed.tokens.refresh_token, first_refresh,
        "the mock rotates it"
    );
    assert_eq!(renewed.patient.as_deref(), Some(PATIENT_ID));
    let refresh = mock
        .requests()
        .into_iter()
        .rev()
        .find(|r| r.path() == "/token")
        .unwrap();
    assert!(refresh.body.contains("grant_type=refresh_token"));
    assert!(
        refresh.body.contains("scope="),
        "the stored scope goes with it"
    );
    let stored = store.load(&session.account()).unwrap().unwrap();
    assert_eq!(stored.tokens.access_token, renewed.tokens.access_token);

    let tokens_before = mock.requests().len();
    let again = renewer.fresh(&session.account(), NOW).unwrap();
    assert_eq!(again.tokens.access_token, renewed.tokens.access_token);
    assert_eq!(
        mock.requests().len(),
        tokens_before,
        "a fresh token is not renewed"
    );

    let same = renewer
        .after_refusal(&session.account(), "a-token-already-replaced", NOW)
        .unwrap();
    assert_eq!(same.tokens.access_token, renewed.tokens.access_token);
    assert_eq!(
        mock.requests().len(),
        tokens_before,
        "another process renewed already"
    );
}

#[test]
fn a_refused_grant_is_recorded_and_the_next_use_tries_again() {
    let mock = MockServer::start();
    let client = ClientAuth::public(CLIENT_ID);
    let mut session = sign_in(&mock, &client, &NoQuirks);
    session.tokens.expires_at = Some(NOW - 1);
    let store = MemoryStore::default();
    store.save(&session).unwrap();
    let (lock, oauth) = (lock(), oauth());
    let renewer = Renewer::new(&store, &lock, &oauth, &client, &NoQuirks);

    mock.fail(Fault::new(
        "POST",
        "/token",
        400,
        r#"{"error":"invalid_grant"}"#,
    ));
    let err = renewer.fresh(&session.account(), NOW).unwrap_err();
    assert_eq!(
        (err.exit_code(), err.reason()),
        (3, Some(Reason::CredentialRefused))
    );
    let stored = store.load(&session.account()).unwrap().unwrap();
    assert_eq!(stored.refused_at, Some(NOW));
    assert_eq!(
        stored.tokens.refresh_token, session.tokens.refresh_token,
        "the sign-in is kept"
    );

    let renewed = renewer.fresh(&session.account(), NOW + 1).unwrap();
    assert_eq!(renewed.refused_at, None);
    let stored = store.load(&session.account()).unwrap().unwrap();
    assert_eq!(stored.refused_at, None);
    assert_eq!(stored.tokens.access_token, renewed.tokens.access_token);
}

#[test]
fn nothing_stored_or_nothing_to_renew_with_is_exit_3() {
    let mock = MockServer::start();
    let client = ClientAuth::public(CLIENT_ID);
    let store = MemoryStore::default();
    let (lock, oauth) = (lock(), oauth());
    let renewer = Renewer::new(&store, &lock, &oauth, &client, &NoQuirks);
    let err = renewer.fresh("smart/org-0001/pat-0001", NOW).unwrap_err();
    assert_eq!(
        (err.exit_code(), err.reason()),
        (3, Some(Reason::NoCredential))
    );

    let mut session = sign_in(&mock, &client, &NoQuirks);
    session.tokens.refresh_token = None;
    session.tokens.expires_at = Some(NOW - 1);
    store.save(&session).unwrap();
    let err = renewer.fresh(&session.account(), NOW).unwrap_err();
    assert_eq!(
        (err.exit_code(), err.reason()),
        (3, Some(Reason::CredentialRefused))
    );
}

/// Not a signature: the mock reads the assertion's claims only.
struct FakeSigner;
impl JwtSigner for FakeSigner {
    fn alg(&self) -> &str {
        "RS384"
    }
    fn sign(&self, _: &[u8]) -> Result<Vec<u8>, CliError> {
        Ok(b"synthetic-signature".to_vec())
    }
}

fn fake_signer(_: &DynamicClient) -> Result<Box<dyn JwtSigner>, CliError> {
    Ok(Box::new(FakeSigner))
}

fn token_requests(mock: &MockServer) -> usize {
    mock.requests()
        .iter()
        .filter(|r| r.path() == "/token")
        .count()
}

#[test]
fn a_registered_client_gets_and_renews_tokens_with_its_assertion() {
    let mock = MockServer::start();
    let client = ClientAuth::public(CLIENT_ID);
    let session = sign_in(&mock, &client, &NoQuirks);
    let config = config(&mock);
    let jwks = serde_json::json!({ "keys": [{ "kty": "RSA", "n": "AQAB", "e": "AQAB" }] });
    let oauth = oauth();
    let tokens_before = token_requests(&mock);
    let registered = block_on(register_dynamic_client(
        &oauth,
        &config,
        session.clone(),
        CLIENT_ID,
        jwks,
        &FakeSigner,
        Secret::new("synthetic-private-key"),
    ))
    .unwrap();
    let dynamic = registered.dynamic_client.as_ref().unwrap();
    assert_eq!(dynamic.client_id, DYNAMIC_CLIENT_ID);
    assert_eq!(
        registered.tokens.access_token, session.tokens.access_token,
        "registering grants nothing"
    );
    assert_eq!(token_requests(&mock), tokens_before);
    assert_eq!(registered.patient.as_deref(), Some(PATIENT_ID));
    assert!(!format!("{registered:?}").contains("synthetic-private-key"));

    let seen = mock.requests();
    let register = seen.iter().find(|r| r.path() == "/register").unwrap();
    assert_eq!(
        register.header("authorization"),
        Some(format!("Bearer {}", session.tokens.access_token).as_str())
    );
    let body: serde_json::Value = serde_json::from_str(&register.body).unwrap();
    assert_eq!(body["software_id"], CLIENT_ID);
    assert_eq!(body["token_endpoint_auth_method"], "none");
    assert_eq!(body["grant_types"][0], pk_med_smart::JWT_BEARER);
    assert!(body["jwks"]["keys"].is_array());

    // The first grant fails: the stored client survives it, and the next
    // try needs no new registration.
    let store = MemoryStore::default();
    store.save(&registered).unwrap();
    let lock = lock();
    let renewer = Renewer::new(&store, &lock, &oauth, &client, &NoQuirks).signer(&fake_signer);
    mock.fail(Fault::new("POST", "/token", 503, ""));
    let err = renewer.renew(&registered.account(), NOW).unwrap_err();
    assert_eq!(err.exit_code(), 5);
    let kept = store.load(&registered.account()).unwrap().unwrap();
    assert_eq!(
        kept.dynamic_client.as_ref().map(|c| c.client_id.as_str()),
        Some(DYNAMIC_CLIENT_ID)
    );
    let renewed = renewer.renew(&registered.account(), NOW).unwrap();
    assert_ne!(renewed.tokens.access_token, registered.tokens.access_token);
    let grant = mock
        .requests()
        .into_iter()
        .rev()
        .find(|r| r.path() == "/token")
        .unwrap();
    assert!(grant.body.contains("jwt-bearer"), "{}", grant.body);
    assert_eq!(
        mock.requests()
            .iter()
            .filter(|r| r.path() == "/register")
            .count(),
        1
    );

    // Lapsed: renewed by the assertion with no refresh token.
    let mut lapsed = renewed.clone();
    lapsed.tokens.refresh_token = None;
    lapsed.tokens.expires_at = Some(NOW - 1);
    store.save(&lapsed).unwrap();
    let again = renewer.fresh(&lapsed.account(), NOW).unwrap();
    assert_ne!(again.tokens.access_token, lapsed.tokens.access_token);

    let unsigned = Renewer::new(&store, &lock, &oauth, &client, &NoQuirks);
    store.save(&lapsed).unwrap();
    assert!(unsigned.fresh(&lapsed.account(), NOW).is_err());
}

/// Signs with a key other than the one registered.
struct OtherKey;
impl JwtSigner for OtherKey {
    fn alg(&self) -> &str {
        "RS384"
    }
    fn kid(&self) -> Option<String> {
        Some("another-key".into())
    }
    fn sign(&self, _: &[u8]) -> Result<Vec<u8>, CliError> {
        Ok(b"synthetic-signature".to_vec())
    }
}

#[test]
fn the_signer_is_made_from_the_session_read_under_the_lock() {
    let mock = MockServer::start();
    let client = ClientAuth::public(CLIENT_ID);
    let mut session = sign_in(&mock, &client, &NoQuirks);
    session.dynamic_client = Some(DynamicClient::new(
        DYNAMIC_CLIENT_ID,
        "RS384",
        Some("registered-key".into()),
        Secret::new("synthetic-private-key"),
    ));
    let store = MemoryStore::default();
    store.save(&session).unwrap();
    let (lock, oauth) = (lock(), oauth());
    let seen_kid = std::sync::Mutex::new(None);
    let made = |c: &DynamicClient| -> Result<Box<dyn JwtSigner>, CliError> {
        *seen_kid.lock().unwrap() = c.kid.clone();
        Ok(Box::new(OtherKey))
    };
    let renewer = Renewer::new(&store, &lock, &oauth, &client, &NoQuirks).signer(&made);
    let before = token_requests(&mock);
    let err = renewer.renew(&session.account(), NOW).unwrap_err();
    assert!(err.to_string().contains("not the one"), "{err}");
    assert_eq!(seen_kid.lock().unwrap().as_deref(), Some("registered-key"));
    assert_eq!(token_requests(&mock), before, "nothing was sent");
}

#[test]
fn a_refresh_asks_for_the_scope_granted_not_the_one_asked_for() {
    let mock = MockServer::start();
    let client = ClientAuth::public(CLIENT_ID);
    let mut session = sign_in(&mock, &client, &NoQuirks);
    assert!(session.scope.contains("patient/*.rs"));
    // The person unticked everything but observations at consent.
    session.tokens.scope = Some("openid fhirUser offline_access patient/Observation.rs".into());
    session.tokens.expires_at = Some(NOW - 1);
    let store = MemoryStore::default();
    store.save(&session).unwrap();
    let (lock, oauth) = (lock(), oauth());
    Renewer::new(&store, &lock, &oauth, &client, &NoQuirks)
        .fresh(&session.account(), NOW)
        .unwrap();
    let refresh = mock
        .requests()
        .into_iter()
        .rev()
        .find(|r| r.path() == "/token")
        .unwrap();
    assert!(
        refresh.body.contains("patient%2FObservation.rs"),
        "{}",
        refresh.body
    );
    assert!(!refresh.body.contains("%2A"), "{}", refresh.body);
}

#[test]
fn a_renewer_for_another_client_sends_nothing() {
    let mock = MockServer::start();
    let mut session = sign_in(&mock, &ClientAuth::public(CLIENT_ID), &NoQuirks);
    session.tokens.expires_at = Some(NOW - 1);
    let store = MemoryStore::default();
    store.save(&session).unwrap();
    let other = ClientAuth::SecretBasic {
        client_id: "another-orgs-client".into(),
        secret: Secret::new("another-orgs-secret"),
    };
    let (lock, oauth) = (lock(), oauth());
    let before = token_requests(&mock);
    let err = Renewer::new(&store, &lock, &oauth, &other, &NoQuirks)
        .fresh(&session.account(), NOW)
        .unwrap_err();
    assert!(err.to_string().contains("another client"), "{err}");
    assert_eq!(token_requests(&mock), before);
}

fn state_of(url: &str) -> String {
    url.split(['?', '&'])
        .find_map(|p| p.strip_prefix("state="))
        .unwrap()
        .to_string()
}

#[test]
fn an_answer_for_another_sign_in_is_refused_and_the_wait_goes_on() {
    let mock = MockServer::start();
    let sign_in = start(&mock, &ClientAuth::public(CLIENT_ID), &NoQuirks);
    let stray = format!("{}?code=stolen&state=not-this-one", sign_in.redirect_uri());
    let url = sign_in.url().to_string();
    let browser = std::thread::spawn(move || {
        http_get(&stray).unwrap();
        let answer = http_get(&url).unwrap();
        http_get(answer.header("location").unwrap()).unwrap();
    });
    let mut refused = Vec::new();
    let code = sign_in
        .wait_for_code(Duration::from_secs(10), |why| refused.push(why.to_string()))
        .unwrap();
    browser.join().unwrap();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert_ne!(code.expose(), "stolen");
}

#[test]
fn an_answer_from_another_issuer_is_refused() {
    let mock = MockServer::start();
    let sign_in = start(&mock, &ClientAuth::public(CLIENT_ID), &NoQuirks);
    assert!(sign_in.issuer_checked());
    let forged = format!(
        "{}?code=synthetic-code-9&state={}&iss=https%3A%2F%2Fother.example.test",
        sign_in.redirect_uri(),
        state_of(sign_in.url())
    );
    let browser = std::thread::spawn(move || {
        http_get(&forged).unwrap();
    });
    let err = sign_in
        .wait_for_code(Duration::from_secs(10), |_| {})
        .unwrap_err();
    browser.join().unwrap();
    assert_eq!(err.exit_code(), 3);
    assert!(err.to_string().contains("issuer"), "{err}");
}

#[test]
fn a_declined_sign_in_is_exit_3() {
    let mock = MockServer::start();
    mock.deny_next_authorization();
    let sign_in = start(&mock, &ClientAuth::public(CLIENT_ID), &NoQuirks);
    let browser = browse(sign_in.url());
    let err = sign_in
        .wait_for_code(Duration::from_secs(10), |_| {})
        .unwrap_err();
    browser.join().unwrap();
    assert_eq!(err.exit_code(), 3);
    assert!(err.to_string().contains("access_denied"), "{err}");
}

#[test]
fn a_token_answer_without_a_token_is_refused() {
    let mock = MockServer::start();
    let client = ClientAuth::public(CLIENT_ID);
    let sign_in = start(&mock, &client, &NoQuirks);
    let browser = browse(sign_in.url());
    let code = sign_in
        .wait_for_code(Duration::from_secs(10), |_| {})
        .unwrap();
    browser.join().unwrap();
    mock.fail(Fault::new(
        "POST",
        "/token",
        200,
        r#"{"token_type":"Bearer"}"#,
    ));
    let err = block_on(sign_in.exchange(&oauth(), &client, &code, "org-0001", NOW)).unwrap_err();
    assert_eq!(err.exit_code(), 5, "{err}");
}

#[test]
fn a_server_without_s256_is_refused_before_anything_is_sent() {
    let mock = MockServer::start();
    let body = format!(
        r#"{{"authorization_endpoint":"{0}/authorize","token_endpoint":"{0}/token","code_challenge_methods_supported":["plain"]}}"#,
        mock.origin()
    );
    let config = SmartConfiguration::parse(&body).unwrap();
    let base = mock.fhir_base();
    let err = begin(
        &app(&ClientAuth::public(CLIENT_ID)),
        LaunchRequest {
            config: &config,
            fhir_base: &base,
            aud: None,
            scopes: ScopeSet::standalone_patient(),
            quirks: &NoQuirks,
        },
    )
    .err()
    .unwrap();
    assert_eq!(err.exit_code(), 5);
    assert!(err.to_string().contains("S256"), "{err}");
    assert!(mock.requests().is_empty());
}

#[test]
fn a_429_on_discovery_is_not_a_reason_to_ask_elsewhere_and_a_redirect_is() {
    let mock = MockServer::start();
    mock.fail(Fault::too_many_requests("/fhir/.well-known", 5));
    let err = block_on(discover(
        &transport(),
        &mock.fhir_base(),
        &NoQuirks,
        "pk-test",
        "0",
    ))
    .unwrap_err();
    assert_eq!(err.exit_code(), 5);
    assert_eq!(mock.requests().len(), 1);

    let mock = MockServer::start();
    mock.fail(
        Fault::new("GET", "/fhir/.well-known", 301, "")
            .header("location", "https://elsewhere.example.test/"),
    );
    let config = config(&mock);
    assert_eq!(
        config.token_endpoint,
        format!("{}/token", mock.origin()),
        "read from /metadata"
    );
    assert_eq!(mock.requests()[1].path(), "/fhir/metadata");
}

#[test]
fn an_https_server_naming_a_local_http_endpoint_is_refused() {
    let body = r#"{"issuer":"https://fhir.example.test/r4","authorization_endpoint":"https://fhir.example.test/authorize","token_endpoint":"http://127.0.0.1:8080/token"}"#;
    let config = SmartConfiguration::parse(body).unwrap();
    let err = config
        .check_for("https://fhir.example.test/r4")
        .unwrap_err();
    assert!(err.to_string().contains("token endpoint"), "{err}");
    assert!(config.check_for("http://127.0.0.1:8080/r4").is_ok());

    let body = r#"{"authorization_endpoint":"https://a.example.test/authorize","token_endpoint":"https://a.example.test/token","revocation_endpoint":"http://a.example.test/revoke"}"#;
    let err = SmartConfiguration::parse(body).unwrap_err();
    assert!(err.to_string().contains("revocation endpoint"), "{err}");
}

#[test]
fn aud_drops_a_trailing_slash() {
    let mock = MockServer::start();
    let config = config(&mock);
    let base = format!("{}/", mock.fhir_base());
    let sign_in = begin(
        &app(&ClientAuth::public(CLIENT_ID)),
        LaunchRequest {
            config: &config,
            fhir_base: &base,
            aud: None,
            scopes: ScopeSet::standalone_patient(),
            quirks: &NoQuirks,
        },
    )
    .unwrap();
    let aud = sign_in
        .url()
        .split('&')
        .find_map(|p| p.strip_prefix("aud="))
        .unwrap();
    assert!(!aud.ends_with("%2F"), "{aud}");
}

#[test]
fn an_endpoint_that_is_not_https_is_refused() {
    let body = r#"{"authorization_endpoint":"http://auth.example.test/authorize","token_endpoint":"https://auth.example.test/token"}"#;
    let err = SmartConfiguration::parse(body).unwrap_err();
    assert_eq!(err.exit_code(), 5);
    assert!(err.to_string().contains("not https"), "{err}");
    let body = r#"{"authorization_endpoint":"https://auth.example.test/authorize","token_endpoint":"https://auth.example.test/token","capabilities":["permission-v2"],"code_challenge_methods_supported":["plain"]}"#;
    let config = SmartConfiguration::parse(body).unwrap();
    assert_eq!(ScopeVersion::for_server(&config), ScopeVersion::V2);
    assert!(!config.supports_s256());
}
