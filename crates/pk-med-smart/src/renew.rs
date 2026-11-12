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
use crate::session::{DynamicClient, SessionStore, SmartSession};

/// Makes the signer for a registered client from the client as the session
/// stores it, read under the refresh lock. With feature `rsa`:
/// `&|client| Ok(Box::new(RsaKey::for_client(client)?))`.
pub type SignerFor<'s> = dyn Fn(&DynamicClient) -> Result<Box<dyn JwtSigner>, CliError> + 's;

/// Everything a renewal needs. `client` is how the CLI authenticates to the
/// token endpoint (public, or `SecretBasic` with the secret from the
/// keychain), and must be the client the session's tokens were issued to.
/// `signer` makes a registered client's signer from the session read under
/// the lock, so a sign-in another process stored meanwhile is signed with
/// its own key.
pub struct Renewer<'a, T> {
    pub store: &'a dyn SessionStore,
    pub lock: &'a RefreshLock,
    pub oauth: &'a OAuthClient<T>,
    pub client: &'a ClientAuth,
    pub quirks: &'a dyn VendorQuirks,
    pub signer: Option<&'a SignerFor<'a>>,
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

    pub fn signer(mut self, signer: &'a SignerFor<'a>) -> Self {
        self.signer = Some(signer);
        self
    }

    /// The stored session for `account`, renewed first when its access token
    /// has lapsed at `now` (Unix seconds) or the server refused it
    /// (`refused_at`): a refused token is never handed out again, and a
    /// refused sign-in with nothing to renew with is refused before anything
    /// is sent. [`CliError::Auth`] with `Reason::NoCredential` when nothing
    /// is stored.
    ///
    /// Synchronous, because the refresh lock is: the grant inside it runs on
    /// `pk_cli_core::block_on`, so the transport must be one it can drive (the
    /// family's blocking transport). Call it before the command's own
    /// `block_on`, not from inside a future.
    pub fn fresh(&self, account: &str, now: u64) -> Result<SmartSession, CliError> {
        let session = self.load(account)?;
        if !self.due(&session, now) {
            return Ok(session);
        }
        self.lock.renew(
            || self.load(account),
            |latest| self.due(latest, now),
            |latest| self.grant_and_store(latest, now),
        )
    }

    fn due(&self, session: &SmartSession, now: u64) -> bool {
        session.refused_at.is_some() || session.tokens.is_expired(now, self.skew_secs)
    }

    /// Renew now, due or not, unless another process renewed since this
    /// read: the first JWT-bearer grant after
    /// [`crate::register_dynamic_client`], or a token the CLI wants
    /// replaced.
    pub fn renew(&self, account: &str, now: u64) -> Result<SmartSession, CliError> {
        let read = self.load(account)?;
        self.after_refusal(account, &read.tokens.access_token, now)
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

    /// Grant and store under the lock. A grant the server refused
    /// (`Reason::CredentialRefused`) stamps `refused_at` on the stored
    /// session and keeps the rest; one it accepts clears it.
    fn grant_and_store(&self, latest: SmartSession, now: u64) -> Result<SmartSession, CliError> {
        match pk_cli_core::block_on(self.grant(&latest, now)) {
            Ok(tokens) => {
                let renewed = SmartSession {
                    tokens,
                    refused_at: None,
                    ..latest
                };
                self.store.save(&renewed)?;
                Ok(renewed)
            }
            Err(refused) if refused.reason() == Some(Reason::CredentialRefused) => {
                // Best effort: the refusal is the answer the caller needs,
                // and a failure to record it is not reported over it.
                let _ = self.store.save(&SmartSession {
                    refused_at: Some(now),
                    ..latest
                });
                Err(refused)
            }
            Err(e) => Err(e),
        }
    }

    async fn grant(&self, session: &SmartSession, now: u64) -> Result<TokenSet, CliError> {
        if let Some(client) = &session.dynamic_client {
            let make = self.signer.ok_or_else(|| {
                CliError::Other(
                    "this session's client signs its grants, and no signer was given".into(),
                )
            })?;
            let signer = make(client)?;
            if signer.alg() != client.alg || (client.kid.is_some() && signer.kid() != client.kid) {
                return Err(CliError::Other(
                    "the signer's key is not the one this session's client registered; nothing \
                     was sent"
                        .into(),
                ));
            }
            return jwt_bearer_tokens(self.oauth, session, client, &*signer, self.quirks, now)
                .await;
        }
        if self.client.client_id() != session.client_id {
            return Err(CliError::Other(
                "this sign-in was issued to another client than the one renewing it; nothing \
                 was sent"
                    .into(),
            ));
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
