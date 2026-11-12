//! A usable access token from the stored session, renewed under the refresh
//! lock when it has lapsed, so two processes never spend one refresh token.

use pk_cli_auth::oauth::{ClientAuth, OAuthClient, TokenRequest, TokenSet};
use pk_cli_auth::refresh_lock::RefreshLock;
use pk_cli_auth::token::DEFAULT_SKEW_SECS;
use pk_cli_core::transport::Transport;
use pk_cli_core::{CliError, Reason};

use crate::assertion::JwtSigner;
use crate::quirks::VendorQuirks;
use crate::register::jwt_bearer_tokens;
use crate::session::{SessionStore, SmartSession};

/// Everything a renewal needs. `client` is how the CLI authenticates to the
/// token endpoint (public, or `SecretBasic` with the secret from the
/// keychain); `signer` is the key of a dynamically registered client.
pub struct Renewer<'a, T> {
    pub store: &'a dyn SessionStore,
    pub lock: &'a RefreshLock,
    pub oauth: &'a OAuthClient<T>,
    pub client: &'a ClientAuth,
    pub quirks: &'a dyn VendorQuirks,
    pub signer: Option<&'a dyn JwtSigner>,
    /// Treat a token expiring within this many seconds as lapsed.
    pub skew_secs: u64,
}

impl<'a, T: Transport> Renewer<'a, T> {
    pub fn new(
        store: &'a dyn SessionStore,
        lock: &'a RefreshLock,
        oauth: &'a OAuthClient<T>,
        client: &'a ClientAuth,
        quirks: &'a dyn VendorQuirks,
    ) -> Self {
        Renewer {
            store,
            lock,
            oauth,
            client,
            quirks,
            signer: None,
            skew_secs: DEFAULT_SKEW_SECS,
        }
    }

    pub fn signer(mut self, signer: &'a dyn JwtSigner) -> Self {
        self.signer = Some(signer);
        self
    }

    /// The stored session for `account`, renewed first when its access token
    /// has lapsed at `now` (Unix seconds). [`CliError::Auth`] with
    /// `Reason::NoCredential` when nothing is stored.
    ///
    /// Synchronous, because the refresh lock is: the grant inside it runs on
    /// `pk_cli_core::block_on`, so the transport must be one it can drive (the
    /// family's blocking transport). Call it before the command's own
    /// `block_on`, not from inside a future.
    pub fn fresh(&self, account: &str, now: u64) -> Result<SmartSession, CliError> {
        let session = self.load(account)?;
        if !session.tokens.is_expired(now, self.skew_secs) {
            return Ok(session);
        }
        self.lock.renew(
            || self.load(account),
            |latest| latest.tokens.is_expired(now, self.skew_secs),
            |latest| self.grant_and_store(latest, now),
        )
    }

    /// Renew after the server refused `refused_access_token` (a 401), unless
    /// another process already replaced it.
    pub fn after_refusal(
        &self,
        account: &str,
        refused_access_token: &str,
        now: u64,
    ) -> Result<SmartSession, CliError> {
        self.lock.renew(
            || self.load(account),
            |latest| latest.tokens.access_token == refused_access_token,
            |latest| self.grant_and_store(latest, now),
        )
    }

    fn load(&self, account: &str) -> Result<SmartSession, CliError> {
        self.store.load(account)?.ok_or_else(|| {
            CliError::Auth(
                "not signed in to this organization for this patient; run auth login".into(),
            )
            .because(Reason::NoCredential)
        })
    }

    fn grant_and_store(&self, latest: SmartSession, now: u64) -> Result<SmartSession, CliError> {
        let tokens = pk_cli_core::block_on(self.grant(&latest, now))?;
        let renewed = SmartSession { tokens, ..latest };
        self.store.save(&renewed)?;
        Ok(renewed)
    }

    async fn grant(&self, session: &SmartSession, now: u64) -> Result<TokenSet, CliError> {
        if let Some(client) = &session.dynamic_client {
            let signer = self.signer.ok_or_else(|| {
                CliError::Other(
                    "this session's client signs its grants, and no signer was given".into(),
                )
            })?;
            return jwt_bearer_tokens(self.oauth, session, client, signer, now).await;
        }
        let Some(refresh_token) = session.tokens.refresh_token() else {
            return Err(CliError::Auth(
                "the sign-in has lapsed and the server gave no refresh token; run auth login again"
                    .into(),
            )
            .because(Reason::CredentialRefused));
        };
        let mut request = TokenRequest::refresh(&refresh_token);
        if let Some(scope) = self.quirks.refresh_scope(session) {
            request = request.scope(scope);
        }
        let response = self
            .oauth
            .token(&session.token_endpoint, self.client, &request)
            .await?;
        Ok(TokenSet::from_response(
            response,
            now,
            Some(&session.tokens),
        ))
    }
}
