//! A client registered for one sign-in, as Epic's patient apps do: the code
//! exchange's access token is the initial access token, the registration
//! carries the client's public key, and every later access token comes from
//! the JWT-bearer grant the client signs. No refresh token.
//!
//! Registration and the first grant are two steps, so a failed grant loses
//! nothing: [`register_dynamic_client`] returns the session holding the new
//! client, the CLI stores it (`RefreshLock::store_signin`), and
//! [`crate::Renewer`] makes every grant from then on, the first included,
//! under the refresh lock.

use pk_cli_auth::oauth::{ClientAuth, ClientMetadata, OAuthClient, TokenSet};
use pk_cli_core::transport::Transport;
use pk_cli_core::CliError;
use serde_json::Value;

use crate::assertion::{jwt_bearer_grant, signed_assertion, JwtSigner, DEFAULT_LIFETIME_SECS};
use crate::discovery::SmartConfiguration;
use crate::quirks::VendorQuirks;
use crate::session::{DynamicClient, SmartSession};

/// Register a client for `session` with the public JWK Set `jwks` and
/// `software_id` (Epic: the app's own client id), authorized by the
/// session's access token. `signer` names the key's `alg` and `kid`, and
/// `key` is the private key as the signer reads it.
///
/// Nothing else is sent: the session comes back holding the client and its
/// key, with the exchange's tokens unchanged. Store it. The first JWT-bearer
/// grant is [`crate::Renewer`]'s: [`crate::Renewer::renew`] makes it now,
/// [`crate::Renewer::fresh`] when the exchange's token is due. A grant that
/// fails then leaves the stored client in place for the next try, so the
/// person need not sign in again and no second client is registered.
///
/// The registration asks for `token_endpoint_auth_method: none`, the
/// JWT-bearer grant type and no response types, which is what the grant then
/// does: the assertion is the grant (RFC 7523 §2.1), not a client
/// authentication.
pub async fn register_dynamic_client<T: Transport>(
    oauth: &OAuthClient<T>,
    config: &SmartConfiguration,
    mut session: SmartSession,
    software_id: &str,
    jwks: Value,
    signer: &dyn JwtSigner,
    key: pk_cli_secrets::Secret,
) -> Result<SmartSession, CliError> {
    let endpoint = config
        .registration_endpoint
        .as_deref()
        .ok_or_else(|| CliError::Upstream("the server offers no registration endpoint".into()))?;
    let mut metadata = ClientMetadata::native_public(&session.client_id, Vec::new()).jwks(jwks);
    metadata.software_id = Some(software_id.to_string());
    metadata.token_endpoint_auth_method = "none".into();
    metadata.grant_types = vec![crate::assertion::JWT_BEARER.into()];
    metadata.response_types = Vec::new();
    metadata.client_name = None;
    let registered = oauth
        .register(endpoint, &metadata, Some(&session.bearer()))
        .await?;
    session.dynamic_client = Some(DynamicClient::new(
        registered.client_id.clone(),
        signer.alg(),
        signer.kid(),
        key,
    ));
    Ok(session)
}

/// A fresh token set for a registered client: a signed assertion (4 minutes
/// to live), the JWT-bearer grant with the scope
/// [`VendorQuirks::refresh_scope`] gives (none at all when it gives none),
/// and the session's other members carried over. [`crate::Renewer`] calls
/// this under the refresh lock; call it directly only when no other process
/// can hold the session.
pub async fn jwt_bearer_tokens<T: Transport>(
    oauth: &OAuthClient<T>,
    session: &SmartSession,
    client: &DynamicClient,
    signer: &dyn JwtSigner,
    quirks: &dyn VendorQuirks,
    now: u64,
) -> Result<TokenSet, CliError> {
    let assertion = signed_assertion(
        signer,
        &client.client_id,
        &session.token_endpoint,
        now,
        DEFAULT_LIFETIME_SECS,
    )?;
    let mut grant = jwt_bearer_grant(&assertion);
    if let Some(scope) = quirks.refresh_scope(session) {
        grant = grant.scope(scope);
    }
    let response = oauth
        .token(
            &session.token_endpoint,
            &ClientAuth::public(&client.client_id),
            &grant,
        )
        .await?;
    Ok(TokenSet::from_response(
        response,
        now,
        Some(&session.tokens),
    ))
}
