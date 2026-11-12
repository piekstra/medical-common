//! A standalone patient launch: the browser sign-in, the code exchange, and
//! the session it leaves.

use std::time::Duration;

use pk_cli_auth::oauth::{
    random_token, AuthorizationCodeFlow, ClientAuth, OAuthClient, PendingAuthorization, TokenSet,
};
use pk_cli_core::transport::Transport;
use pk_cli_core::{CliError, Reason};
use pk_cli_secrets::Secret;
use serde_json::Map;

use crate::discovery::SmartConfiguration;
use crate::quirks::VendorQuirks;
use crate::scopes::ScopeSet;
use crate::session::{LaunchContext, SmartSession};

/// Who signs in: the CLI's registered client and its redirect ports.
pub struct SmartApp {
    /// The CLI's name, on the page the browser shows at the end.
    pub app: String,
    /// A public client, or a confidential one (`ClientAuth::SecretBasic` with
    /// the secret read from the keychain).
    pub client: ClientAuth,
    /// The loopback ports registered with the vendor, tried in order; empty
    /// for any port (a vendor that matches only the host).
    pub ports: Vec<u16>,
}

/// What a sign-in is for.
pub struct LaunchRequest<'a> {
    pub config: &'a SmartConfiguration,
    /// The FHIR base, and SMART's `aud` (with no trailing `/`) unless `aud`
    /// says otherwise.
    pub fhir_base: &'a str,
    /// `aud` as the server spells the base, when that differs from
    /// `fhir_base` (a directory's own host case).
    pub aud: Option<&'a str>,
    pub scopes: ScopeSet,
    pub quirks: &'a dyn VendorQuirks,
}

/// Bind the loopback listener and build the address to open. Nothing has been
/// sent anywhere yet: print [`SmartSignIn::url`] and open it. When the scope
/// asks for `openid`, the address carries a fresh `nonce` that
/// [`SmartSignIn::exchange`] checks against the ID token
/// ([`VendorQuirks::send_nonce`]).
pub fn begin(app: &SmartApp, request: LaunchRequest<'_>) -> Result<SmartSignIn, CliError> {
    let config = request.config;
    if !config.supports_s256() {
        return Err(CliError::Upstream(
            "the server does not offer PKCE S256; refusing to sign in without it".into(),
        ));
    }
    let scopes = request.quirks.scopes(request.scopes);
    let scope = scopes.to_string();
    let mut flow = AuthorizationCodeFlow::new(
        &app.app,
        &config.authorization_endpoint,
        app.client.client_id(),
    )
    .scope(&scope)
    .ports(&app.ports);
    if request.quirks.send_aud() {
        let aud = request
            .aud
            .unwrap_or_else(|| request.fhir_base.trim_end_matches('/'));
        flow = flow.param("aud", aud);
    }
    for (name, value) in request.quirks.authorize_params() {
        flow = flow.param(name, value);
    }
    let nonce = if request.quirks.send_nonce() && scope.split(' ').any(|s| s == "openid") {
        Some(Secret::new(random_token(32)?))
    } else {
        None
    };
    if let Some(nonce) = &nonce {
        flow = flow.param("nonce", nonce.expose());
    }
    if let Some(issuer) = &config.issuer {
        flow = flow.issuer(issuer);
    }
    Ok(SmartSignIn {
        pending: flow.start()?,
        config: config.clone(),
        fhir_base: request.fhir_base.trim_end_matches('/').to_string(),
        scope,
        nonce,
    })
}

/// A sign-in under way.
pub struct SmartSignIn {
    pending: PendingAuthorization,
    config: SmartConfiguration,
    fhir_base: String,
    scope: String,
    /// Sent with the authorization; the ID token must carry it.
    nonce: Option<Secret>,
}

impl SmartSignIn {
    /// The address to print and open.
    pub fn url(&self) -> &str {
        self.pending.url()
    }

    /// Whether the answer's `iss` (RFC 9207) is checked against the
    /// discovered issuer. Only when discovery named one: the `/metadata`
    /// fallback never does, and a well-known document may leave it out.
    /// Even then an answer without `iss` passes (cli-common v0.31.1), so a
    /// mix-up between two authorization servers is caught only for servers
    /// that send it. The session records the issuer it was bound to
    /// ([`SmartSession::issuer`]).
    pub fn issuer_checked(&self) -> bool {
        self.config.issuer.is_some()
    }

    pub fn redirect_uri(&self) -> &str {
        self.pending.redirect_uri()
    }

    /// Whether the authorization carries a `nonce` that the exchange checks.
    pub fn nonce_checked(&self) -> bool {
        self.nonce.is_some()
    }

    /// Wait for the code, calling `on_refused` with each stray answer the
    /// listener refuses while it waits (print it on stderr).
    pub fn wait_for_code(
        &self,
        timeout: Duration,
        on_refused: impl FnMut(&str),
    ) -> Result<Secret, CliError> {
        self.pending.wait_for_code_with(timeout, on_refused)
    }

    /// Exchange the code for tokens and build the session, for organization
    /// `organization`. `now` is Unix seconds. The session is not stored: do
    /// that under `RefreshLock::store_signin`. When a `nonce` was sent, an ID
    /// token without it, or with another, is refused (exit 3): it answers
    /// some other sign-in. The ID token is otherwise read unverified, for
    /// display only.
    pub async fn exchange<T: Transport>(
        &self,
        oauth: &OAuthClient<T>,
        client: &ClientAuth,
        code: &Secret,
        organization: &str,
        now: u64,
    ) -> Result<SmartSession, CliError> {
        let response = oauth
            .token(
                &self.config.token_endpoint,
                client,
                &self.pending.token_request(code),
            )
            .await?;
        if let (Some(sent), Some(id_token)) = (&self.nonce, &response.id_token) {
            let carried = pk_cli_auth::token::claims(id_token.expose())
                .and_then(|c| c.get("nonce").and_then(|n| n.as_str().map(str::to_string)));
            if carried.as_deref() != Some(sent.expose()) {
                return Err(CliError::Auth(
                    "the ID token does not carry this sign-in's nonce; refusing it. Sign in again"
                        .into(),
                )
                .because(Reason::CredentialRefused));
            }
        }
        let context = LaunchContext::from_response(&response);
        let tokens = TokenSet::from_response(response, now, None);
        Ok(SmartSession {
            format: crate::session::SESSION_FORMAT,
            organization: organization.to_string(),
            patient: context.patient,
            fhir_user: context.fhir_user,
            fhir_base: self.fhir_base.clone(),
            issuer: self.config.issuer.clone(),
            token_endpoint: self.config.token_endpoint.clone(),
            client_id: client.client_id().to_string(),
            scope: self.scope.clone(),
            tokens,
            dynamic_client: None,
            refused_at: None,
            extra: Map::new(),
        })
    }
}
